//! `ShareX.HelpersLib/SingleInstanceManager.cs` (with `MutexManager`): the
//! first copy holds a named mutex and listens on a named pipe; a later
//! copy finds the mutex taken, sends its command line down the pipe and
//! leaves. The first copy hands each command line it receives to a
//! callback (ShareX's `ArgumentsReceived`).
//!
//! The message is what ShareX's `BinaryWriter` writes: the number of
//! arguments as a little-endian `Int32` (at most 100), then each argument
//! as a 7-bit encoded UTF-8 byte count and its UTF-8 bytes. The sender
//! connects within 5 seconds or gives up.
//!
//! - Windows, as ShareX: a session-local mutex (`CreateMutexW`, owned when
//!   `WaitForSingleObject(0)` returns), and `\\.\pipe\<name>`, one instance at
//!   a time, readable and writable only by the current user. The sender lets
//!   the first copy take the foreground (`AllowSetForegroundWindow` with
//!   the pipe server's process ID), since only the foreground process may.
//! - macOS and Linux (ShareX has no build there): the mutex is an exclusive
//!   lock on `<name>.lock` (released when the process ends, however it
//!   ends) and the pipe a Unix socket `<name>.sock` (mode 0600), both in the
//!   per-user runtime folder (`XDG_RUNTIME_DIR`, else the temporary folder).

use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// `SingleInstanceManager.MaxArgumentsLength`.
pub const MAX_ARGUMENTS_LENGTH: i32 = 100;
/// `SingleInstanceManager.ConnectTimeout`.
pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);

/// What the first copy calls with each command line it receives.
pub type ArgumentsReceived = Arc<dyn Fn(Vec<String>) + Send + Sync>;

/// `SingleInstanceManager`.
pub struct SingleInstanceManager {
    pub mutex_name: String,
    pub pipe_name: String,
    pub is_single_instance: bool,
    pub is_first_instance: bool,
    // Dropped in this order: the listener stops, then the mutex goes.
    listener: Option<Listener>,
    _mutex: platform::Mutex,
}

impl SingleInstanceManager {
    /// `new SingleInstanceManager(mutexName, pipeName, isSingleInstance, args)`:
    /// takes the mutex; with `is_single_instance`, the first copy starts
    /// listening and a later one sends `args` to it (errors are logged, as
    /// ShareX's `DebugHelper.WriteException`).
    pub fn new(
        mutex_name: &str,
        pipe_name: &str,
        is_single_instance: bool,
        args: &[String],
        arguments_received: ArgumentsReceived,
    ) -> Self {
        let mutex = platform::Mutex::new(mutex_name);
        let is_first_instance = mutex.has_handle;
        let mut listener = None;
        if is_single_instance {
            if is_first_instance {
                listener = Some(Listener::start(pipe_name, arguments_received));
            } else if let Err(e) = redirect_arguments_to_first_instance(pipe_name, args) {
                eprintln!("SingleInstanceManager: couldn't pass the arguments on: {e}");
            }
        }
        Self {
            mutex_name: mutex_name.to_owned(),
            pipe_name: pipe_name.to_owned(),
            is_single_instance,
            is_first_instance,
            listener,
            _mutex: mutex,
        }
    }
}

impl SingleInstanceManager {
    /// Whether this copy listens for later ones.
    pub fn is_listening(&self) -> bool {
        self.listener.is_some()
    }
}

/// `RedirectArgumentsToFirstInstance`.
pub fn redirect_arguments_to_first_instance(pipe_name: &str, args: &[String]) -> io::Result<()> {
    platform::send(pipe_name, &encode_arguments(args), CONNECT_TIMEOUT)
}

/// The listening thread (`ListenForConnectionsAsync`) and its cancellation.
struct Listener {
    stop: Arc<AtomicBool>,
    pipe_name: String,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    fn start(pipe_name: &str, callback: ArgumentsReceived) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let server = platform::Server::new(pipe_name);
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || listen(server, &stop, &callback))
                .ok()
        };
        Self { stop, pipe_name: pipe_name.to_owned(), thread }
    }
}

impl Drop for Listener {
    /// `Dispose`: `cts.Cancel()`. A blocking accept wakes up to a connection
    /// of our own, so the thread sees the flag and ends.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            if !thread.is_finished() {
                let _ = platform::wake(&self.pipe_name);
            }
            let _ = thread.join();
        }
        platform::cleanup(&self.pipe_name);
    }
}

