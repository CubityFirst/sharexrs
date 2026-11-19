//! `System.Net.NetworkInformation.Ping` on macOS and Linux, as .NET runs
//! ShareX's `Ping.SendPingAsync(host, timeout)` there (`Ping.Unix.cs`,
//! `Ping.RawSocket.cs`, `UnixCommandLinePing.cs`): an ICMP echo over a raw
//! socket, or an unprivileged ICMP datagram socket where the system allows
//! one (macOS always; Linux within `net.ipv4.ping_group_range`), else the
//! system's `ping` with .NET's arguments and its output parsed for `time=`.
//!
//! The packet and command-line logic is platform-neutral and tested
//! everywhere; [`ping`] itself is Unix only.

/// `Ping.DefaultSendBuffer`: 32 bytes, `'a' + i % 23` (Windows ping's).
pub fn default_send_buffer() -> [u8; 32] {
    std::array::from_fn(|i| b'a' + (i % 23) as u8)
}

/// `IcmpHeaderLengthInBytes`.
pub const ICMP_HEADER_LEN: usize = 8;
/// `MinIpHeaderLengthInBytes`.
pub const MIN_IP_HEADER_LEN: usize = 20;
/// `MaxIpHeaderLengthInBytes`.
pub const MAX_IP_HEADER_LEN: usize = 60;

/// `CreateSendMessageBuffer`: an ICMPv4 echo request (type 8, code 0) with
/// the identifier and sequence number in memory order, as .NET copies its
/// `IcmpHeader` struct, the payload, and the checksum big-endian.
pub fn echo_request(identifier: u16, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![8, 0, 0, 0];
    b.extend_from_slice(&identifier.to_ne_bytes());
    b.extend_from_slice(&0u16.to_ne_bytes());
    b.extend_from_slice(payload);
    let c = checksum(&b);
    b[2] = (c >> 8) as u8;
    b[3] = (c & 0xFF) as u8;
    b
}

/// `ComputeBufferChecksum`: the Internet checksum.
pub fn checksum(b: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for chunk in b.chunks(2) {
        let word = if chunk.len() == 2 { u16::from_be_bytes([chunk[0], chunk[1]]) } else { u16::from(chunk[0]) << 8 };
        sum += u32::from(word);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// `IPStatus` as `ToString()` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpStatus {
    Success,
    DestinationNetworkUnreachable,
    DestinationHostUnreachable,
    DestinationProtocolUnreachable,
    DestinationPortUnreachable,
    DestinationUnreachable,
    SourceQuench,
    TtlExpired,
    BadHeader,
    Unknown,
}

/// `IcmpV4MessageConstants.MapV4TypeToIPStatus`.
pub fn map_v4_status(kind: u8, code: u8) -> IpStatus {
    match kind {
        0 => IpStatus::Success,
        3 => match code {
            0 => IpStatus::DestinationNetworkUnreachable,
            1 => IpStatus::DestinationHostUnreachable,
            2 => IpStatus::DestinationProtocolUnreachable,
            3 => IpStatus::DestinationPortUnreachable,
            _ => IpStatus::DestinationUnreachable,
        },
        4 => IpStatus::SourceQuench,
        11 => IpStatus::TtlExpired,
        12 => IpStatus::BadHeader,
        _ => IpStatus::Unknown,
    }
}

/// `TryGetPingReply` for IPv4: the reply's status when `packet` (an IPv4
/// header, then ICMP) answers our echo `identifier`; `None` for anything
/// else read off the socket (our own request, other pings).
pub fn parse_v4_reply(packet: &[u8], identifier: u16) -> Option<IpStatus> {
    let ip_len = 4 * usize::from(*packet.first()? & 0x0F);
    if packet.len() < ip_len + ICMP_HEADER_LEN {
        return None;
    }
    let icmp = &packet[ip_len..];
    let (kind, code) = (icmp[0], icmp[1]);
    let id = match kind {
        0 => u16::from_ne_bytes([icmp[4], icmp[5]]),
        // Destination unreachable, source quench, redirect, time exceeded,
        // parameter problem: the original IP and ICMP headers follow.
        3 | 4 | 5 | 11 | 12 => {
            let data = ip_len + ICMP_HEADER_LEN;
            if data + MIN_IP_HEADER_LEN + ICMP_HEADER_LEN > packet.len() {
                return None;
            }
            let inner_ip = 4 * usize::from(packet[data] & 0x0F);
            if packet.len() - data < inner_ip + ICMP_HEADER_LEN {
                return None;
            }
            let o = data + inner_ip;
            u16::from_ne_bytes([packet[o + 4], packet[o + 5]])
        }
        _ => return None,
    };
    (id == identifier).then(|| map_v4_status(kind, code))
}

/// `UnixCommandLinePing`'s folders for `ping`/`ping6` (it never takes one
/// from `PATH`).
pub const PING_FOLDERS: [&str; 6] = ["/bin", "/sbin", "/usr/sbin", "/usr/bin", "/system/bin", "/run/current-system/sw/bin"];

/// Which flavour of `ping` takes the arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingOs {
    MacOs,
    FreeBsd,
    /// Linux and BusyBox.
    Linux,
}

