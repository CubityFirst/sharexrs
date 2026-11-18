//! Network monitor: `NetworkMonitorViewModel`'s state without the window
//! (samples, connection transitions, the event list and log lines, the
//! summary texts), `NetworkMonitorProbe`'s choice among the endpoints'
//! answers, and `NetworkInterface.GetIsNetworkAvailable`. Sending the ICMP
//! echoes is the caller's: [`Pinger`].

use chrono::{DateTime, Duration, Local};

use crate::text::{format_dotnet, Arg};

/// `NetworkMonitorEndpoint`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub name: String,
    pub host: String,
}

/// `NetworkMonitorTargetItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub display_name: String,
    pub endpoints: Vec<Endpoint>,
}

/// `Targets`: Automatic (all three, the fastest answer wins), then each
/// resolver alone.
pub fn targets() -> Vec<Target> {
    let e = |name: &str, host: &str| Endpoint { name: name.into(), host: host.into() };
    let (cf, google, quad9) = (e("Cloudflare", "1.1.1.1"), e("Google DNS", "8.8.8.8"), e("Quad9", "9.9.9.9"));
    vec![
        Target {
            display_name: sharex_strings::tools::NetworkMonitorViewModel_Automatic_recommended().into(),
            endpoints: vec![cf.clone(), google.clone(), quad9.clone()],
        },
        Target { display_name: "Cloudflare (1.1.1.1)".into(), endpoints: vec![cf] },
        Target { display_name: "Google DNS (8.8.8.8)".into(), endpoints: vec![google] },
        Target { display_name: "Quad9 (9.9.9.9)".into(), endpoints: vec![quad9] },
    ]
}

/// `Intervals` (the default is the second, 2 s).
pub const INTERVALS: [(&str, u64); 5] = [("1 s", 1), ("2 s", 2), ("5 s", 5), ("10 s", 10), ("30 s", 30)];
pub const DEFAULT_INTERVAL: usize = 1;

/// `TimeRanges` (the default is the first): name and length, `None` for
/// the whole session.
pub fn time_ranges() -> Vec<(String, Option<Duration>)> {
    use sharex_strings::tools as s;
    vec![
        (s::NetworkMonitorViewModel_Last_5_minutes().into(), Some(Duration::minutes(5))),
        (s::NetworkMonitorViewModel_Last_15_minutes().into(), Some(Duration::minutes(15))),
        (s::NetworkMonitorViewModel_Last_hour().into(), Some(Duration::hours(1))),
        (s::NetworkMonitorViewModel_Last_6_hours().into(), Some(Duration::hours(6))),
        (s::NetworkMonitorViewModel_Session().into(), None),
    ]
}

/// Each probe's timeout (`TimeSpan.FromSeconds(2)`); an echo is abandoned
/// 250 ms after it.
pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// `DisconnectConfirmationDelay`: a failure is probed again after this.
pub const DISCONNECT_CONFIRMATION_DELAY: std::time::Duration = std::time::Duration::from_secs(1);
const MAXIMUM_VISIBLE_EVENTS: usize = 1000;

/// One endpoint's answer to an echo (`ProbeEndpointAsync`).
#[derive(Debug, Clone, PartialEq)]
pub enum EchoReply {
    /// `IPStatus.Success`: the round trip, or the stopwatch's time when the
    /// reply says 0 ms.
    Success { latency_ms: f64 },
    /// `IPStatus.TimedOut`, or no answer within the timeout.
    TimedOut,
    /// Another status or an exception: its name or message.
    Failed(String),
}

/// Sends one ICMP echo (`Ping.SendPingAsync(host, timeout)`).
pub trait Pinger {
    fn ping(&self, host: &str, timeout: std::time::Duration) -> EchoReply;
}

/// `NetworkMonitorProbeResult`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeResult {
    pub timestamp: DateTime<Local>,
    pub success: bool,
    pub latency_ms: Option<f64>,
    pub endpoint: String,
    pub method: String,
    pub error: String,
}