fn listen(mut server: platform::Server, stop: &AtomicBool, callback: &ArgumentsReceived) {
    while !stop.load(Ordering::SeqCst) {
        let mut connection = match server.accept() {
            Ok(c) => c,
            // The pipe couldn't be made (ShareX's `namedPipeServerCreated`
            // false): stop listening.
            Err(platform::AcceptError::Create(e)) => {
                eprintln!("SingleInstanceManager: {e}");
                break;
            }
            // A connection that failed: logged, and on to the next.
            Err(platform::AcceptError::Connect(e)) => {
                eprintln!("SingleInstanceManager: {e}");
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        };
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match decode_arguments(&mut connection) {
            Ok(args) => callback(args),
            Err(e) => eprintln!("SingleInstanceManager: {e}"),
        }
    }
}

/// What ShareX's sender writes: `BinaryWriter.Write(args.Length)`, then
/// `Write(string)` for each argument.
pub fn encode_arguments(args: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(args.len() as i32).to_le_bytes());
    for a in args {
        write_7bit_encoded_int(&mut out, a.len() as u32);
        out.extend_from_slice(a.as_bytes());
    }
    out
}

/// What ShareX's listener reads (`BinaryReader.ReadInt32`, then
/// `ReadString` for each), rejecting a count below 0 or above 100.
pub fn decode_arguments(r: &mut impl Read) -> io::Result<Vec<String>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let length = i32::from_le_bytes(len);
    if !(0..=MAX_ARGUMENTS_LENGTH).contains(&length) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid length: {length}")));
    }
    let mut args = Vec::with_capacity(length as usize);
    for _ in 0..length {
        let n = read_7bit_encoded_int(r)?;
        let mut bytes = vec![0u8; n as usize];
        r.read_exact(&mut bytes)?;
        // `Encoding.UTF8` replaces what isn't UTF-8.
        args.push(String::from_utf8_lossy(&bytes).into_owned());
    }
    Ok(args)
}

/// `BinaryWriter.Write7BitEncodedInt`.
fn write_7bit_encoded_int(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// `BinaryReader.Read7BitEncodedInt`: at most 5 bytes, the fifth using 4 bits.
fn read_7bit_encoded_int(r: &mut impl Read) -> io::Result<i32> {
    let mut result: u32 = 0;
    for shift in (0..35).step_by(7) {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        let byte = b[0];
        if shift == 28 && byte > 0x0F {
            break;
        }
        result |= u32::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            let value = result as i32;
            if value < 0 {
                break;
            }
            return Ok(value);
        }
    }
    Err(io::Error::new(io::ErrorKind::InvalidData, "Too many bytes in what should have been a 7-bit encoded integer."))
}

