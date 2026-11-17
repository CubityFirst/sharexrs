//! `ShareX.HelpersLib/Helpers/URLHelpers.cs` and the parts of .NET's
//! `HttpUtility` uploads rely on: query strings, URL encoding (ShareX's own
//! and `HttpUtility.UrlEncode`, which differ), prefixes and host names.

use std::sync::LazyLock;

/// `URLHelpers.URLCharacters`: `Helpers.Alphanumeric + "-._~"`.
const URL_CHARACTERS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-._~";
/// `URLHelpers.URLPrefixes`.
const URL_PREFIXES: [&str; 6] = ["http://", "https://", "ftp://", "ftps://", "file://", "//"];

/// `URLHelpers.URLEncode(text, isPath)`: every UTF-8 byte outside
/// `A-Za-z0-9-._~` (and `/` for a path) as `%XX`, upper case.
pub fn url_encode(text: &str, is_path: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        let c = b as char;
        if b < 0x80 && (URL_CHARACTERS.contains(c) || (is_path && c == '/')) {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

static IGNORE_EMOJI: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `HelpersOptions.URLEncodeIgnoreEmoji` (`ApplicationConfig.URLEncodeIgnoreEmoji`).
pub fn set_url_encode_ignore_emoji(value: bool) {
    IGNORE_EMOJI.store(value, std::sync::atomic::Ordering::Relaxed);
}

/// `URLHelpers.URLEncode(text, isPath, HelpersOptions.URLEncodeIgnoreEmoji)`.
pub fn url_encode_with_options(text: &str, is_path: bool) -> String {
    if IGNORE_EMOJI.load(std::sync::atomic::Ordering::Relaxed) { url_encode_ignore_emoji(text, is_path) } else { url_encode(text, is_path) }
}

/// `URLHelpers.URLEncodeIgnoreEmoji`: emoji (`Emoji.SearchEmoji`, the
/// longest known emoji at each position) kept, the rest encoded.
pub fn url_encode_ignore_emoji(text: &str, is_path: bool) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let emoji = sharex_helpers::emoji::EMOJIS.iter().filter(|e| rest.starts_with(**e)).max_by_key(|e| e.len());
        match emoji {
            Some(e) => {
                out.push_str(e);
                rest = &rest[e.len()..];
            }
            None => {
                out.push_str(&url_encode(&rest[..c.len_utf8()], is_path));
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// `URLHelpers.AddSlash(url, SlashType.Prefix)`.
pub fn add_slash_prefix(url: &str) -> String {
    format!("/{}", url.strip_prefix('/').unwrap_or(url))
}

/// .NET's `HttpUtility.UrlEncode`: UTF-8, space as `+`, `A-Za-z0-9` and
/// `-_.!*()` kept, everything else `%xx` in lower case.
pub fn http_utility_url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b' ' => out.push('+'),
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'*' | b'(' | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02x}")),
        }
    }
    out
}

