//! `CustomUploader/ShareXSyntaxParser.cs`, `ShareXCustomUploaderSyntaxParser.cs`,
//! `Functions/*` and `Helpers/EscapeHelper.cs`: the `{function:parameter|...}`
//! syntax of custom uploaders.
//!
//! `{` starts a function, `}` ends it, `:` starts its parameters, `|`
//! separates them, `\` escapes the next character. Functions:
//!
//! | Function | Example | Gives |
//! |---|---|---|
//! | `response` | `{response}` | the response text |
//! | `responseurl` | `{responseurl}` | the URL the response came from |
//! | `header` | `{header:Location}` | a response header |
//! | `json` | `{json:files[0].url}`, `{json:{response}\|files[0].url}` | a JSONPath value |
//! | `xml` | `{xml:/files/file[1]/url}`, `{xml:input\|xpath}` | an XPath value |
//! | `regex` | `{regex:href="(.+)"\|1}`, `{regex:input\|pattern\|group}` | a match or group |
//! | `filename` | `{filename}` | the file name (URL encoded in URLs) |
//! | `input` | `{input}` | the text or URL being sent (URL encoded in URLs) |
//! | `random` | `{random:a\|b}` | one of its parameters |
//! | `select` | `{select:a\|b}` | one the user picks |
//! | `inputbox` / `prompt` | `{inputbox:title\|default}` | text the user types |
//! | `outputbox` | `{outputbox:title\|text}` | nothing; shows text |
//! | `base64` | `{base64:user:pass}` | the parameter in base 64 |

use std::sync::{Arc, LazyLock, RwLock};

use sharex_helpers::{NameParser, NameParserType, random};
use sharex_strings::uploaders_lib as strings;

use crate::http::ResponseInfo;
use crate::url_helpers;
use crate::uploader::text_to_base64;

/// The dialogs `{inputbox}`, `{outputbox}` and `{select}` open
/// (`InputBoxWindowIntegration`, `OutputBoxWindowIntegration`,
/// `ParserSelectWindowIntegration`). Without one (headless), `{inputbox}`
/// and `{select}` give nothing and `{outputbox}` shows nothing.
pub trait ParserUi: Send + Sync {
    /// The text typed, `None` when cancelled.
    fn input_box(&self, title: &str, default_text: &str) -> Option<String>;
    fn output_box(&self, text: &str, title: &str);
    /// The value picked, `None` when cancelled.
    fn select(&self, values: &[String]) -> Option<String>;
}

static UI: LazyLock<RwLock<Option<Arc<dyn ParserUi>>>> = LazyLock::new(|| RwLock::new(None));

/// Sets the dialogs the parser opens.
pub fn set_parser_ui(ui: Option<Arc<dyn ParserUi>>) {
    *UI.write().expect("parser ui") = ui;
}

fn ui() -> Option<Arc<dyn ParserUi>> {
    UI.read().expect("parser ui").clone()
}

/// `EscapeHelper` with `KeepEscapeCharacter`: runs `action` (the name
/// parser) on `input` with `\\` and `\%` hidden from it.
pub fn escape_helper_parse(input: &str, keep_escape_character: bool, action: impl FnOnce(&str) -> String) -> String {
    let escape_reserve = random::string(random::ALPHANUMERIC, 32);
    let escapeable_reserve = random::string(random::ALPHANUMERIC, 32);
    let input = input.replace("\\\\", &escape_reserve).replace("\\%", &escapeable_reserve);
    let output = action(&input);
    if keep_escape_character {
        output.replace(&escapeable_reserve, "\\%").replace(&escape_reserve, "\\\\")
    } else {
        output.replace(&escapeable_reserve, "%").replace(&escape_reserve, "\\")
    }
}

/// `ShareXCustomUploaderSyntaxParser`.
#[derive(Clone, Default)]
pub struct CustomUploaderParser {
    pub file_name: Option<String>,
    pub input: Option<String>,
    pub response_info: Option<ResponseInfo>,
    /// `URLEncode`: `{filename}` and `{input}` URL encoded.
    pub url_encode: bool,
    /// `UseNameParser`: name pattern tokens (`%y`, `%ra{5}`...) first.
    pub use_name_parser: bool,
    /// `AllowNameParserFileRead`.
    pub allow_name_parser_file_read: bool,
}