#[cfg(windows)]
mod platform {
    use std::io::{self, Read};
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, ERROR_SEM_TIMEOUT,
        GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_ABANDONED, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FlushFileBuffers, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
    };
    use windows_sys::Win32::System::Threading::{
        CreateMutexW, GetCurrentProcess, OpenProcessToken, ReleaseMutex, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn pipe_path(name: &str) -> Vec<u16> {
        wide(&format!(r"\\.\pipe\{name}"))
    }

    pub enum AcceptError {
        Create(io::Error),
        Connect(io::Error),
    }

    /// `MutexManager`: `new Mutex(false, name)`, then `WaitOne(0)` (an
    /// abandoned mutex counts as taken).
    pub struct Mutex {
        handle: HANDLE,
        pub has_handle: bool,
    }

    impl Mutex {
        pub fn new(name: &str) -> Self {
            let name = wide(name);
            // SAFETY: a null-terminated name; the handle is closed on drop.
            let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
            if handle.is_null() {
                // .NET throws here; carry on as the only copy rather than
                // passing the arguments to nobody.
                eprintln!("SingleInstanceManager: CreateMutex failed ({})", io::Error::last_os_error());
                return Self { handle, has_handle: true };
            }
            // SAFETY: a valid mutex handle.
            let wait = unsafe { WaitForSingleObject(handle, 0) };
            Self { handle, has_handle: wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED }
        }
    }

    impl Drop for Mutex {
        fn drop(&mut self) {
            if self.handle.is_null() {
                return;
            }
            // SAFETY: our handle; ReleaseMutex only when we own it.
            unsafe {
                if self.has_handle {
                    ReleaseMutex(self.handle);
                }
                CloseHandle(self.handle);
            }
        }
    }

    /// A pipe end, read until the sender closes it.
    pub struct Pipe(HANDLE);

    // SAFETY: a pipe handle may be used from any thread.
    unsafe impl Send for Pipe {}

    impl Read for Pipe {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut read = 0u32;
            let len = buf.len().min(u32::MAX as usize) as u32;
            // SAFETY: `buf` is `len` bytes long.
            let ok = unsafe { ReadFile(self.0, buf.as_mut_ptr(), len, &mut read, std::ptr::null_mut()) };
            if ok == 0 {
                // SAFETY: no other call in between.
                let err = unsafe { GetLastError() };
                return if err == ERROR_BROKEN_PIPE { Ok(0) } else { Err(io::Error::from_raw_os_error(err as i32)) };
            }
            Ok(read as usize)
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            // SAFETY: our handle.
            unsafe {
                DisconnectNamedPipe(self.0);
                CloseHandle(self.0);
            }
        }
    }

    /// `NamedPipeServerStreamAcl.Create(pipeName, InOut, 1, Byte,
    /// Asynchronous, 0, 0, pipeSecurity)`: one instance, the current user
    /// allowed `PipeAccessRights.ReadWrite` (plus the `Synchronize` .NET
    /// adds to an allow rule), remote clients refused.
    pub struct Server {
        path: Vec<u16>,
        security: Option<SecurityDescriptor>,
    }

    // SAFETY: the security descriptor is only read, by one thread at a time.
    unsafe impl Send for Server {}

    impl Server {
        pub fn new(name: &str) -> Self {
            Self { path: pipe_path(name), security: SecurityDescriptor::current_user() }
        }

        /// A new pipe instance, once a client connects to it.
        pub fn accept(&mut self) -> Result<Pipe, AcceptError> {
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.security.as_ref().map_or(std::ptr::null_mut(), |s| s.0),
                bInheritHandle: 0,
            };
            // SAFETY: a null-terminated path and attributes that outlive the call.
            let handle = unsafe {
                CreateNamedPipeW(
                    self.path.as_ptr(),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    0,
                    0,
                    0,
                    &attributes,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(AcceptError::Create(io::Error::last_os_error()));
            }
            let pipe = Pipe(handle);
            // SAFETY: a valid pipe handle, synchronous.
            let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } != 0
                || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if connected { Ok(pipe) } else { Err(AcceptError::Connect(io::Error::last_os_error())) }
        }
    }

    /// `PipeSecurity` with one rule: the current user's SID, allowed
    /// `0x12019B` (ReadWrite | Synchronize).
    struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

    impl SecurityDescriptor {
        fn current_user() -> Option<Self> {
            let sid = current_user_sid()?;
            let sddl = wide(&format!("D:(A;;0x12019b;;;{sid})"));
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            // SAFETY: a null-terminated SDDL string; `sd` is freed on drop.
            let ok = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut())
            };
            (ok != 0).then_some(Self(sd))
        }
    }

    impl Drop for SecurityDescriptor {
        fn drop(&mut self) {
            // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
            unsafe { LocalFree(self.0) };
        }
    }

    /// `WindowsIdentity.GetCurrent().User`, as an SDDL SID string.
    fn current_user_sid() -> Option<String> {
        // SAFETY: the token and buffers are ours and sized as the calls ask.
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return None;
            }
            let mut len = 0u32;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
            let mut buf = vec![0u64; (len as usize).div_ceil(8)];
            let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), len, &mut len);
            CloseHandle(token);
            if ok == 0 {
                return None;
            }
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s: windows_sys::core::PWSTR = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
                return None;
            }
            let n = (0..).take_while(|&i| *s.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
            LocalFree(s.cast());
            Some(sid)
        }
    }

    /// `NamedPipeClientStream(".", pipeName, Out).Connect(timeout)`: waits
    /// for the pipe while it's busy or not there yet, until the timeout.
    fn connect(name: &str, timeout: Duration) -> io::Result<HANDLE> {
        let path = pipe_path(name);
        let deadline = Instant::now() + timeout;
        loop {
            // SAFETY: a null-terminated path.
            let handle = unsafe {
                CreateFileW(path.as_ptr(), GENERIC_WRITE, 0, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut())
            };
            if handle != INVALID_HANDLE_VALUE {
                return Ok(handle);
            }
            // SAFETY: right after the failed call.
            let err = unsafe { GetLastError() };
            if err != ERROR_PIPE_BUSY && err != ERROR_FILE_NOT_FOUND && err != ERROR_SEM_TIMEOUT {
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "The operation has timed out."));
            }
            if err == ERROR_PIPE_BUSY {
                // SAFETY: as above.
                unsafe { WaitNamedPipeW(path.as_ptr(), left.as_millis().min(u32::MAX as u128) as u32) };
            } else {
                std::thread::sleep(left.min(Duration::from_millis(10)));
            }
        }
    }

    pub fn send(name: &str, data: &[u8], timeout: Duration) -> io::Result<()> {
        let handle = connect(name, timeout)?;
        // `AllowFirstInstanceToSetForeground`.
        let mut pid = 0u32;
        // SAFETY: a connected client handle.
        unsafe {
            if GetNamedPipeServerProcessId(handle, &mut pid) != 0 {
                AllowSetForegroundWindow(pid);
            }
        }
        let mut written_all = Ok(());
        let mut rest = data;
        while !rest.is_empty() {
            let mut written = 0u32;
            // SAFETY: `rest` is valid for its length.
            let ok = unsafe { WriteFile(handle, rest.as_ptr(), rest.len() as u32, &mut written, std::ptr::null_mut()) };
            if ok == 0 {
                written_all = Err(io::Error::last_os_error());
                break;
            }
            rest = &rest[written as usize..];
        }
        // SAFETY: our handle. Flushing waits until the server has read it all.
        unsafe {
            FlushFileBuffers(handle);
            CloseHandle(handle);
        }
        written_all
    }

    /// Wakes a listener blocked in `accept` with an empty connection.
    pub fn wake(name: &str) -> io::Result<()> {
        let handle = connect(name, Duration::from_millis(500))?;
        // SAFETY: our handle.
        unsafe { CloseHandle(handle) };
        Ok(())
    }

    pub fn cleanup(_name: &str) {}
}