/// `ProbeAsync`'s result from the endpoints' replies (in the target's
/// order): the fastest success, else the distinct errors joined.
pub fn probe_result(timestamp: DateTime<Local>, target: &Target, network_available: bool, replies: &[EchoReply]) -> ProbeResult {
    use sharex_strings::tools as s;
    let fail = |error: String| ProbeResult {
        timestamp,
        success: false,
        latency_ms: None,
        endpoint: target.display_name.clone(),
        method: String::new(),
        error,
    };
    if !network_available {
        return fail(s::NetworkMonitorProbe_No_network_interface().into());
    }
    // OrderBy(latency).FirstOrDefault(): the first of the fastest.
    let mut fastest: Option<(usize, f64)> = None;
    for (i, r) in replies.iter().enumerate() {
        if let EchoReply::Success { latency_ms } = r {
            if fastest.is_none_or(|(_, l)| *latency_ms < l) {
                fastest = Some((i, *latency_ms));
            }
        }
    }
    if let Some((i, latency)) = fastest {
        return ProbeResult {
            timestamp,
            success: true,
            latency_ms: Some(latency),
            endpoint: target.endpoints[i].name.clone(),
            method: "ICMP".into(),
            error: String::new(),
        };
    }
    let mut errors: Vec<String> = Vec::new();
    for (e, r) in target.endpoints.iter().zip(replies) {
        let msg = match r {
            EchoReply::TimedOut => format_dotnet(s::NetworkMonitorProbe_Endpoint_timed_out(), &[e.name.as_str().into()]),
            EchoReply::Failed(m) => format_dotnet(s::NetworkMonitorProbe_Endpoint_failed(), &[e.name.as_str().into(), m.as_str().into()]),
            EchoReply::Success { .. } => continue,
        };
        if !msg.trim().is_empty() && !errors.contains(&msg) {
            errors.push(msg);
        }
    }
    let error = errors.join("; ");
    fail(if error.trim().is_empty() { s::NetworkMonitorProbe_No_response().into() } else { error })
}

/// `ProbeAsync` with real echoes, one endpoint after another (ShareX sends
/// them at once; only the answers matter).
pub fn probe(target: &Target, pinger: &dyn Pinger) -> ProbeResult {
    let timestamp = Local::now();
    let available = is_network_available();
    let replies: Vec<EchoReply> = if available {
        target.endpoints.iter().map(|e| pinger.ping(&e.host, PROBE_TIMEOUT)).collect()
    } else {
        Vec::new()
    };
    probe_result(timestamp, target, available, &replies)
}

/// `NetworkMonitorSample`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub timestamp: DateTime<Local>,
    pub success: bool,
    pub latency_ms: Option<f64>,
}

/// `NetworkMonitorEventStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    Connected,
    Disconnected,
}

/// `NetworkMonitorEvent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    pub timestamp: DateTime<Local>,
    pub status: EventStatus,
}

impl Event {
    /// `TimestampText`: the local time as the culture's general date ("G").
    pub fn timestamp_text(&self) -> String {
        sharex_helpers::date_format::CultureFormat::current().format_g(self.timestamp.naive_local())
    }

    /// `StatusText`.
    pub fn status_text(&self) -> &'static str {
        match self.status {
            EventStatus::Connected => sharex_strings::tools::NetworkMonitorViewModel_Connected(),
            EventStatus::Disconnected => sharex_strings::tools::NetworkMonitorViewModel_Disconnected(),
        }
    }

    /// `LogText`: `yyyy-MM-dd HH:mm:ss - Connected.` (invariant).
    pub fn log_text(&self) -> String {
        let status = match self.status {
            EventStatus::Connected => "Connected",
            EventStatus::Disconnected => "Disconnected",
        };
        format!("{} - {status}.", self.timestamp.format("%Y-%m-%d %H:%M:%S"))
    }
}

/// `NetworkMonitorViewModel`'s state.
#[derive(Debug, Clone)]
pub struct NetworkMonitor {
    pub samples: Vec<Sample>,
    /// Newest first, at most 1000.
    pub events: Vec<Event>,
    pub connected: Option<bool>,
    pub monitoring: bool,
    pub target: usize,
    pub interval: usize,
    pub time_range: usize,
    pub status_text: String,
    pub current_latency_text: String,
    pub average_latency_text: String,
    pub availability_text: String,
    pub disconnect_count_text: String,
}

impl Default for NetworkMonitor {
    fn default() -> Self {
        NetworkMonitor {
            samples: Vec::new(),
            events: Vec::new(),
            connected: None,
            monitoring: false,
            target: 0,
            interval: DEFAULT_INTERVAL,
            time_range: 0,
            status_text: sharex_strings::tools::NetworkMonitorViewModel_Checking().into(),
            current_latency_text: "—".into(),
            average_latency_text: "—".into(),
            availability_text: "—".into(),
            disconnect_count_text: "0".into(),
        }
    }
}