const SYNTAX_START: char = '{';
const SYNTAX_END: char = '}';
const SYNTAX_PARAMETER_START: char = ':';
const SYNTAX_PARAMETER_DELIMITER: char = '|';
const SYNTAX_ESCAPE: char = '\\';

impl CustomUploaderParser {
    /// `new ShareXCustomUploaderSyntaxParser(input)`.
    pub fn with_input(file_name: &str, input: &str) -> Self {
        Self {
            file_name: Some(file_name.to_owned()),
            input: Some(input.to_owned()),
            allow_name_parser_file_read: true,
            ..Default::default()
        }
    }

    /// `Parse(text)`: an error is the exception ShareX throws (bad function
    /// name, too few parameters, invalid JSON, ...).
    pub fn parse(&self, text: &str) -> Result<String, String> {
        let mut text = text.to_owned();
        if self.use_name_parser && !text.is_empty() {
            let allow = self.allow_name_parser_file_read;
            text = escape_helper_parse(&text, true, |t| {
                let mut np = NameParser::new(NameParserType::Text);
                np.allow_file_read = allow;
                np.parse(t)
            });
        }
        if text.is_empty() {
            return Ok(String::new());
        }
        let chars: Vec<char> = text.chars().collect();
        let mut end = 0;
        self.parse_at(&chars, false, 0, &mut end).map(|s| s.unwrap_or_default())
    }

    /// `ShareXSyntaxParser.Parse(text, isFunction, startPosition, out endPosition)`.
    fn parse_at(&self, text: &[char], is_function: bool, start: usize, end_position: &mut usize) -> Result<Option<String>, String> {
        let mut out = String::new();
        let mut escape = false;
        let mut i = start;
        while i < text.len() {
            let c = text[i];
            if !escape {
                if c == SYNTAX_START {
                    let mut end = 0;
                    let parsed = self.parse_at(text, true, i + 1, &mut end)?;
                    out.push_str(parsed.as_deref().unwrap_or(""));
                    i = end + 1;
                    continue;
                } else if c == SYNTAX_END || c == SYNTAX_PARAMETER_DELIMITER {
                    break;
                } else if c == SYNTAX_ESCAPE {
                    escape = true;
                    i += 1;
                    continue;
                } else if is_function && c == SYNTAX_PARAMETER_START {
                    let mut parameters = Vec::new();
                    loop {
                        let mut end = 0;
                        let parsed = self.parse_at(text, false, i + 1, &mut end)?;
                        parameters.push(parsed.unwrap_or_default());
                        i = end;
                        if !(i < text.len() && text[i] == SYNTAX_PARAMETER_DELIMITER) {
                            break;
                        }
                    }
                    *end_position = i;
                    return self.call_function(&out, Some(&parameters));
                }
            }
            escape = false;
            out.push(c);
            i += 1;
        }
        *end_position = i;
        if is_function {
            return self.call_function(&out, None);
        }
        Ok(Some(out))
    }

    fn response_text(&self) -> Option<String> {
        self.response_info.as_ref().and_then(|r| r.response_text.clone())
    }

    fn response(&self) -> Result<&ResponseInfo, String> {
        self.response_info.as_ref().ok_or_else(|| "Object reference not set to an instance of an object.".to_owned())
    }

    /// `CallFunction`.
    fn call_function(&self, name: &str, parameters: Option<&[String]>) -> Result<Option<String>, String> {
        if name.is_empty() {
            return Err(strings::CustomUploaderParser_Function_name_cannot_be_empty().to_owned());
        }
        let function = FUNCTIONS
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name) || f.aliases.iter().any(|a| a.eq_ignore_ascii_case(name)))
            .ok_or_else(|| strings::CustomUploaderParser_Invalid_function_name().replace("{0}", name))?;
        if function.min_parameter_count > 0 && parameters.is_none_or(|p| p.len() < function.min_parameter_count) {
            return Err(strings::CustomUploaderParser_Minimum_parameter_count()
                .replace("{0}", function.name)
                .replace("{1}", &function.min_parameter_count.to_string()));
        }
        (function.call)(self, parameters.unwrap_or(&[]))
    }
}

