//! Watch folders on scratch folders: the wait, the filter and subfolders,
//! duplicates, the manager and moving to the screenshots folder.

use super::*;
use std::sync::mpsc;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sharexrs-watch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // FSEvents reports real paths (`/private/var/...` for `/var/...`).
    #[cfg(not(windows))]
    let dir = dir.canonicalize().unwrap();
    dir
}

/// ShareX's waits divided by 10 (duplicates still within ShareX's second,
/// as some backends report a creation more than once).
fn fast() -> Timing {
    Timing {
        wait_start: Duration::from_millis(100),
        interval: Duration::from_millis(25),
        timeout: Duration::from_millis(500),
        duplicate_expire: Duration::from_millis(1000),
    }
}

fn folder(path: &Path, filter: &str, sub: bool) -> WatchFolderSettings {
    WatchFolderSettings { FolderPath: Some(path.to_string_lossy().into_owned()), Filter: Some(filter.into()), IncludeSubdirectories: sub, ..Default::default() }
}

fn channel_trigger() -> (Trigger, mpsc::Receiver<PathBuf>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    (Arc::new(move |p| tx.lock().unwrap().send(p).unwrap()), rx)
}

/// Every path triggered until `wait` passes with nothing new.
fn collect(rx: &mpsc::Receiver<PathBuf>, wait: Duration) -> Vec<PathBuf> {
    let mut out = Vec::new();
    while let Ok(p) = rx.recv_timeout(wait) {
        out.push(p);
    }
    out.sort();
    out
}