/// .NET's `HttpUtility.UrlDecode`: `+` as space, `%XX` bytes as UTF-8 and
/// `%uXXXX` as a UTF-16 unit; anything malformed is kept as written.
pub fn http_utility_url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'+' {
            out.push(b' ');
        } else if b == b'%' && i + 6 <= bytes.len() && bytes[i + 1] == b'u' {
            let digits: Option<Vec<u8>> = bytes[i + 2..i + 6].iter().map(|&d| hex(d)).collect();
            if let Some(d) = digits {
                let unit = u16::from(d[0]) << 12 | u16::from(d[1]) << 8 | u16::from(d[2]) << 4 | u16::from(d[3]);
                let s = String::from_utf16_lossy(&[unit]);
                out.extend_from_slice(s.as_bytes());
                i += 6;
                continue;
            }
            out.push(b);
        } else if b == b'%' && i + 3 <= bytes.len() {
            match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                _ => out.push(b),
            }
        } else {
            out.push(b);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `URLHelpers.URLDecode(url, count)`.
pub fn url_decode(url: &str, count: usize) -> String {
    let mut url = url.to_owned();
    for _ in 0..count {
        let decoded = http_utility_url_decode(&url);
        if decoded == url {
            break;
        }
        url = decoded;
    }
    url
}

/// `URLHelpers.CreateQueryString(args, customEncoding)`: `key=value` pairs
/// joined with `&`; an empty value leaves just the key.
pub fn create_query_string(args: &[(String, String)], custom_encoding: bool) -> String {
    args.iter()
        .map(|(key, value)| {
            if value.is_empty() {
                key.clone()
            } else if custom_encoding {
                format!("{key}={}", url_encode(value, false))
            } else {
                format!("{key}={}", http_utility_url_encode(value))
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// `URLHelpers.CreateQueryString(url, args, customEncoding)`.
pub fn create_query_string_url(url: &str, args: &[(String, String)], custom_encoding: bool) -> String {
    let query = create_query_string(args, custom_encoding);
    if query.is_empty() {
        url.to_owned()
    } else if url.contains('?') {
        format!("{url}&{query}")
    } else {
        format!("{url}?{query}")
    }
}

/// `URLHelpers.CombineURL(url1, url2)`: one `/` between them.
pub fn combine_url(url1: &str, url2: &str) -> String {
    match (url1.is_empty(), url2.is_empty()) {
        (true, true) => String::new(),
        (true, false) => url2.to_owned(),
        (false, true) => url1.to_owned(),
        _ => {
            let a = url1.strip_suffix('/').unwrap_or(url1);
            let b = url2.strip_prefix('/').unwrap_or(url2);
            format!("{a}/{b}")
        }
    }
}

/// `URLHelpers.CombineURL(params string[] urls)`.
pub fn combine_urls(urls: &[&str]) -> String {
    let mut it = urls.iter();
    let Some(first) = it.next() else { return String::new() };
    it.fold((*first).to_owned(), |acc, u| combine_url(&acc, u))
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len() && text.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// `URLHelpers.HasPrefix`.
pub fn has_prefix(url: &str) -> bool {
    URL_PREFIXES.iter().any(|p| starts_with_ignore_case(url, p))
}

/// `URLHelpers.GetPrefix`.
pub fn get_prefix(url: &str) -> Option<&'static str> {
    URL_PREFIXES.iter().copied().find(|p| starts_with_ignore_case(url, p))
}

/// `URLHelpers.FixPrefix(url)`: `https://` in front of a URL without one.
pub fn fix_prefix(url: &str) -> String {
    fix_prefix_with(url, "https://")
}

/// `URLHelpers.FixPrefix(url, prefix)`.
pub fn fix_prefix_with(url: &str, prefix: &str) -> String {
    if !url.is_empty() && !has_prefix(url) { format!("{prefix}{url}") } else { url.to_owned() }
}

/// `URLHelpers.ForcePrefix(url, prefix)`.
pub fn force_prefix(url: &str, prefix: &str) -> String {
    if url.is_empty() { String::new() } else { format!("{prefix}{}", remove_prefixes(url)) }
}

/// `URLHelpers.RemovePrefixes`: the first matching prefix only.
pub fn remove_prefixes(url: &str) -> String {
    match get_prefix(url) {
        Some(p) => url[p.len()..].to_owned(),
        None => url.to_owned(),
    }
}

/// The host of an absolute URI as .NET's `Uri.Host` gives it (lower case,
/// without user info and port), or `None` when `Uri.TryCreate(url,
/// UriKind.Absolute)` would fail.
pub fn uri_host(url: &str) -> Option<String> {
    let url = url.trim();
    let colon = url.find(':')?;
    let scheme = &url[..colon];
    if scheme.is_empty()
        || !scheme.as_bytes()[0].is_ascii_alphabetic()
        || !scheme.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
    {
        return None;
    }
    let rest = &url[colon + 1..];
    let rest = rest.strip_prefix("//")?;
    let end = rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let host_port = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        let close = host_port.find(']')?;
        &host_port[..=close]
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    if host.is_empty() && !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    Some(host.to_lowercase())
}

/// `URLHelpers.GetHostName`: the host without `www.`, or the text itself
/// when it isn't an absolute URL.
pub fn get_host_name(url: &str) -> String {
    if !url.is_empty()
        && let Some(host) = uri_host(url)
        && !host.is_empty()
    {
        return if starts_with_ignore_case(&host, "www.") { host[4..].to_owned() } else { host };
    }
    url.to_owned()
}

/// `URLHelpers.GetFileName`: what follows the last `/`, before `?` and `#`.
pub fn get_file_name(path: &str) -> String {
    let mut path = match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    };
    if let Some(i) = path.find('?') {
        path = &path[..i];
    }
    if let Some(i) = path.find('#') {
        path = &path[..i];
    }
    path.to_owned()
}

/// `URLHelpers.RemoveQueryString`.
pub fn remove_query_string(url: &str) -> String {
    match url.find('?') {
        Some(i) => url[..i].to_owned(),
        None => url.to_owned(),
    }
}

/// .NET's `HttpUtility.ParseQueryString` into a `NameValueCollection`:
/// keys in first-seen order with all their values; a pair without `=` has
/// a `None` key.
pub fn parse_query(query: &str) -> Vec<(Option<String>, Vec<String>)> {
    let mut out: Vec<(Option<String>, Vec<String>)> = Vec::new();
    let query = query.strip_prefix('?').unwrap_or(query);
    for part in query.split('&') {
        let (key, value) = match part.find('=') {
            Some(i) => (Some(http_utility_url_decode(&part[..i])), http_utility_url_decode(&part[i + 1..])),
            None => (None, http_utility_url_decode(part)),
        };
        // ParseQueryString skips an empty trailing segment.
        if key.is_none() && value.is_empty() && part.is_empty() {
            continue;
        }
        match out.iter_mut().find(|(k, _)| match (k, &key) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            (None, None) => true,
            _ => false,
        }) {
            Some((_, values)) => values.push(value),
            None => out.push((key, vec![value])),
        }
    }
    out
}

/// `URLHelpers.ParseQueryString(url)`: the query of `url`, if it has one.
pub fn parse_query_string(url: &str) -> Option<Vec<(Option<String>, Vec<String>)>> {
    let index = url.find('?')?;
    if index + 1 >= url.len() {
        return None;
    }
    Some(parse_query(&url[index + 1..]))
}

/// `URLHelpers.JSONEncode`: the text as a JSON string, without its quotes.
pub fn json_encode(text: &str) -> String {
    let quoted = crate::json::to_string(&serde_json::Value::String(text.to_owned()));
    quoted[1..quoted.len() - 1].to_owned()
}

/// `URLHelpers.XMLEncode` (`SecurityElement.Escape`).
pub fn xml_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '&' => out.push_str("&amp;"),
            c => out.push(c),
        }
    }
    out
}

