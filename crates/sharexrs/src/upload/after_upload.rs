//! `WorkerTask.DoAfterUploadJobs` (URL regex replace, force HTTPS, shorten
//! URL, share URL, copy URL with `ClipboardContentFormat`, open URL, QR
//! code), `WorkerTask.ShortenURL`/`ShareURL`, and `ShareX/UploadInfoParser.cs`.

use std::path::Path;
use std::time::Duration;

use sharex_helpers::{NameParser, NameParserType};
use sharex_settings::AfterUploadTasks;
use sharex_uploaders::UploadResult;
use sharex_uploaders::factory::{self, Ported};
use sharex_uploaders::uploader::{share_url, shorten_url};

use super::worker::{Worker, not_ported_result};
use crate::tasks::TaskJob;

/// What `UploadInfoParser.Parse` reads of a task (`TaskInfo`).
pub struct InfoFields<'a> {
    pub result: &'a UploadResult,
    pub file_name: &'a str,
    pub file_path: Option<&'a Path>,
    pub thumbnail_file_path: Option<&'a Path>,
    /// `UploadDuration`.
    pub upload_duration: Option<Duration>,
}

/// `UploadInfoParser.Parse(info, pattern)`: the name parser's variables,
/// then the task's (`$result`, `$url`, `$shorturl`, `$thumbnailurl`,
/// `$deletionurl`, `$filenamenoext`, `$filename`, `$filepath`,
/// `$folderpath`, `$foldername`, `$thumbnailfilenamenoext`,
/// `$thumbnailfilename`, `$uploadtime`).
pub fn parse_upload_info(info: &InfoFields, pattern: &str) -> String {
    if pattern.is_empty() {
        return pattern.to_owned();
    }
    let mut pattern = NameParser::new(NameParserType::Default).parse(pattern);
    let s = |p: Option<&Path>| p.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let r = info.result;
    let mut result = r.to_string();
    if result.is_empty() && info.file_path.is_some() {
        result = s(info.file_path);
    }
    let v = |o: &Option<String>| o.clone().unwrap_or_default();
    pattern = pattern.replace("$result", &result);
    pattern = pattern.replace("$url", &v(&r.url));
    pattern = pattern.replace("$shorturl", &v(&r.shortened_url));
    pattern = pattern.replace("$thumbnailurl", &v(&r.thumbnail_url));
    pattern = pattern.replace("$deletionurl", &v(&r.deletion_url));
    let stem = |p: &Path| p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = info.file_path.and_then(Path::parent);
    pattern = pattern.replace("$filenamenoext", &if info.file_name.is_empty() { String::new() } else { stem(Path::new(info.file_name)) });
    pattern = pattern.replace("$filename", info.file_name);
    pattern = pattern.replace("$filepath", &s(info.file_path));
    pattern = pattern.replace("$folderpath", &s(folder));
    pattern = pattern.replace("$foldername", &folder.map(name).unwrap_or_default());
    pattern = pattern.replace("$thumbnailfilenamenoext", &info.thumbnail_file_path.map(stem).unwrap_or_default());
    pattern = pattern.replace("$thumbnailfilename", &info.thumbnail_file_path.map(name).unwrap_or_default());
    if let Some(d) = info.upload_duration {
        pattern = pattern.replace("$uploadtime", &d.as_millis().to_string());
    }
    pattern
}