struct Function {
    name: &'static str,
    aliases: &'static [&'static str],
    min_parameter_count: usize,
    call: fn(&CustomUploaderParser, &[String]) -> Result<Option<String>, String>,
}

/// `Helpers.GetInstances<CustomUploaderFunction>()`, in ShareX's (file
/// name) order.
static FUNCTIONS: &[Function] = &[
    Function { name: "base64", aliases: &[], min_parameter_count: 1, call: fn_base64 },
    Function { name: "filename", aliases: &[], min_parameter_count: 0, call: fn_filename },
    Function { name: "header", aliases: &[], min_parameter_count: 1, call: fn_header },
    Function { name: "input", aliases: &[], min_parameter_count: 0, call: fn_input },
    Function { name: "inputbox", aliases: &["prompt"], min_parameter_count: 0, call: fn_inputbox },
    Function { name: "json", aliases: &[], min_parameter_count: 1, call: fn_json },
    Function { name: "outputbox", aliases: &[], min_parameter_count: 1, call: fn_outputbox },
    Function { name: "random", aliases: &[], min_parameter_count: 2, call: fn_random },
    Function { name: "regex", aliases: &[], min_parameter_count: 1, call: fn_regex },
    Function { name: "response", aliases: &[], min_parameter_count: 0, call: fn_response },
    Function { name: "responseurl", aliases: &[], min_parameter_count: 0, call: fn_responseurl },
    Function { name: "select", aliases: &[], min_parameter_count: 1, call: fn_select },
    Function { name: "xml", aliases: &[], min_parameter_count: 1, call: fn_xml },
];

/// `{base64:text}`.
fn fn_base64(_: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    Ok((!p[0].is_empty()).then(|| text_to_base64(&p[0])))
}

/// `{filename}`.
fn fn_filename(parser: &CustomUploaderParser, _: &[String]) -> Result<Option<String>, String> {
    Ok(if parser.url_encode { Some(url_helpers::url_encode(parser.file_name.as_deref().unwrap_or(""), false)) } else { parser.file_name.clone() })
}

/// `{header:name}`.
fn fn_header(parser: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    Ok(parser.response()?.header(&p[0]).map(str::to_owned))
}

/// `{input}`.
fn fn_input(parser: &CustomUploaderParser, _: &[String]) -> Result<Option<String>, String> {
    Ok(if parser.url_encode { Some(url_helpers::url_encode(parser.input.as_deref().unwrap_or(""), false)) } else { parser.input.clone() })
}

/// `{inputbox}`, `{inputbox:title}`, `{inputbox:title|default text}`.
fn fn_inputbox(_: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    let title = p.first().map(String::as_str).unwrap_or("Input");
    let default_text = p.get(1).map(String::as_str).unwrap_or("");
    Ok(ui().and_then(|ui| ui.input_box(title, default_text)))
}

/// `{json:path}`, `{json:input|path}`.
fn fn_json(parser: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    let (input, path) = if p.len() > 1 { (Some(p[0].clone()), p[1].clone()) } else { (parser.response()?.response_text.clone(), p[0].clone()) };
    let Some(input) = input.filter(|i| !i.is_empty()) else { return Ok(None) };
    if path.is_empty() {
        return Ok(None);
    }
    let path = if path.starts_with("$.") { path } else { format!("$.{path}") };
    let root = crate::json::parse(&input).map_err(|e| format!("{JSON_READER_EXCEPTION}{e}"))?;
    match crate::json::select_token(&root, &path)? {
        Some(token) => crate::json::to_dotnet_string(token),
        None => Ok(None),
    }
}

/// Marks a JSON parse error, which `TryParseResponse` reports as invalid
/// JSON (`JsonReaderException`) rather than an unparsable response.
pub(crate) const JSON_READER_EXCEPTION: &str = "JsonReaderException: ";