fn milliseconds(v: f64) -> String {
    format_dotnet(sharex_strings::tools::NetworkMonitorViewModel_Milliseconds(), &[Arg::F64(v)])
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// The probe loop's wait between rounds.
    pub fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(INTERVALS[self.interval].1)
    }

    /// `StopMonitoring`.
    pub fn stop(&mut self) {
        self.monitoring = false;
        if self.connected.is_none() {
            self.status_text = sharex_strings::tools::NetworkMonitorViewModel_Paused().into();
        }
    }

    /// `RunProbeAsync` after the probe: the sample kept (24 hours at most)
    /// and the summary updated.
    pub fn record(&mut self, result: &ProbeResult, now: DateTime<Local>) {
        self.samples.push(Sample { timestamp: result.timestamp, success: result.success, latency_ms: result.latency_ms });
        let cutoff = result.timestamp - Duration::hours(24);
        let keep = self.samples.iter().position(|s| s.timestamp >= cutoff).unwrap_or(self.samples.len());
        self.samples.drain(..keep);
        self.current_latency_text = match (result.success, result.latency_ms) {
            (true, Some(l)) => milliseconds(l),
            _ => "—".into(),
        };
        self.update_aggregates(now);
    }

    /// `ProbeOnceAsync`: whether a failed probe is tried again (after
    /// [`DISCONNECT_CONFIRMATION_DELAY`]) before it counts.
    pub fn needs_confirmation(&self, result: &ProbeResult) -> bool {
        !result.success && self.connected != Some(false)
    }

    /// `UpdateConnectionStateAsync`: a new event when the state changes
    /// (the caller appends its `log_text` to the log file).
    pub fn update_connection(&mut self, result: &ProbeResult) -> Option<Event> {
        let status = if result.success {
            if self.connected == Some(true) {
                self.status_text = sharex_strings::tools::NetworkMonitorViewModel_Connected().into();
                return None;
            }
            EventStatus::Connected
        } else {
            if self.connected == Some(false) {
                return None;
            }
            EventStatus::Disconnected
        };
        self.connected = Some(status == EventStatus::Connected);
        let event = Event { timestamp: result.timestamp, status };
        self.status_text = event.status_text().into();
        self.events.insert(0, event);
        self.events.truncate(MAXIMUM_VISIBLE_EVENTS);
        self.update_event_summary();
        Some(event)
    }

    /// `SelectedTimeRange` changed.
    pub fn set_time_range(&mut self, index: usize, now: DateTime<Local>) {
        self.time_range = index;
        self.update_aggregates(now);
    }

    /// `ChartRange`.
    pub fn chart_range(&self) -> Option<Duration> {
        time_ranges()[self.time_range].1
    }

    /// `GetVisibleSamples`.
    pub fn visible_samples(&self, now: DateTime<Local>) -> Vec<Sample> {
        match self.chart_range() {
            None => self.samples.clone(),
            Some(d) => {
                let cutoff = now - d;
                self.samples.iter().filter(|s| s.timestamp >= cutoff).copied().collect()
            }
        }
    }

    fn update_aggregates(&mut self, now: DateTime<Local>) {
        let visible = self.visible_samples(now);
        let latencies: Vec<f64> = visible.iter().filter(|s| s.success).filter_map(|s| s.latency_ms).collect();
        self.average_latency_text = if latencies.is_empty() {
            "—".into()
        } else {
            milliseconds(latencies.iter().sum::<f64>() / latencies.len() as f64)
        };
        self.availability_text = if visible.is_empty() {
            "—".into()
        } else {
            let up = visible.iter().filter(|s| s.success).count() as f64;
            format_dotnet(sharex_strings::tools::NetworkMonitorViewModel_Percentage(), &[Arg::F64(up * 100.0 / visible.len() as f64)])
        };
    }

    fn update_event_summary(&mut self) {
        self.disconnect_count_text = self.events.iter().filter(|e| e.status == EventStatus::Disconnected).count().to_string();
    }

    /// `HasEvents`.
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// `CopyAll`: the events oldest first, one log line each.
    pub fn copy_all_text(&self) -> String {
        let mut s = String::new();
        for e in self.events.iter().rev() {
            s.push_str(&e.log_text());
            s.push_str(super::NEW_LINE);
        }
        s.trim_end().to_string()
    }

    /// `ClearLogAsync` once the log file is emptied.
    pub fn clear_events(&mut self) {
        self.events.clear();
        self.update_event_summary();
    }
}

/// `NetworkMonitorEventLog.AppendAsync`: the line and a new line appended
/// (UTF-8 without a BOM), the folder and file created first.
pub fn append_log(path: &std::path::Path, event: &Event) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(format!("{}{}", event.log_text(), super::NEW_LINE).as_bytes())
}