static VALID_URL: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
    // Source: https://gist.github.com/729294 (as in ShareX).
    let pattern = concat!(
        "(?i)^",
        "(?:(?:https?|ftp)://)",
        "(?:\\S+(?::\\S*)?@)?",
        "(?:",
        "(?!(?:10|127)(?:\\.\\d{1,3}){3})",
        "(?!(?:169\\.254|192\\.168)(?:\\.\\d{1,3}){2})",
        "(?!172\\.(?:1[6-9]|2\\d|3[0-1])(?:\\.\\d{1,3}){2})",
        "(?:[1-9]\\d?|1\\d\\d|2[01]\\d|22[0-3])",
        "(?:\\.(?:1?\\d{1,2}|2[0-4]\\d|25[0-5])){2}",
        "(?:\\.(?:[1-9]\\d?|1\\d\\d|2[0-4]\\d|25[0-4]))",
        "|",
        "(?:(?:[a-z\\x{00a1}-\\x{ffff}0-9]-*)*[a-z\\x{00a1}-\\x{ffff}0-9]+)",
        "(?:\\.(?:[a-z\\x{00a1}-\\x{ffff}0-9]-*)*[a-z\\x{00a1}-\\x{ffff}0-9]+)*",
        "(?:\\.(?:[a-z\\x{00a1}-\\x{ffff}]{2,}))",
        "\\.?",
        ")",
        "(?::\\d{2,5})?",
        "(?:[/?#]\\S*)?",
        "$"
    );
    fancy_regex::Regex::new(pattern).expect("valid URL pattern")
});

/// `URLHelpers.IsValidURL(url)` (with its regex).
pub fn is_valid_url(url: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    VALID_URL.is_match(url.trim()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_differ_as_in_dotnet() {
        assert_eq!(url_encode("a b/é~", false), "a%20b%2F%C3%A9~");
        assert_eq!(url_encode("a b/c", true), "a%20b/c");
        assert_eq!(http_utility_url_encode("a b/é~!*()'"), "a+b%2f%c3%a9%7e!*()%27");
        assert_eq!(http_utility_url_decode("a+b%2f%C3%A9%u0041%zz"), "a b/éA%zz");
    }

    #[test]
    fn query_strings() {
        let args = vec![("a".to_owned(), "1 2".to_owned()), ("b".to_owned(), String::new())];
        assert_eq!(create_query_string_url("http://x/y", &args, false), "http://x/y?a=1+2&b");
        assert_eq!(create_query_string_url("http://x/y?z=1", &args, true), "http://x/y?z=1&a=1%202&b");
        let parsed = parse_query_string("http://x/?a=1&flag&a=2&b=%20").unwrap();
        assert_eq!(
            parsed,
            vec![
                (Some("a".into()), vec!["1".into(), "2".into()]),
                (None, vec!["flag".into()]),
                (Some("b".into()), vec![" ".into()])
            ]
        );
    }

    #[test]
    fn prefixes_and_hosts() {
        assert_eq!(fix_prefix("example.com"), "https://example.com");
        assert_eq!(fix_prefix("http://example.com"), "http://example.com");
        assert_eq!(force_prefix("http://a.com", "https://"), "https://a.com");
        assert_eq!(get_host_name("https://www.Example.com:8080/x"), "example.com");
        assert_eq!(get_host_name("example.com/upload"), "example.com/upload");
        assert_eq!(combine_url("https://a/", "/b"), "https://a/b");
        assert_eq!(get_file_name("https://a/b/c.png?x#y"), "c.png");
    }

    #[test]
    fn valid_urls() {
        assert!(is_valid_url("https://pastebin.com/abc"));
        assert!(!is_valid_url("Bad API request, invalid api_dev_key"));
        assert!(!is_valid_url("http://127.0.0.1/x"));
    }

    #[test]
    fn escapes() {
        assert_eq!(xml_encode("<a & 'b'>"), "&lt;a &amp; &apos;b&apos;&gt;");
        assert_eq!(json_encode("a\"b\\c\n\u{1}\u{2028}é"), "a\\\"b\\\\c\\n\\u0001\\u2028é");
    }
}