/// `UnixCommandLinePing.ConstructCommandLine` with no TTL or fragment
/// options: one echo, the timeout (seconds rounded up on Linux,
/// milliseconds on macOS and FreeBSD, none for macOS's `ping6`), at least
/// 16 payload bytes (fewer and `ping` reports no time), the address.
pub fn command_line(os: PingOs, packet_size: usize, timeout_ms: u64, address: &str, ipv4: bool) -> Vec<String> {
    let mut args = vec!["-c".to_string(), "1".to_string()];
    // A zero timeout can leave some pings waiting forever.
    let timeout = timeout_ms.max(1);
    match os {
        PingOs::FreeBsd => {
            args.push(if ipv4 { "-W" } else { "-x" }.into());
            args.push(timeout.to_string());
        }
        PingOs::MacOs => {
            if ipv4 {
                args.push("-W".into());
                args.push(timeout.to_string());
            }
        }
        PingOs::Linux => {
            args.push("-W".into());
            args.push(timeout.div_ceil(1000).to_string());
        }
    }
    args.push("-s".into());
    args.push(packet_size.max(16).to_string());
    args.push(address.into());
    args
}

/// `UnixCommandLinePing.ParseRoundTripTime`: the number between `time=`
/// and the space before `ms`, rounded half to even.
pub fn parse_round_trip_time(output: &str) -> Option<i64> {
    let after = output.find("time=")? + "time=".len();
    let ms = after + output[after..].find("ms")?;
    let number = output.get(after..ms.checked_sub(1)?)?;
    let v: f64 = number.trim().parse().ok()?;
    Some(v.round_ties_even() as i64)
}

/// `SendWithPingUtility`'s answer from the exit and output: not exited in
/// time, or exit code 1 or 2, is a time-out; otherwise the round trip, and
/// output without one is .NET's `PingException`.
pub fn utility_reply(exited_in_time: bool, exit_code: Option<i32>, output: &str) -> Result<Option<i64>, String> {
    if !exited_in_time || matches!(exit_code, Some(1) | Some(2)) {
        return Ok(None);
    }
    parse_round_trip_time(output).map(Some).ok_or_else(|| "An exception occurred during a Ping request.".into())
}

/// `Ping.SendPingAsync(host, timeout)` with ShareX's latency rule (a 0 ms
/// round trip takes the stopwatch's time).
#[cfg(unix)]
pub fn ping(host: &str, timeout: std::time::Duration) -> super::network_monitor::EchoReply {
    use super::network_monitor::EchoReply;
    use std::net::{IpAddr, ToSocketAddrs};
    let started = std::time::Instant::now();
    let address = match host.parse::<IpAddr>() {
        Ok(a) => a,
        Err(_) => match (host, 0).to_socket_addrs().map(|mut a| a.next()) {
            Ok(Some(a)) => a.ip(),
            Ok(None) => return EchoReply::Failed("No such host is known.".into()),
            Err(e) => return EchoReply::Failed(e.to_string()),
        },
    };
    let timeout_ms = timeout.as_millis() as u64;
    let result = match address {
        IpAddr::V4(v4) => match icmp_socket::open() {
            Some(socket) => icmp_socket::echo(socket, v4, timeout_ms),
            None => utility(address, timeout_ms),
        },
        // The monitor's targets are IPv4; ping6 for anything else.
        IpAddr::V6(_) => utility(address, timeout_ms),
    };
    match result {
        Ok(Some((IpStatus::Success, rtt))) => {
            EchoReply::Success { latency_ms: if rtt > 0 { rtt as f64 } else { started.elapsed().as_secs_f64() * 1000.0 } }
        }
        Ok(None) => EchoReply::TimedOut,
        Ok(Some((status, _))) => EchoReply::Failed(format!("{status:?}")),
        Err(e) => EchoReply::Failed(e),
    }
}