/// `Regex.Replace(input, pattern, replacement)` with .NET's substitutions:
/// `$$`, `$&`, `$0`..`$n` (all the digits, when that's a group; else
/// literal), `${name}`, `` $` ``, `$'`, `$+`, `$_`, with groups numbered
/// as .NET numbers them (unnamed groups first, then named ones). An
/// invalid pattern is .NET's `ArgumentException` (its message).
pub fn dotnet_regex_replace(input: &str, pattern: &str, replacement: &str) -> Result<String, String> {
    let re = fancy_regex::Regex::new(pattern).map_err(|e| format!("Invalid pattern '{pattern}': {e}"))?;
    let names: Vec<Option<&str>> = re.capture_names().collect();
    // .NET group number -> this regex's group index.
    let unnamed = (1..names.len()).filter(|&i| names[i].is_none());
    let named = (1..names.len()).filter(|&i| names[i].is_some());
    let numbering: Vec<usize> = std::iter::once(0).chain(unnamed).chain(named).collect();
    let mut out = String::new();
    let mut last = 0;
    for caps in re.captures_iter(input) {
        let caps = caps.map_err(|e| e.to_string())?;
        let m = caps.get(0).expect("group 0");
        out.push_str(&input[last..m.start()]);
        expand(&caps, &names, &numbering, input, replacement, &mut out);
        last = m.end();
    }
    out.push_str(&input[last..]);
    Ok(out)
}

fn expand(caps: &fancy_regex::Captures, names: &[Option<&str>], numbering: &[usize], input: &str, replacement: &str, out: &mut String) {
    let group = |i: usize| caps.get(i).map(|m| m.as_str()).unwrap_or("");
    let whole = caps.get(0).expect("group 0");
    let bytes = replacement.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c != b'$' || i + 1 >= bytes.len() {
            let ch = replacement[i..].chars().next().expect("char");
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let next = bytes[i + 1];
        match next {
            b'$' => {
                out.push('$');
                i += 2;
            }
            b'&' => {
                out.push_str(whole.as_str());
                i += 2;
            }
            b'`' => {
                out.push_str(&input[..whole.start()]);
                i += 2;
            }
            b'\'' => {
                out.push_str(&input[whole.end()..]);
                i += 2;
            }
            b'+' => {
                // `LastGroup`: the highest numbered group.
                out.push_str(if numbering.len() > 1 { group(numbering[numbering.len() - 1]) } else { "" });
                i += 2;
            }
            b'_' => {
                out.push_str(input);
                i += 2;
            }
            b'0'..=b'9' => {
                let end = bytes[i + 1..].iter().position(|b| !b.is_ascii_digit()).map_or(bytes.len(), |p| i + 1 + p);
                match replacement[i + 1..end].parse::<usize>() {
                    Ok(n) if n < numbering.len() => {
                        out.push_str(group(numbering[n]));
                        i = end;
                    }
                    _ => {
                        out.push('$');
                        i += 1;
                    }
                }
            }
            b'{' => {
                let close = replacement[i + 2..].find('}').map(|p| i + 2 + p);
                let name = close.map(|c| &replacement[i + 2..c]);
                let index = name.and_then(|n| {
                    if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) {
                        n.parse::<usize>().ok().and_then(|g| numbering.get(g).copied())
                    } else {
                        names.iter().position(|x| *x == Some(n))
                    }
                });
                match (index, close) {
                    (Some(g), Some(c)) => {
                        out.push_str(group(g));
                        i = c + 1;
                    }
                    _ => {
                        out.push('$');
                        i += 1;
                    }
                }
            }
            _ => {
                out.push('$');
                i += 1;
            }
        }
    }
}