/// `NetworkInterface.GetIsNetworkAvailable()`: an interface that's up and
/// isn't a loopback or a tunnel. Read-only. Outside Windows this isn't
/// ported yet and says yes.
pub fn is_network_available() -> bool {
    #[cfg(windows)]
    {
        windows_network_available()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[cfg(windows)]
fn windows_network_available() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::Networking::WinSock::AF_UNSPEC;
    const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
    const IF_TYPE_TUNNEL: u32 = 131;
    const IF_OPER_STATUS_UP: i32 = 1;
    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    for _ in 0..4 {
        let mut buf = vec![0u64; (size as usize).div_ceil(8)];
        let p = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        // SAFETY: `buf` holds `size` bytes, aligned for the structure.
        let r = unsafe { GetAdaptersAddresses(AF_UNSPEC as u32, flags, std::ptr::null(), p, &mut size) };
        if r == ERROR_BUFFER_OVERFLOW {
            continue;
        }
        if r != 0 {
            return false;
        }
        let mut cur = p as *const IP_ADAPTER_ADDRESSES_LH;
        while !cur.is_null() {
            // SAFETY: the list GetAdaptersAddresses wrote into `buf`.
            let a = unsafe { &*cur };
            if a.OperStatus == IF_OPER_STATUS_UP && a.IfType != IF_TYPE_SOFTWARE_LOOPBACK && a.IfType != IF_TYPE_TUNNEL {
                return true;
            }
            cur = a.Next;
        }
        return false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(s: i64) -> DateTime<Local> {
        Local.timestamp_opt(1_800_000_000 + s, 0).unwrap()
    }

    fn ok(t: i64, l: f64) -> ProbeResult {
        ProbeResult { timestamp: at(t), success: true, latency_ms: Some(l), endpoint: "x".into(), method: "ICMP".into(), error: String::new() }
    }

    fn bad(t: i64) -> ProbeResult {
        ProbeResult { timestamp: at(t), success: false, latency_ms: None, endpoint: "x".into(), method: String::new(), error: "e".into() }
    }

    #[test]
    fn probe_answers() {
        let t = &targets()[0];
        let r = probe_result(at(0), t, true, &[EchoReply::TimedOut, EchoReply::Success { latency_ms: 12.0 }, EchoReply::Success { latency_ms: 12.0 }]);
        assert_eq!((r.success, r.endpoint.as_str(), r.method.as_str()), (true, "Google DNS", "ICMP"));
        let r = probe_result(at(0), t, true, &[EchoReply::TimedOut, EchoReply::Failed("DestinationHostUnreachable".into()), EchoReply::TimedOut]);
        assert_eq!(r.error, "Cloudflare timed out.; Google DNS: DestinationHostUnreachable; Quad9 timed out.");
        assert_eq!(r.endpoint, t.display_name);
        let r = probe_result(at(0), t, false, &[]);
        assert!(!r.success && !r.error.is_empty());
    }

    #[test]
    fn transitions_and_summary() {
        let mut m = NetworkMonitor::new();
        m.stop();
        assert_eq!(m.status_text, sharex_strings::tools::NetworkMonitorViewModel_Paused());
        let r = ok(0, 10.4);
        m.record(&r, at(0));
        assert_eq!(m.update_connection(&r).map(|e| e.status), Some(EventStatus::Connected));
        assert_eq!(m.current_latency_text, "10 ms");
        let r = bad(2);
        assert!(m.needs_confirmation(&r));
        m.record(&r, at(2));
        assert_eq!(m.update_connection(&r).map(|e| e.status), Some(EventStatus::Disconnected));
        assert!(!m.needs_confirmation(&bad(4)));
        assert_eq!(m.update_connection(&bad(4)), None);
        m.record(&ok(5, 20.6), at(5));
        assert_eq!(m.average_latency_text, "16 ms");
        assert_eq!(m.availability_text, "66.7%");
        assert_eq!(m.disconnect_count_text, "1");
        assert_eq!(m.copy_all_text().lines().count(), 2);
        assert!(m.copy_all_text().ends_with(" - Disconnected."));
        // Outside the 5 minute range.
        m.set_time_range(0, at(5 + 300));
        assert_eq!(m.availability_text, "100.0%");
        // A day later the old samples are dropped.
        m.record(&ok(90_000, 1.0), at(90_000));
        assert_eq!(m.samples.len(), 1);
        let _ = is_network_available();
    }
}
