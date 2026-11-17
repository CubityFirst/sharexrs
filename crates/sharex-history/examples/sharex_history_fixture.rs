//! Writes a synthetic History.db for validating the History, Image history
//! and Stats windows against ShareX: items over the past 14 months (dated
//! from today, so both apps show the same stats on the day it's made), every
//! type, hosts, process names, window titles, tags and favourites, pointing
//! at the test-pattern files `scripts/history-fixture.py` makes.
//!
//!     cargo run -p sharex-history --example sharex_history_fixture -- <fixture dir>
//!
//! Writes `<fixture dir>/History.db` (replacing it), the files being in
//! `<fixture dir>/files`.

use std::path::Path;

use sharex_history::{DateTime, HistoryItem, HistoryManagerSqlite, Tags};

const TICKS_PER_MINUTE: i64 = 600_000_000;

/// days ago, time, file, type, host, URL id, shortened, process, window
/// title, favourite, tag.
type Spec = (i64, (i64, i64), &'static str, &'static str, Option<&'static str>, Option<&'static str>, bool, Option<&'static str>, Option<&'static str>, bool, Option<&'static str>);

const ITEMS: &[Spec] = &[
    (420, (10, 15), "pattern-12.png", "Image", None, None, false, Some("explorer"), Some("Pictures"), false, None),
    (400, (14, 40), "pattern-11.png", "Image", Some("Imgur"), Some("k3Lm9Qa"), false, Some("chrome"), Some("Release planning - Chrome"), false, None),
    (365, (9, 5), "notes-01.txt", "Text", Some("Pastebin"), Some("tX81pZ"), false, Some("notepad"), Some("notes-01.txt - Notepad"), false, None),
    (300, (16, 20), "pattern-10.png", "Image", None, None, false, Some("code"), Some("main.rs - Visual Studio Code"), false, None),
    (240, (11, 0), "archive-01.zip", "File", Some("Dropbox"), Some("arch01"), false, Some("explorer"), Some("Downloads"), false, None),
    (180, (13, 30), "pattern-09.png", "Image", Some("Amazon S3"), Some("s3-0009"), false, Some("chrome"), Some("Dashboard - Chrome"), true, Some("dashboard")),
    (120, (15, 45), "pattern-08.png", "Image", None, None, false, Some("ShareX"), Some("ShareX"), false, None),
    (90, (10, 10), "animation-01.gif", "Image", Some("Imgur"), Some("gIf0001"), true, Some("chrome"), Some("Animation test - Chrome"), false, None),
    (60, (20, 5), "pattern-07.png", "Image", None, None, false, Some("code"), Some("lib.rs - Visual Studio Code"), false, Some("review")),
    (45, (8, 50), "photo-01.jpg", "Image", Some("Imgur"), Some("pH0t0aa"), false, Some("explorer"), Some("Photos"), true, None),
    (30, (12, 12), "pattern-06.png", "Image", None, None, false, Some("chrome"), Some("Design system - Chrome"), false, None),
    (21, (17, 35), "pattern-05.png", "Image", Some("Amazon S3"), Some("s3-0005"), false, Some("code"), Some("theme.rs - Visual Studio Code"), false, None),
    (14, (9, 25), "missing-01.png", "Image", None, None, false, Some("chrome"), Some("Old capture - Chrome"), false, None),
    (12, (14, 0), "pattern-04.png", "Image", None, None, false, Some("explorer"), Some("Documents"), false, None),
    (10, (10, 30), "pattern-03.png", "Image", Some("Imgur"), Some("pAt3rn3"), true, Some("chrome"), Some("Wide banner - Chrome"), false, None),
    (9, (16, 15), "notes-01.txt", "Text", None, None, false, Some("notepad"), Some("notes-01.txt - Notepad"), false, None),
    (6, (11, 45), "pattern-02.png", "Image", None, None, false, Some("code"), Some("history.rs - Visual Studio Code"), true, Some("portrait")),
    (5, (13, 5), "ocr-sample.png", "Image", None, None, false, Some("ShareX"), Some("ShareX"), false, Some("ocr")),
    (4, (9, 55), "pattern-01.png", "Image", Some("Imgur"), Some("pAt3rn1"), false, Some("chrome"), Some("Release planning - Chrome"), false, None),
    (3, (15, 20), "pattern-06.png", "Image", None, None, false, Some("explorer"), Some("Pictures"), false, None),
    (2, (10, 40), "pattern-05.png", "Image", Some("Custom image uploader"), Some("cu-0005"), false, Some("code"), Some("app.rs - Visual Studio Code"), false, None),
    (1, (18, 0), "pattern-04.png", "Image", None, None, false, Some("chrome"), Some("Design system - Chrome"), false, None),
    (1, (19, 30), "https://example.com/very/long/link", "URL", Some("bit.ly"), None, false, None, None, false, None),
    (0, (8, 30), "pattern-03.png", "Image", None, None, false, Some("code"), Some("main.rs - Visual Studio Code"), false, None),
    (0, (9, 10), "pattern-02.png", "Image", Some("Imgur"), Some("pAt3rn2"), false, Some("chrome"), Some("Release planning - Chrome"), false, None),
    (0, (9, 45), "pattern-01.png", "Image", None, None, false, Some("ShareX"), Some("ShareX"), true, None),
];

fn main() {
    let dir = std::env::args().nth(1).expect("usage: sharex_history_fixture <fixture dir>");
    let dir = Path::new(&dir);
    let files = dir.join("files");
    let db = dir.join("History.db");
    let _ = std::fs::remove_file(&db);
    let mut manager = HistoryManagerSqlite::open(&db).expect("open History.db");
    let today = DateTime::now().date();
    let mut items: Vec<HistoryItem> = ITEMS
        .iter()
        .map(|&(days, (h, m), file, ty, host, url_id, short, process, title, fav, tag)| {
            let date = today.add_days(-days).add_ticks((h * 60 + m) * TICKS_PER_MINUTE);
            let is_url = ty == "URL";
            let ext = Path::new(file).extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
            let url = if is_url { Some(file.to_string()) } else { url_id.map(|id| format!("https://i.example.com/{id}.{ext}")) };
            let mut tags = Tags::new();
            if let Some(t) = title {
                tags.set("WindowTitle", Some(t.to_string()));
            }
            if let Some(p) = process {
                tags.set("ProcessName", Some(p.to_string()));
            }
            if fav {
                tags.set("Favorite", None);
            }
            if let Some(t) = tag {
                tags.set("Tag", Some(t.to_string()));
            }
            HistoryItem {
                id: 0,
                file_name: Some(if is_url { "link".to_string() } else { file.to_string() }),
                file_path: (!is_url).then(|| files.join(file).to_string_lossy().replace('/', "\\")),
                date_time: date,
                r#type: Some(ty.to_string()),
                host: host.map(str::to_string),
                thumbnail_url: url_id.filter(|_| ty == "Image").map(|id| format!("https://i.example.com/{id}t.{ext}")),
                deletion_url: url_id.map(|id| format!("https://example.com/delete/{id}")),
                shortened_url: if short { url_id.map(|id| format!("https://sho.rt/{id}")) } else { None },
                url,
                tags: (!tags.is_empty()).then_some(tags),
            }
        })
        .collect();
    assert!(manager.append_history_items(&mut items), "append failed");
    println!("wrote {} items to {}", items.len(), db.display());
}