#[cfg(unix)]
mod platform {
    use std::fs::{File, OpenOptions};
    use std::io::{self, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// The per-user folder for the lock and the socket.
    fn folder() -> PathBuf {
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(std::env::temp_dir)
    }

    /// `name` as a file name, short enough for a socket path (104 bytes on
    /// macOS): what isn't a letter, digit, `-`, `_` or `.` becomes `_`, and a
    /// long name is cut and ends with its hash.
    fn file_name(name: &str, extension: &str) -> String {
        let mut clean: String =
            name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect();
        if clean.len() > 40 {
            let hash = name.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3));
            clean.truncate(23);
            clean = format!("{clean}-{hash:016x}");
        }
        format!("{clean}.{extension}")
    }

    pub enum AcceptError {
        Create(io::Error),
        Connect(io::Error),
    }

    fn socket_path(name: &str) -> PathBuf {
        folder().join(file_name(name, "sock"))
    }

    /// The mutex: an exclusive lock on `<name>.lock`, held while the file is open.
    pub struct Mutex {
        _file: Option<File>,
        pub has_handle: bool,
    }

    impl Mutex {
        pub fn new(name: &str) -> Self {
            let path = folder().join(file_name(name, "lock"));
            let file = match OpenOptions::new().create(true).truncate(false).write(true).open(&path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("SingleInstanceManager: {}: {e}", path.display());
                    return Self { _file: None, has_handle: true };
                }
            };
            match file.try_lock() {
                Ok(()) => Self { _file: Some(file), has_handle: true },
                Err(std::fs::TryLockError::WouldBlock) => Self { _file: None, has_handle: false },
                Err(std::fs::TryLockError::Error(e)) => {
                    eprintln!("SingleInstanceManager: {}: {e}", path.display());
                    Self { _file: None, has_handle: true }
                }
            }
        }
    }

    pub struct Server {
        path: PathBuf,
        listener: Option<UnixListener>,
    }

    impl Server {
        pub fn new(name: &str) -> Self {
            Self { path: socket_path(name), listener: None }
        }

        pub fn accept(&mut self) -> Result<UnixStream, AcceptError> {
            if self.listener.is_none() {
                // Only the mutex's owner gets here: a socket left by a copy
                // that crashed is stale.
                let _ = std::fs::remove_file(&self.path);
                let listener = UnixListener::bind(&self.path).map_err(AcceptError::Create)?;
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).map_err(AcceptError::Create)?;
                self.listener = Some(listener);
            }
            let listener = self.listener.as_ref().expect("bound above");
            listener.accept().map(|(s, _)| s).map_err(AcceptError::Connect)
        }
    }

    fn connect(name: &str, timeout: Duration) -> io::Result<UnixStream> {
        let path = socket_path(name);
        let deadline = Instant::now() + timeout;
        loop {
            match UnixStream::connect(&path) {
                Ok(s) => return Ok(s),
                Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "The operation has timed out."));
                    }
                    std::thread::sleep(left.min(Duration::from_millis(10)));
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn send(name: &str, data: &[u8], timeout: Duration) -> io::Result<()> {
        let mut stream = connect(name, timeout)?;
        stream.write_all(data)?;
        stream.flush()?;
        let _ = stream.shutdown(std::net::Shutdown::Write);
        Ok(())
    }

    pub fn wake(name: &str) -> io::Result<()> {
        connect(name, Duration::from_millis(500)).map(drop)
    }

    pub fn cleanup(name: &str) {
        let _ = std::fs::remove_file(socket_path(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn binary_writer_format() {
        let long = "x".repeat(200);
        let args = strings(&["-a", "é", "", &long]);
        let bytes = encode_arguments(&args);
        let mut expected = vec![4, 0, 0, 0, 2, b'-', b'a', 2, 0xC3, 0xA9, 0];
        // 200 as a 7-bit encoded int: 0xC8 0x01.
        expected.extend([0xC8, 0x01]);
        expected.extend(long.as_bytes());
        assert_eq!(bytes, expected);
        assert_eq!(decode_arguments(&mut bytes.as_slice()).unwrap(), args);
    }

    #[test]
    fn bad_messages_are_refused() {
        assert!(decode_arguments(&mut [101u8, 0, 0, 0].as_slice()).is_err());
        assert!(decode_arguments(&mut [0xFFu8, 0xFF, 0xFF, 0xFF].as_slice()).is_err());
        // Truncated.
        assert!(decode_arguments(&mut [1u8, 0, 0, 0, 5, b'a'].as_slice()).is_err());
        // A 7-bit int that runs on.
        assert!(decode_arguments(&mut [1u8, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01].as_slice()).is_err());
        assert_eq!(decode_arguments(&mut [0u8, 0, 0, 0].as_slice()).unwrap(), Vec::<String>::new());
    }

    fn unique(name: &str) -> String {
        format!("sharex-helpers-test-{name}-{}", std::process::id())
    }

    #[test]
    fn a_second_instance_passes_its_arguments_to_the_first() {
        let (mutex, pipe) = (unique("mutex"), unique("pipe"));
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let first = SingleInstanceManager::new(
            &mutex,
            &pipe,
            true,
            &[],
            Arc::new(move |a| tx.lock().unwrap().send(a).unwrap()),
        );
        assert!(first.is_first_instance);
        // The mutex is held by this thread; a second copy asks from another
        // thread, as a second process would (Windows mutexes are re-entrant
        // on the thread that owns them).
        let (m, p) = (mutex.clone(), pipe.clone());
        let second = std::thread::spawn(move || {
            let args = strings(&["-workflow", "My task", "C:\\a b.png"]);
            let s = SingleInstanceManager::new(&m, &p, true, &args, Arc::new(|_| panic!("the second copy received")));
            (s.is_first_instance, s.is_single_instance)
        })
        .join()
        .unwrap();
        assert_eq!(second, (false, true));
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got, strings(&["-workflow", "My task", "C:\\a b.png"]));
        // Again: the listener goes on after each message.
        let (m, p) = (mutex.clone(), pipe.clone());
        std::thread::spawn(move || drop(SingleInstanceManager::new(&m, &p, true, &[], Arc::new(|_| {}))))
            .join()
            .unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Vec::<String>::new());
        drop(first);
        // Gone: the mutex is free again.
        let (m, p) = (mutex.clone(), pipe.clone());
        let again = std::thread::spawn(move || SingleInstanceManager::new(&m, &p, true, &[], Arc::new(|_| {})).is_first_instance)
            .join()
            .unwrap();
        assert!(again);
    }

    #[test]
    fn multi_instance_doesnt_listen_or_send() {
        let (mutex, pipe) = (unique("multi-mutex"), unique("multi-pipe"));
        let first = SingleInstanceManager::new(&mutex, &pipe, false, &[], Arc::new(|_| {}));
        assert!(first.is_first_instance && !first.is_listening());
        let (m, p) = (mutex.clone(), pipe.clone());
        let start = std::time::Instant::now();
        let second = std::thread::spawn(move || SingleInstanceManager::new(&m, &p, false, &strings(&["x"]), Arc::new(|_| {})).is_first_instance)
            .join()
            .unwrap();
        assert!(!second);
        // It didn't wait 5 seconds for a pipe nobody listens on.
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn nobody_listening_times_out() {
        let start = std::time::Instant::now();
        let r = platform::send(&unique("nobody"), &encode_arguments(&[]), Duration::from_millis(300));
        assert!(r.is_err());
        assert!(start.elapsed() >= Duration::from_millis(250));
    }
}