/// `SendWithPingUtility`: `(status, rtt)`, `None` for a time-out.
#[cfg(unix)]
fn utility(address: std::net::IpAddr, timeout_ms: u64) -> Result<Option<(IpStatus, i64)>, String> {
    use std::process::{Command, Stdio};
    let ipv4 = address.is_ipv4();
    let name = if ipv4 { "ping" } else { "ping6" };
    let exe = PING_FOLDERS
        .iter()
        .map(|d| std::path::Path::new(d).join(name))
        .find(|p| p.is_file())
        .ok_or_else(|| "The system's ping utility could not be found.".to_string())?;
    let os = if cfg!(target_os = "macos") {
        PingOs::MacOs
    } else if cfg!(target_os = "freebsd") {
        PingOs::FreeBsd
    } else {
        PingOs::Linux
    };
    let mut child = Command::new(exe)
        .args(command_line(os, default_send_buffer().len(), timeout_ms, &address.to_string(), ipv4))
        // Output the parser understands, whatever the user's locale.
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(5)),
            _ => break None,
        }
    };
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let mut output = String::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        let _ = out.read_to_string(&mut output);
    }
    Ok(utility_reply(status.is_some(), status.and_then(|s| s.code()), &output)?.map(|rtt| (IpStatus::Success, rtt)))
}

#[cfg(unix)]
mod icmp_socket {
    use std::mem::MaybeUninit;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::time::{Duration, Instant};

    use socket2::{Domain, Protocol, SockAddr, Socket, Type};

    use super::{ICMP_HEADER_LEN, IpStatus, MAX_IP_HEADER_LEN, MIN_IP_HEADER_LEN, default_send_buffer, echo_request, parse_v4_reply};