/// `{outputbox:text}`, `{outputbox:title|text}`.
fn fn_outputbox(_: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    let (title, text) = if p.len() > 1 { (p[0].as_str(), p[1].as_str()) } else { ("", p[0].as_str()) };
    if !text.is_empty() {
        let title = if title.is_empty() { "Output" } else { title };
        if let Some(ui) = ui() {
            ui.output_box(text, title);
        }
    }
    Ok(None)
}

/// `{random:a|b|...}`.
fn fn_random(_: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    Ok(random::pick(p).cloned())
}

/// `{regex:pattern}`, `{regex:pattern|group}`, `{regex:input|pattern|group}`.
fn fn_regex(parser: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    let (input, pattern, group) = if p.len() > 2 {
        (Some(p[0].clone()), p[1].clone(), p[2].clone())
    } else {
        (parser.response()?.response_text.clone(), p[0].clone(), p.get(1).cloned().unwrap_or_default())
    };
    let Some(input) = input.filter(|i| !i.is_empty()) else { return Ok(None) };
    if pattern.is_empty() {
        return Ok(None);
    }
    let re = fancy_regex::Regex::new(&dotnet_pattern(&pattern)).map_err(|e| format!("Invalid pattern '{pattern}': {e}"))?;
    let Some(caps) = re.captures(&input).map_err(|e| e.to_string())? else { return Ok(None) };
    if !group.is_empty() {
        let value = match group.trim().parse::<i32>() {
            Ok(n) => usize::try_from(n).ok().and_then(|n| caps.get(n)),
            Err(_) => caps.name(&group),
        };
        return Ok(Some(value.map(|m| m.as_str().to_owned()).unwrap_or_default()));
    }
    Ok(Some(caps.get(0).map(|m| m.as_str().to_owned()).unwrap_or_default()))
}

/// A .NET pattern for fancy-regex: `(?<name>` is accepted by both; `.`
/// excludes only `\n` in .NET, `$` matches before a final `\n`; fancy-regex
/// (like Perl) agrees on both, so the pattern goes through as written.
fn dotnet_pattern(pattern: &str) -> String {
    pattern.to_owned()
}

/// `{response}`.
fn fn_response(parser: &CustomUploaderParser, _: &[String]) -> Result<Option<String>, String> {
    parser.response()?;
    Ok(parser.response_text())
}

/// `{responseurl}`.
fn fn_responseurl(parser: &CustomUploaderParser, _: &[String]) -> Result<Option<String>, String> {
    Ok(parser.response()?.response_url.clone())
}

/// `{select:a|b|...}`.
fn fn_select(_: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    let values: Vec<String> = p.iter().filter(|v| !v.is_empty()).cloned().collect();
    if values.is_empty() {
        return Ok(None);
    }
    Ok(ui().and_then(|ui| ui.select(&values)))
}

/// `{xml:xpath}`, `{xml:input|xpath}`: `XPathNavigator.SelectSingleNode(xpath).Value`.
fn fn_xml(parser: &CustomUploaderParser, p: &[String]) -> Result<Option<String>, String> {
    let (input, xpath) = if p.len() > 1 { (Some(p[0].clone()), p[1].clone()) } else { (parser.response()?.response_text.clone(), p[0].clone()) };
    let Some(input) = input.filter(|i| !i.is_empty()) else { return Ok(None) };
    if xpath.is_empty() {
        return Ok(None);
    }
    xpath_select_single(&input, &xpath)
}