impl Worker<'_> {
    pub(super) fn info_fields(&self) -> InfoFields<'_> {
        InfoFields {
            result: &self.result,
            file_name: &self.file_name,
            file_path: self.file_path.as_deref(),
            thumbnail_file_path: None,
            upload_duration: self.upload_duration,
        }
    }

    /// `DoAfterUploadJobs`. An exception stops the rest and becomes an error.
    pub(super) fn do_after_upload_jobs(&mut self) {
        if let Err(e) = self.after_upload_jobs() {
            eprintln!("After upload jobs: {e}");
            self.result.errors.add(e);
        }
    }

    fn after_upload_jobs(&mut self) -> Result<(), String> {
        let upload = self.task.UploadSettings.clone().unwrap_or_default();
        let advanced = self.task.AdvancedSettings.clone().unwrap_or_default();
        let jobs = self.task.AfterUploadJob;
        let url = self.result.url.clone().unwrap_or_default();
        if upload.URLRegexReplace {
            self.result.url = Some(dotnet_regex_replace(&url, &upload.URLRegexReplacePattern, &upload.URLRegexReplaceReplacement)?);
        }
        if advanced.ResultForceHTTPS {
            self.result.force_https();
        }
        let url = self.result.url.clone().unwrap_or_default();
        // `URL.Length`: UTF-16 code units.
        let auto_shorten = advanced.AutoShortenURLLength > 0 && url.encode_utf16().count() > advanced.AutoShortenURLLength as usize;
        if self.job != TaskJob::ShareURL
            && (jobs.contains(AfterUploadTasks::UseURLShortener) || self.job == TaskJob::ShortenURL || auto_shorten)
            && let Some(result) = self.shorten_url(&url)?
        {
            self.result.shortened_url = result.shortened_url;
            self.result.errors.extend(&result.errors);
        }
        if self.job != TaskJob::ShortenURL && (jobs.contains(AfterUploadTasks::ShareURL) || self.job == TaskJob::ShareURL) {
            if let Some(result) = self.share_url(&self.result.to_string())? {
                self.result.errors.extend(&result.errors);
            }
            if self.job == TaskJob::ShareURL {
                self.result.is_url_expected = false;
            }
        }
        if jobs.contains(AfterUploadTasks::CopyURLToClipboard) {
            let txt = if advanced.ClipboardContentFormat.is_empty() {
                self.result.to_string()
            } else {
                parse_upload_info(&self.info_fields(), &advanced.ClipboardContentFormat)
            };
            if !txt.is_empty() {
                self.ctx.ui.copy_text(&txt);
            }
        }
        if jobs.contains(AfterUploadTasks::OpenURL) {
            let result = if advanced.OpenURLFormat.is_empty() {
                self.result.to_string()
            } else {
                parse_upload_info(&self.info_fields(), &advanced.OpenURLFormat)
            };
            self.ctx.ui.open_url(&result);
        }
        if jobs.contains(AfterUploadTasks::ShowQRCode) {
            self.ctx.ui.show_qr_code(&self.result.to_string());
        }
        Ok(())
    }

    /// `WorkerTask.ShortenURL`: `Ok(None)` where ShareX gets `null`; an
    /// exception is `Err`.
    fn shorten_url(&self, url: &str) -> Result<Option<UploadResult>, String> {
        let dest = self.task.URLShortenerDestination.clone();
        let name = factory::service_name("UrlShortenerType", dest.name());
        let mut shortener = {
            let mut guard = self.ctx.config.wait();
            let config = guard.as_mut().expect("loaded");
            match factory::check_url_shortener(&dest, config) {
                Ported::Yes(true) => {}
                Ported::Yes(false) => return Ok(Some(self.invalid_config_result(&name))),
                Ported::No => return Ok(Some(not_ported_result(&name))),
            }
            match factory::create_url_shortener(&dest, config, &self.task_ref) {
                Some(s) => s,
                None => return Ok(None),
            }
        };
        let result = shorten_url(shortener.as_mut(), url);
        self.ctx.config.with(|config| shortener.write_back(config));
        result
    }

    /// `WorkerTask.ShareURL`.
    fn share_url(&self, url: &str) -> Result<Option<UploadResult>, String> {
        if url.is_empty() {
            return Ok(None);
        }
        let dest = self.task.URLSharingServiceDestination.clone();
        let name = factory::service_name("URLSharingServices", dest.name());
        let mut sharer = {
            let mut guard = self.ctx.config.wait();
            let config = guard.as_mut().expect("loaded");
            match factory::check_url_sharing(&dest, config) {
                Ported::Yes(true) => {}
                Ported::Yes(false) => return Ok(Some(self.invalid_config_result(&name))),
                Ported::No => return Ok(Some(not_ported_result(&name))),
            }
            match factory::create_url_sharing(&dest, config, &self.task_ref) {
                Some(s) => s,
                None => return Ok(None),
            }
        };
        let result = share_url(sharer.as_mut(), url);
        self.ctx.config.with(|config| sharer.write_back(config));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotnet_substitutions() {
        let r = |i, p, rep| dotnet_regex_replace(i, p, rep).unwrap();
        assert_eq!(r("http://a.com/x", "^https?://(.+)$", "https://$1"), "https://a.com/x");
        assert_eq!(r("abc", "(b)", "[$1a]"), "a[ba]c");
        assert_eq!(r("abc", "(b)", "[$12]"), "a[$12]c");
        assert_eq!(r("abc", "(?<n>b)", "[${n}|$0|$&|$$|$`|$'|$+|$_]"), "a[b|b|b|$|a|c|b|abc]c");
        assert_eq!(r("abc", "(b)", "[${x}|$x|$]"), "a[${x}|$x|$]c");
        assert_eq!(r("aaa", "a", "b"), "bbb");
        // .NET numbers the unnamed group first.
        assert_eq!(r("ab", "(?<first>a)(b)", "$1$2|${first}|$+"), "ba|a|a");
        assert_eq!(r("abc", "x*", "-"), "-a-b-c-");
    }

    /// `fixtures/after_upload.sharex.json` is what ShareX's own
    /// `UploadInfoParser.Parse` and `Regex.Replace` made of
    /// `tools/uploaders-harness/after_upload_cases.json`
    /// (`UploadersHarness --after-upload`, ShareX.dll of the reference build).
    #[test]
    fn after_upload_results_match_sharex() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("fixtures/after_upload.sharex.json")).unwrap();
        let s = |v: &serde_json::Value| v.as_str().map(str::to_owned);
        let mut compared = 0;
        for c in fixture["parse"].as_array().unwrap() {
            if c["windows_only"].as_bool() == Some(true) && !cfg!(windows) {
                continue;
            }
            let result = UploadResult {
                url: s(&c["url"]),
                thumbnail_url: s(&c["thumbnail_url"]),
                deletion_url: s(&c["deletion_url"]),
                shortened_url: s(&c["shortened_url"]),
                ..Default::default()
            };
            let file_path = s(&c["file_path"]).map(std::path::PathBuf::from);
            // `FilePath`'s setter sets `FileName`; a given one comes after.
            let file_name = s(&c["file_name"]).unwrap_or_else(|| {
                file_path.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            });
            let info = InfoFields { result: &result, file_name: &file_name, file_path: file_path.as_deref(), thumbnail_file_path: None, upload_duration: None };
            assert_eq!(parse_upload_info(&info, c["pattern"].as_str().unwrap()), c["output"].as_str().unwrap(), "{}", c["id"]);
            compared += 1;
        }
        for c in fixture["regex"].as_array().unwrap() {
            let out = dotnet_regex_replace(c["input"].as_str().unwrap(), c["pattern"].as_str().unwrap(), c["replacement"].as_str().unwrap());
            assert_eq!(out.unwrap(), c["output"].as_str().unwrap(), "{}", c["id"]);
            compared += 1;
        }
        assert!(compared >= if cfg!(windows) { 33 } else { 31 });
    }

    #[test]
    fn upload_info_variables() {
        let result = UploadResult {
            url: Some("https://i.example.com/a.png".into()),
            thumbnail_url: Some("https://i.example.com/t.png".into()),
            ..Default::default()
        };
        let path = Path::new("dir").join("sub").join("shot.png");
        let info = InfoFields {
            result: &result,
            file_name: "shot.png",
            file_path: Some(&path),
            thumbnail_file_path: None,
            upload_duration: Some(Duration::from_millis(42)),
        };
        assert_eq!(parse_upload_info(&info, "$result"), "https://i.example.com/a.png");
        assert_eq!(
            parse_upload_info(&info, "[url=$url][img]$thumbnailurl[/img][/url] $filenamenoext $foldername $uploadtime"),
            "[url=https://i.example.com/a.png][img]https://i.example.com/t.png[/img][/url] shot sub 42"
        );
    }
}