    /// `GetRawSocket`: raw when permitted, else an ICMP datagram socket;
    /// `None` when neither is (`CanUseRawSockets` false: the utility).
    pub(super) fn open() -> Option<(Socket, bool)> {
        if let Ok(s) = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)) {
            return Some((s, true));
        }
        Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::ICMPV4)).ok().map(|s| (s, false))
    }

    /// `SendIcmpEchoRequestOverRawSocket`.
    pub(super) fn echo((socket, raw): (Socket, bool), address: Ipv4Addr, timeout_ms: u64) -> Result<Option<(IpStatus, i64)>, String> {
        let timeout = Duration::from_millis(timeout_ms.max(1));
        let _ = socket.set_read_timeout(Some(timeout));
        let _ = socket.set_write_timeout(Some(timeout));
        // .NET identifies its echo by the managed thread's id.
        let mut identifier = (std::process::id() as u16) ^ (thread_number() as u16);
        let payload = default_send_buffer();
        let to = SockAddr::from(SocketAddr::from((address, 0)));
        socket.send_to(&echo_request(identifier, &payload), &to).map_err(|e| e.to_string())?;
        // Linux's datagram sockets swap the identifier for the socket's
        // port and hand back the ICMP message without its IP header.
        let linux_dgram = cfg!(target_os = "linux") && !raw;
        if linux_dgram && let Some(port) = socket.local_addr().ok().and_then(|a| a.as_socket()).map(|a| a.port()) {
            identifier = port;
        }
        let mut buf = [MaybeUninit::<u8>::uninit(); MAX_IP_HEADER_LEN + ICMP_HEADER_LEN + 32];
        let started = Instant::now();
        while started.elapsed() < timeout {
            let _ = socket.set_read_timeout(Some(timeout.saturating_sub(started.elapsed()).max(Duration::from_millis(1))));
            let n = match socket.recv(&mut buf) {
                Ok(n) => n,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => break,
                Err(e) => return Err(e.to_string()),
            };
            // SAFETY: `recv` initialised the first `n` bytes.
            let bytes: Vec<u8> = buf[..n].iter().map(|b| unsafe { b.assume_init() }).collect();
            let packet = if linux_dgram { with_ip_header(&bytes) } else { bytes };
            if packet.len() < MIN_IP_HEADER_LEN + ICMP_HEADER_LEN {
                continue;
            }
            if let Some(status) = parse_v4_reply(&packet, identifier) {
                return Ok(Some((status, started.elapsed().as_millis() as i64)));
            }
        }
        Ok(None)
    }

    /// A minimal IPv4 header in front of a bare ICMP message.
    fn with_ip_header(icmp: &[u8]) -> Vec<u8> {
        let mut p = vec![0x45];
        p.resize(MIN_IP_HEADER_LEN, 0);
        p.extend_from_slice(icmp);
        p
    }

    /// A small number for this thread (`Environment.CurrentManagedThreadId`).
    fn thread_number() -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        std::thread::current().id().hash(&mut h);
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_buffer_is_windows_pings() {
        assert_eq!(&default_send_buffer(), b"abcdefghijklmnopqrstuvwabcdefghi");
    }

    #[test]
    fn checksum_verifies_to_zero() {
        let p = echo_request(0x1234, &default_send_buffer());
        assert_eq!(p.len(), 40);
        assert_eq!(p[0..2], [8, 0]);
        assert_eq!(checksum(&p), 0);
        // RFC 1071's example.
        assert_eq!(checksum(&[0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7]), !0xddf2);
    }

    fn reply(kind: u8, code: u8, id: u16) -> Vec<u8> {
        let mut p = vec![0x45];
        p.resize(20, 0);
        p.extend_from_slice(&[kind, code, 0, 0]);
        p.extend_from_slice(&id.to_ne_bytes());
        p.extend_from_slice(&[0, 0]);
        p
    }

    #[test]
    fn replies_match_our_identifier() {
        assert_eq!(parse_v4_reply(&reply(0, 0, 7), 7), Some(IpStatus::Success));
        assert_eq!(parse_v4_reply(&reply(0, 0, 8), 7), None);
        // Our own echo request read back (pinging localhost).
        assert_eq!(parse_v4_reply(&reply(8, 0, 7), 7), None);
        // An error carries the original IP and ICMP headers.
        let mut unreachable = reply(3, 1, 0);
        unreachable.extend(reply(8, 0, 7));
        assert_eq!(parse_v4_reply(&unreachable, 7), Some(IpStatus::DestinationHostUnreachable));
        let mut expired = reply(11, 0, 0);
        expired.extend(reply(8, 0, 9));
        assert_eq!(parse_v4_reply(&expired, 9), Some(IpStatus::TtlExpired));
        assert_eq!(parse_v4_reply(&expired[..30], 9), None);
        assert_eq!(map_v4_status(3, 9), IpStatus::DestinationUnreachable);
        assert_eq!(map_v4_status(12, 0), IpStatus::BadHeader);
        assert_eq!(map_v4_status(5, 0), IpStatus::Unknown);
    }

    #[test]
    fn command_lines_follow_dotnet() {
        let s = |v: Vec<String>| v.join(" ");
        assert_eq!(s(command_line(PingOs::Linux, 32, 1000, "1.1.1.1", true)), "-c 1 -W 1 -s 32 1.1.1.1");
        assert_eq!(s(command_line(PingOs::Linux, 32, 1500, "1.1.1.1", true)), "-c 1 -W 2 -s 32 1.1.1.1");
        assert_eq!(s(command_line(PingOs::Linux, 32, 0, "1.1.1.1", true)), "-c 1 -W 1 -s 32 1.1.1.1");
        assert_eq!(s(command_line(PingOs::MacOs, 32, 1500, "8.8.8.8", true)), "-c 1 -W 1500 -s 32 8.8.8.8");
        assert_eq!(s(command_line(PingOs::MacOs, 32, 1500, "::1", false)), "-c 1 -s 32 ::1");
        assert_eq!(s(command_line(PingOs::FreeBsd, 8, 1500, "::1", false)), "-c 1 -x 1500 -s 16 ::1");
    }

    #[test]
    fn round_trips_parse_as_dotnet() {
        let linux = "PING 1.1.1.1 (1.1.1.1) 32(60) bytes of data.\n40 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=12.5 ms\n";
        assert_eq!(parse_round_trip_time(linux), Some(12));
        assert_eq!(parse_round_trip_time("x time=13.5 ms"), Some(14));
        let mac = "40 bytes from 8.8.8.8: icmp_seq=0 ttl=117 time=9.871 ms";
        assert_eq!(parse_round_trip_time(mac), Some(10));
        assert_eq!(parse_round_trip_time("time<1ms"), None);
        assert_eq!(utility_reply(true, Some(0), mac), Ok(Some(10)));
        assert_eq!(utility_reply(true, Some(1), ""), Ok(None));
        assert_eq!(utility_reply(false, None, ""), Ok(None));
        assert!(utility_reply(true, Some(0), "no time here").is_err());
    }
}