/// The string value of the first node `xpath` selects in `xml`.
pub fn xpath_select_single(xml: &str, xpath: &str) -> Result<Option<String>, String> {
    use sxd_xpath::{Context, Factory, Value, nodeset::Node};
    let package = sxd_document::parser::parse(xml.trim_start_matches('\u{feff}')).map_err(|e| format!("{e:?}"))?;
    let document = package.as_document();
    let compiled = Factory::new().build(xpath).map_err(|e| format!("'{xpath}' has an invalid token. ({e})"))?;
    let Some(compiled) = compiled else { return Ok(None) };
    let value = compiled.evaluate(&Context::new(), document.root()).map_err(|e| format!("{e:?}"))?;
    match value {
        Value::Nodeset(nodes) => Ok(nodes.document_order_first().map(|n: Node| n.string_value())),
        // SelectSingleNode requires a node set.
        _ => Err("Expression must evaluate to a node-set.".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(text: &str) -> ResponseInfo {
        ResponseInfo {
            status_code: 200,
            status_description: "OK".into(),
            response_url: Some("https://example.com/upload".into()),
            headers: vec![("Location".into(), "https://example.com/x".into())],
            response_text: Some(text.into()),
            response_data: None,
        }
    }

    fn p(text: &str, resp: &str) -> Result<String, String> {
        let parser = CustomUploaderParser {
            file_name: Some("my file.png".into()),
            input: Some("a&b".into()),
            response_info: Some(response(resp)),
            url_encode: true,
            ..Default::default()
        };
        parser.parse(text)
    }

    #[test]
    fn syntax() {
        assert_eq!(p("plain", "").unwrap(), "plain");
        assert_eq!(p("a\\{b\\}c\\|d\\\\", "").unwrap(), "a{b}c|d\\");
        assert_eq!(p("https://x/{filename}?q={input}", "").unwrap(), "https://x/my%20file.png?q=a%26b");
        assert_eq!(p("{response}", "hello").unwrap(), "hello");
        assert_eq!(p("{RESPONSEURL}", "").unwrap(), "https://example.com/upload");
        assert_eq!(p("{header:location}", "").unwrap(), "https://example.com/x");
        assert_eq!(p("Basic {base64:user:pass}", "").unwrap(), "Basic dXNlcjpwYXNz");
        assert_eq!(p("{random:a|a}", "").unwrap(), "a");
        assert!(p("{nope}", "").unwrap_err().contains("nope"));
        assert!(p("{}", "").is_err());
        assert!(p("{random:a}", "").unwrap_err().contains("\"random\""));
        // An unterminated function still calls it.
        assert_eq!(p("x{response", "r").unwrap(), "xr");
    }

    #[test]
    fn json_xml_regex() {
        let json = r#"{"files":[{"url":"https://a/1"}],"ok":true}"#;
        assert_eq!(p("{json:files[0].url}", json).unwrap(), "https://a/1");
        assert_eq!(p("{json:$.ok}", json).unwrap(), "True");
        assert_eq!(p("{json:{response}|files[0].url}", json).unwrap(), "https://a/1");
        assert_eq!(p("{json:missing}", json).unwrap(), "");
        assert!(p("{json:files}", json).is_err());
        assert!(p("{json:x}", "not json").unwrap_err().starts_with(JSON_READER_EXCEPTION));
        let xml = "<files><file><url>https://a/1</url></file><file><url>https://a/2</url></file></files>";
        assert_eq!(p("{xml:/files/file[2]/url}", xml).unwrap(), "https://a/2");
        assert_eq!(p("{xml:{response}|//url}", xml).unwrap(), "https://a/1");
        let html = r#"<a href="https://a/b">x</a>"#;
        assert_eq!(p(r#"{regex:(?<=href=").+(?=")}"#, html).unwrap(), "https://a/b");
        assert_eq!(p(r#"{regex:href="(.+)"|1}"#, html).unwrap(), "https://a/b");
        assert_eq!(p(r#"{regex:href="(?<url>.+)"|url}"#, html).unwrap(), "https://a/b");
        assert_eq!(p(r#"{regex:{response}|href="(.+)"|1}"#, html).unwrap(), "https://a/b");
        assert_eq!(p(r#"{regex:href="(.+)"|5}"#, html).unwrap(), "");
        assert_eq!(p("{regex:zzz}", html).unwrap(), "");
    }

    #[test]
    fn name_parser_runs_first_with_escapes_kept() {
        let parser = CustomUploaderParser { use_name_parser: true, ..CustomUploaderParser::with_input("f", "i") };
        let year = chrono::Local::now().format("%Y").to_string();
        assert_eq!(parser.parse("%y-\\%y").unwrap(), format!("{year}-%y"));
    }
}