#[test]
fn ready_after_the_size_holds_four_checks() {
    let dir = scratch("ready");
    let f = dir.join("a.png");
    std::fs::write(&f, b"data").unwrap();
    let t = Instant::now();
    assert!(wait_until_ready(&f, fast()));
    // The start wait and four intervals after the first check.
    assert!(t.elapsed() >= Duration::from_millis(100 + 4 * 25), "{:?}", t.elapsed());
    // Empty or missing files never get there.
    let empty = dir.join("empty.png");
    std::fs::write(&empty, b"").unwrap();
    assert!(!wait_until_ready(&empty, fast()));
    assert!(!wait_until_ready(&dir.join("gone.png"), fast()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn equal_sizes_count_even_between_changes() {
    // ShareX never resets its count when the size changes: four equal
    // readings in all, not in a row, are enough.
    let dir = scratch("grow");
    let f = dir.join("g.bin");
    std::fs::write(&f, b"1").unwrap();
    let path = f.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..10 {
            std::thread::sleep(Duration::from_millis(60));
            let mut v = std::fs::read(&path).unwrap();
            v.push(i);
            std::fs::write(&path, v).unwrap();
        }
    });
    assert!(wait_until_ready(&f, fast()));
    let size_when_ready = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
    writer.join().unwrap();
    assert!(size_when_ready < 11, "ready only after the writes stopped");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(windows)]
#[test]
fn a_locked_file_waits_for_its_handle() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = scratch("locked");
    let f = dir.join("l.png");
    std::fs::write(&f, b"data").unwrap();
    // Another writer holding it (no sharing for others' exclusive opens).
    let handle = std::fs::OpenOptions::new().write(true).share_mode(1).open(&f).unwrap();
    assert!(is_file_locked(&f).unwrap());
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        drop(handle);
    });
    let t = Instant::now();
    assert!(wait_until_ready(&f, fast()));
    release.join().unwrap();
    assert!(t.elapsed() >= Duration::from_millis(250 + 4 * 25));
    // Held past the timeout: given up.
    let handle = std::fs::OpenOptions::new().write(true).share_mode(1).open(&f).unwrap();
    assert!(!wait_until_ready(&f, fast()));
    drop(handle);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn new_files_pass_the_filter_and_subfolders_setting() {
    let dir = scratch("filter");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let (trigger, rx) = channel_trigger();
    let watch = WatchFolder::enable(&folder(&dir, "*.png", false), fast(), trigger).expect("watching");
    std::thread::sleep(Duration::from_millis(100));
    for name in ["a.png", "B.PNG", "c.txt", "sub/d.png", "e.png.txt"] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    std::fs::write(dir.join("empty.png"), b"").unwrap();
    std::fs::create_dir(dir.join("folder.png")).unwrap();
    let got = collect(&rx, Duration::from_millis(1500));
    drop(watch);
    let mut want = vec![dir.join("a.png")];
    // FileSystemWatcher ignores case outside Linux.
    if !cfg!(target_os = "linux") {
        want.insert(0, dir.join("B.PNG"));
    }
    assert_eq!(got, want);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn subfolders_when_asked() {
    let dir = scratch("sub");
    std::fs::create_dir_all(dir.join("sub").join("deeper")).unwrap();
    let (trigger, rx) = channel_trigger();
    let watch = WatchFolder::enable(&folder(&dir, "", true), fast(), trigger).expect("watching");
    std::thread::sleep(Duration::from_millis(100));
    std::fs::write(dir.join("top.txt"), b"x").unwrap();
    std::fs::write(dir.join("sub").join("deeper").join("low.jpg"), b"x").unwrap();
    let got = collect(&rx, Duration::from_millis(1500));
    drop(watch);
    assert_eq!(got, vec![dir.join("sub").join("deeper").join("low.jpg"), dir.join("top.txt")]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn nothing_to_watch() {
    let (trigger, _rx) = channel_trigger();
    let missing = std::env::temp_dir().join("sharexrs-watch-no-such-folder");
    assert!(WatchFolder::enable(&folder(&missing, "", false), fast(), trigger.clone()).is_none());
    assert!(WatchFolder::enable(&WatchFolderSettings::default(), fast(), trigger).is_none());
}

#[test]
fn duplicate_events_within_the_expiry_are_one() {
    let timing = Timing { duplicate_expire: Duration::from_millis(100), ..fast() };
    let timers = Mutex::new(Vec::new());
    let (trigger, rx) = channel_trigger();
    let dir = scratch("dup");
    let f = dir.join("a.png");
    std::fs::write(&f, b"x").unwrap();
    created(f.clone(), &timers, timing, trigger.clone());
    created(f.clone(), &timers, timing, trigger.clone());
    assert_eq!(timers.lock().unwrap().len(), 1);
    // After the expiry the same path is a new file.
    std::thread::sleep(Duration::from_millis(150));
    created(f.clone(), &timers, timing, trigger);
    assert_eq!(collect(&rx, Duration::from_millis(800)), vec![f.clone(), f]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn manager_keeps_unchanged_watchers() {
    let dir = scratch("manager");
    let mut defaults = TaskSettings { WatchFolderEnabled: true, ..Default::default() };
    defaults.WatchFolderList.push(folder(&dir, "*.png", false));
    let mut hotkey = HotkeySettings::default();
    hotkey.TaskSettings.WatchFolderList.push(folder(&dir, "", false));
    let made = Arc::new(Mutex::new(Vec::new()));
    let m2 = made.clone();
    let trigger = move |id: u64| -> Trigger {
        m2.lock().unwrap().push(id);
        Arc::new(|_| {})
    };
    let mut m = WatchFolderManager { timing: fast(), ..Default::default() };
    m.update(&defaults, std::slice::from_ref(&hotkey), &trigger);
    // The hotkey's isn't enabled.
    assert_eq!((m.watching(), made.lock().unwrap().len()), (1, 1));
    m.update(&defaults, std::slice::from_ref(&hotkey), &trigger);
    assert_eq!(made.lock().unwrap().len(), 1, "unchanged: kept");
    hotkey.TaskSettings.WatchFolderEnabled = true;
    m.update(&defaults, std::slice::from_ref(&hotkey), &trigger);
    assert_eq!((m.watching(), made.lock().unwrap().len()), (2, 2));
    let (s, t) = m.get(1).unwrap();
    assert_eq!((s.Filter.as_deref(), t.WatchFolderEnabled), (Some("*.png"), true));
    defaults.WatchFolderList.clear();
    m.update(&defaults, std::slice::from_ref(&hotkey), &trigger);
    assert_eq!(m.watching(), 1);
    assert!(m.get(1).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn moves_to_the_screenshots_folder() {
    let dir = scratch("move");
    let shots = dir.join("shots");
    let orig = dir.join("in").join("a.png");
    std::fs::create_dir_all(orig.parent().unwrap()).unwrap();
    std::fs::write(&orig, b"one").unwrap();
    let task = TaskSettings { OverrideScreenshotsFolder: true, ScreenshotsFolder: shots.to_string_lossy().into_owned(), ..Default::default() };
    let app = ApplicationConfig::default();
    let mut w = folder(&dir.join("in"), "", false);
    // Not asked: the file stays.
    assert_eq!(prepare_file(&orig, &w, &task, &app, &dir), Some(orig.clone()));
    w.MoveFilesToScreenshotsFolder = true;
    assert_eq!(prepare_file(&orig, &w, &task, &app, &dir), Some(shots.join("a.png")));
    assert!(!orig.exists());
    // A name that's taken gets a unique one.
    std::fs::write(&orig, b"two").unwrap();
    let second = prepare_file(&orig, &w, &task, &app, &dir).unwrap();
    assert_ne!(second, shots.join("a.png"));
    assert_eq!(std::fs::read(&second).unwrap(), b"two");
    let _ = std::fs::remove_dir_all(dir);
}

/// ShareXrs's watcher over `tools/actions-harness`'s watch cases, at
/// ShareX's own timing, against what ShareX's `WatchFolder` triggered
/// (`fixtures/results-sharex.json`, recorded on Windows): the same files,
/// each within 400 ms of ShareX's time.
#[cfg(windows)]
#[test]
fn watch_cases_as_sharex() {
    use serde_json::Value;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/watch_folder/fixtures");
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(fixtures.join("cases.json")).unwrap()).unwrap();
    let want: Value = serde_json::from_str(&std::fs::read_to_string(fixtures.join("results-sharex.json")).unwrap()).unwrap();
    let root = scratch("cases");
    let threads: Vec<_> = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .map(|c| {
            let root = root.clone();
            std::thread::spawn(move || run_case(&c, &root))
        })
        .collect();
    let got: Vec<(String, Vec<(String, u64)>)> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let mut failures = Vec::new();
    for ((name, triggers), w) in got.iter().zip(want["cases"].as_array().unwrap()) {
        let expected: Vec<(String, u64)> =
            w["triggers"].as_array().unwrap().iter().map(|t| (t["path"].as_str().unwrap().to_owned(), t["ms"].as_u64().unwrap())).collect();
        let same_files = triggers.iter().map(|t| &t.0).eq(expected.iter().map(|t| &t.0));
        let close = triggers.iter().zip(&expected).all(|(a, b)| a.1.abs_diff(b.1) <= 400);
        if !same_files || !close {
            failures.push(format!("{name}: ShareX {expected:?}, ShareXrs {triggers:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(root);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// One watch case as the harness's `WatchCase` runs it.
#[cfg(windows)]
fn run_case(c: &serde_json::Value, root: &Path) -> (String, Vec<(String, u64)>) {
    use std::os::windows::fs::OpenOptionsExt;
    let name = c["name"].as_str().unwrap().to_owned();
    let dir = root.join(format!("watch-{name}"));
    let outside = root.join(format!("outside-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    for d in c["dirs"].as_array().into_iter().flatten() {
        std::fs::create_dir_all(dir.join(d.as_str().unwrap())).unwrap();
    }
    let s = &c["settings"];
    let settings = WatchFolderSettings {
        FolderPath: Some(dir.to_string_lossy().into_owned()),
        Filter: s["Filter"].as_str().map(str::to_owned),
        IncludeSubdirectories: s["IncludeSubdirectories"].as_bool().unwrap_or(false),
        ..Default::default()
    };
    let started: Arc<Mutex<Option<Instant>>> = Arc::default();
    let triggers: Arc<Mutex<Vec<(String, u64)>>> = Arc::default();
    let (st, tr, base) = (started.clone(), triggers.clone(), dir.clone());
    let trigger: Trigger = Arc::new(move |p: PathBuf| {
        let ms = st.lock().unwrap().map_or(0, |t| t.elapsed().as_millis() as u64);
        tr.lock().unwrap().push((p.strip_prefix(&base).unwrap().to_string_lossy().into_owned(), ms));
    });
    let watch = WatchFolder::enable(&settings, Timing::default(), trigger).expect("watching");
    std::thread::sleep(Duration::from_millis(200));
    let t0 = Instant::now();
    *started.lock().unwrap() = Some(t0);
    let mut held = Vec::new();
    for step in c["steps"].as_array().unwrap() {
        let at = step["at_ms"].as_u64().unwrap_or(0);
        let now = t0.elapsed().as_millis() as u64;
        if at > now {
            std::thread::sleep(Duration::from_millis(at - now));
        }
        let path = dir.join(step["path"].as_str().unwrap());
        match step["op"].as_str().unwrap() {
            "write" => std::fs::write(&path, vec![0u8; step["bytes"].as_u64().unwrap_or(10) as usize]).unwrap(),
            "append" => {
                use std::io::Write;
                std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(&[1]).unwrap();
            }
            "delete" => std::fs::remove_file(&path).unwrap(),
            "write_held" => {
                use std::io::Write;
                // `FileShare.Read`.
                let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).share_mode(1).open(&path).unwrap();
                f.write_all(&[0u8; 10]).unwrap();
                f.flush().unwrap();
                let hold = step["hold_ms"].as_u64().unwrap();
                held.push(std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(hold));
                    drop(f);
                }));
            }
            "move_in" => {
                let from = outside.join(path.file_name().unwrap());
                std::fs::write(&from, [0u8; 10]).unwrap();
                std::fs::rename(&from, &path).unwrap();
            }
            "rename" => std::fs::rename(dir.join(step["from"].as_str().unwrap()), &path).unwrap(),
            op => panic!("unknown step {op}"),
        }
    }
    std::thread::sleep(Duration::from_millis(c["wait_ms"].as_u64().unwrap()));
    for h in held {
        h.join().unwrap();
    }
    drop(watch);
    let mut got = triggers.lock().unwrap().clone();
    got.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    (name, got)
}
