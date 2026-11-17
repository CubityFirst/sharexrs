//! `CustomUploader/CustomUploaderItem.cs`: a custom uploader, as `.sxcu`
//! files and `UploadersConfig.CustomUploadersList` hold it, and how it
//! builds requests and reads responses.

use serde_json::{Map, Value};
use sharex_helpers::{NameParser, NameParserType};
use sharex_settings::settings_base::compare_version;
use sharex_strings::uploaders_lib as strings;

use super::parser::{CustomUploaderParser, JSON_READER_EXCEPTION};
use crate::enums::{CustomUploaderBody, CustomUploaderDestinationType, HttpMethod};
use crate::http::{self, ResponseInfo};
use crate::result::{UploadResult, UploaderErrorManager};
use crate::uploader::Headers;
use crate::url_helpers;

/// An ordered `Dictionary<string, string>` (values may be `null`).
pub type StringMap = Vec<(String, Option<String>)>;

/// `CustomUploaderInput`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomUploaderInput {
    pub file_name: String,
    pub input: String,
}

impl CustomUploaderInput {
    pub fn new(file_name: &str, input: &str) -> Self {
        Self { file_name: file_name.to_owned(), input: input.to_owned() }
    }
}

/// `CustomUploaderItem`.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomUploaderItem {
    pub version: Option<String>,
    pub name: Option<String>,
    pub destination_type: CustomUploaderDestinationType,
    pub request_method: HttpMethod,
    pub request_url: Option<String>,
    pub parameters: Option<StringMap>,
    pub headers: Option<StringMap>,
    pub body: CustomUploaderBody,
    pub arguments: Option<StringMap>,
    pub file_form_name: Option<String>,
    pub data: Option<String>,
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub deletion_url: Option<String>,
    pub error_message: Option<String>,
    /// Properties ShareX doesn't know (kept, so saving loses nothing).
    pub extra: Map<String, Value>,
}

impl Default for CustomUploaderItem {
    /// The private constructor Newtonsoft uses: everything unset but the
    /// request method.
    fn default() -> Self {
        Self {
            version: None,
            name: None,
            destination_type: CustomUploaderDestinationType::None,
            request_method: HttpMethod::POST,
            request_url: None,
            parameters: None,
            headers: None,
            body: CustomUploaderBody::None,
            arguments: None,
            file_form_name: None,
            data: None,
            url: None,
            thumbnail_url: None,
            deletion_url: None,
            error_message: None,
            extra: Map::new(),
        }
    }
}

/// How an item is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializeMode {
    /// In `UploadersConfig.json`: defaults and nulls included.
    Config,
    /// A `.sxcu` export (`DefaultValueHandling.Ignore`, `NullValueHandling.Ignore`).
    Export,
}

fn non_empty(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

fn map_non_empty(m: &Option<StringMap>) -> bool {
    m.as_ref().is_some_and(|m| !m.is_empty())
}

fn is_null_or_empty(s: Option<&str>) -> bool {
    s.is_none_or(str::is_empty)
}

impl CustomUploaderItem {
    /// `CustomUploaderItem.Init()`: a new uploader in the editor.
    pub fn init() -> Self {
        Self {
            version: Some(sharex_settings::settings_base::application_version().to_owned()),
            request_method: HttpMethod::POST,
            body: CustomUploaderBody::MultipartFormData,
            ..Default::default()
        }
    }

    /// Reads an item as `JsonHelpers.Deserialize` does (errors ignored: a
    /// value that doesn't convert keeps its default).
    pub fn from_value(value: Value) -> Option<Self> {
        let Value::Object(mut map) = value else { return None };
        let mut item = Self::default();
        let take = |map: &mut Map<String, Value>, name: &str| sharex_settings::macros::take(map, name);
        let string = |v: Option<Value>| -> Option<Option<String>> {
            match v? {
                Value::Null => Some(None),
                other => crate::json::to_dotnet_string(&other).ok(),
            }
        };
        let dict = |v: Option<Value>| -> Option<Option<StringMap>> {
            match v? {
                Value::Null => Some(None),
                Value::Object(m) => Some(Some(
                    m.into_iter().map(|(k, v)| (k, if v.is_null() { None } else { crate::json::to_dotnet_string(&v).ok().flatten() })).collect(),
                )),
                _ => None,
            }
        };
        if let Some(v) = string(take(&mut map, "Version")) {
            item.version = v;
        }
        if let Some(v) = string(take(&mut map, "Name")) {
            item.name = v;
        }
        if let Some(v) = take(&mut map, "DestinationType").and_then(|v| serde_json::from_value(v).ok()) {
            item.destination_type = v;
        }
        if let Some(v) = take(&mut map, "RequestMethod").and_then(|v| serde_json::from_value(v).ok()) {
            item.request_method = v;
        }
        if let Some(v) = string(take(&mut map, "RequestURL")) {
            item.request_url = v;
        }
        if let Some(v) = dict(take(&mut map, "Parameters")) {
            item.parameters = v;
        }
        if let Some(v) = dict(take(&mut map, "Headers")) {
            item.headers = v;
        }
        if let Some(v) = take(&mut map, "Body").and_then(|v| serde_json::from_value(v).ok()) {
            item.body = v;
        }
        if let Some(v) = dict(take(&mut map, "Arguments")) {
            item.arguments = v;
        }
        if let Some(v) = string(take(&mut map, "FileFormName")) {
            item.file_form_name = v;
        }
        if let Some(v) = string(take(&mut map, "Data")) {
            item.data = v;
        }
        if let Some(v) = string(take(&mut map, "URL")) {
            item.url = v;
        }
        if let Some(v) = string(take(&mut map, "ThumbnailURL")) {
            item.thumbnail_url = v;
        }
        if let Some(v) = string(take(&mut map, "DeletionURL")) {
            item.deletion_url = v;
        }
        if let Some(v) = string(take(&mut map, "ErrorMessage")) {
            item.error_message = v;
        }
        item.extra = map;
        Some(item)
    }

    /// Reads a `.sxcu` file's text (`JsonHelpers.DeserializeFromFile`).
    pub fn from_json(text: &str) -> Result<Option<Self>, String> {
        let text = text.trim_start_matches('\u{feff}');
        if text.trim().is_empty() {
            return Ok(None);
        }
        let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        Ok(Self::from_value(value))
    }

    /// The item as JSON, with `ShouldSerialize*` and the mode's default and
    /// null handling.
    pub fn to_value(&self, mode: SerializeMode) -> Value {
        let export = mode == SerializeMode::Export;
        let mut m = Map::new();
        let s = |v: &Option<String>| v.clone().map(Value::String).unwrap_or(Value::Null);
        let dict = |d: &StringMap| {
            Value::Object(d.iter().map(|(k, v)| (k.clone(), v.clone().map(Value::String).unwrap_or(Value::Null))).collect())
        };
        // [DefaultValue("")]: ignored when null or "" on export.
        let put_str = |m: &mut Map<String, Value>, name: &str, v: &Option<String>| {
            if !export || non_empty(v) {
                m.insert(name.into(), s(v));
            }
        };
        put_str(&mut m, "Version", &self.version);
        // ShouldSerializeName.
        if non_empty(&self.name) && self.name.as_deref() != Some(&url_helpers::get_host_name(self.request_url.as_deref().unwrap_or(""))) {
            m.insert("Name".into(), s(&self.name));
        }
        if !export || self.destination_type != CustomUploaderDestinationType::None {
            m.insert("DestinationType".into(), serde_json::to_value(self.destination_type).expect("flags"));
        }
        // Always written ([JsonProperty(DefaultValueHandling.Include)]).
        m.insert("RequestMethod".into(), serde_json::to_value(&self.request_method).expect("enum"));
        put_str(&mut m, "RequestURL", &self.request_url);
        if map_non_empty(&self.parameters) {
            m.insert("Parameters".into(), dict(self.parameters.as_ref().expect("checked")));
        }
        if map_non_empty(&self.headers) {
            m.insert("Headers".into(), dict(self.headers.as_ref().expect("checked")));
        }
        if !export || self.body != CustomUploaderBody::None {
            m.insert("Body".into(), serde_json::to_value(&self.body).expect("enum"));
        }
        if matches!(self.body, CustomUploaderBody::MultipartFormData | CustomUploaderBody::FormURLEncoded) && map_non_empty(&self.arguments) {
            m.insert("Arguments".into(), dict(self.arguments.as_ref().expect("checked")));
        }
        if self.body == CustomUploaderBody::MultipartFormData && non_empty(&self.file_form_name) {
            m.insert("FileFormName".into(), s(&self.file_form_name));
        }
        if matches!(self.body, CustomUploaderBody::JSON | CustomUploaderBody::XML) && non_empty(&self.data) {
            m.insert("Data".into(), s(&self.data));
        }
        put_str(&mut m, "URL", &self.url);
        put_str(&mut m, "ThumbnailURL", &self.thumbnail_url);
        put_str(&mut m, "DeletionURL", &self.deletion_url);
        put_str(&mut m, "ErrorMessage", &self.error_message);
        for (k, v) in &self.extra {
            if !(export && v.is_null()) {
                m.insert(k.clone(), v.clone());
            }
        }
        Value::Object(m)
    }

    /// The `.sxcu` export text (indented as `JsonHelpers.SerializeToFile`).
    pub fn to_sxcu(&self) -> String {
        sharex_settings::settings_base::to_json(&self.to_value(SerializeMode::Export))
    }

    /// `ToString()`: the name, else the request URL's host, else "Name".
    pub fn display_name(&self) -> String {
        if non_empty(&self.name) {
            return self.name.clone().expect("checked");
        }
        let name = url_helpers::get_host_name(self.request_url.as_deref().unwrap_or(""));
        if !name.is_empty() { name } else { "Name".to_owned() }
    }

    /// `GetFileName()`.
    pub fn file_name(&self) -> String {
        format!("{}.sxcu", self.display_name())
    }

    /// `GetRequestURL(input)`.
    pub fn get_request_url(&self, input: &CustomUploaderInput) -> Result<String, String> {
        let request_url = self.request_url.as_deref().filter(|u| !u.is_empty());
        let Some(request_url) = request_url else { return Err(strings::CustomUploaderItem_Request_URL_must_be_configured().to_owned()) };
        let mut parser = CustomUploaderParser::with_input(&input.file_name, &input.input);
        parser.url_encode = true;
        let url = url_helpers::fix_prefix(&parser.parse(request_url)?);
        let parameters = self.get_parameters(input)?;
        Ok(url_helpers::create_query_string_url(&url, &parameters, false))
    }

    fn parse_map(map: &Option<StringMap>, input: &CustomUploaderInput) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        if let Some(map) = map {
            let mut parser = CustomUploaderParser::with_input(&input.file_name, &input.input);
            parser.use_name_parser = true;
            parser.allow_name_parser_file_read = false;
            for (k, v) in map {
                out.push((k.clone(), parser.parse(v.as_deref().unwrap_or(""))?));
            }
        }
        Ok(out)
    }

    /// `GetParameters(input)`.
    pub fn get_parameters(&self, input: &CustomUploaderInput) -> Result<Vec<(String, String)>, String> {
        Self::parse_map(&self.parameters, input)
    }

    /// `GetArguments(input)`.
    pub fn get_arguments(&self, input: &CustomUploaderInput) -> Result<Vec<(String, String)>, String> {
        Self::parse_map(&self.arguments, input)
    }

    /// `GetHeaders(input)`: `None` when there are none.
    pub fn get_headers(&self, input: &CustomUploaderInput) -> Result<Option<Headers>, String> {
        if !map_non_empty(&self.headers) {
            return Ok(None);
        }
        let mut headers = Headers::new();
        for (k, v) in Self::parse_map(&self.headers, input)? {
            headers.add(&k, v);
        }
        Ok(Some(headers))
    }

    /// `GetContentType()`.
    pub fn get_content_type(&self) -> Option<&'static str> {
        match self.body {
            CustomUploaderBody::MultipartFormData => Some(http::CONTENT_TYPE_MULTIPART_FORM_DATA),
            CustomUploaderBody::FormURLEncoded => Some(http::CONTENT_TYPE_URL_ENCODED),
            CustomUploaderBody::JSON => Some(http::CONTENT_TYPE_JSON),
            CustomUploaderBody::XML => Some(http::CONTENT_TYPE_XML),
            CustomUploaderBody::Binary => Some(http::CONTENT_TYPE_OCTET_STREAM),
            _ => None,
        }
    }

    /// `GetData(input)`: the JSON/XML body with name pattern tokens, then
    /// `{input}` and `{filename}` (encoded for the body) replaced.
    pub fn get_data(&self, input: &CustomUploaderInput) -> String {
        let mut np = NameParser::new(NameParserType::Text);
        np.allow_file_read = false;
        let result = np.parse(self.data.as_deref().unwrap_or(""));
        let replace = [("{input}", self.encode_body_data(&input.input)), ("{filename}", self.encode_body_data(&input.file_name))];
        batch_replace_ignore_case(&result, &replace)
    }

    fn encode_body_data(&self, input: &str) -> String {
        if input.is_empty() {
            return input.to_owned();
        }
        match self.body {
            CustomUploaderBody::JSON => url_helpers::json_encode(input),
            CustomUploaderBody::XML => url_helpers::xml_encode(input),
            _ => input.to_owned(),
        }
    }

    /// `GetFileFormName()`.
    pub fn get_file_form_name(&self) -> Result<String, String> {
        match self.file_form_name.as_deref().filter(|f| !f.is_empty()) {
            Some(f) => Ok(f.to_owned()),
            None => Err(strings::CustomUploaderItem_File_form_name_must_be_configured().to_owned()),
        }
    }

    /// `ParseResponse`: fills the result's URLs from the response, or adds
    /// the uploader's error message when the request failed.
    pub fn parse_response(
        &self,
        result: &mut UploadResult,
        response_info: Option<&ResponseInfo>,
        errors: &mut UploaderErrorManager,
        input: &CustomUploaderInput,
        is_shortened_url: bool,
    ) -> Result<(), String> {
        let Some(response_info) = response_info else { return Ok(()) };
        let mut response_info = response_info.clone();
        if response_info.response_text.is_none() {
            response_info.response_text = Some(String::new());
        }
        result.response_info = Some(response_info.clone());
        let parser = CustomUploaderParser {
            file_name: Some(input.file_name.clone()),
            response_info: Some(response_info.clone()),
            url_encode: true,
            ..Default::default()
        };
        if response_info.is_success() {
            let url = if non_empty(&self.url) {
                let template = self.url.as_deref().expect("checked");
                let url = parser.parse(template)?;
                if url.is_empty() && template.contains("{output:") {
                    result.is_url_expected = false;
                }
                url
            } else {
                response_info.response_text.clone().unwrap_or_default()
            };
            if is_shortened_url {
                result.shortened_url = Some(url);
            } else {
                result.url = Some(url);
            }
            result.thumbnail_url = Some(parser.parse(self.thumbnail_url.as_deref().unwrap_or(""))?);
            result.deletion_url = Some(parser.parse(self.deletion_url.as_deref().unwrap_or(""))?);
        } else if let Some(message) = self.error_message.as_deref().filter(|m| !m.is_empty()) {
            let parsed = parser.parse(message)?;
            if !parsed.is_empty() {
                errors.add_first(parsed);
            }
        }
        Ok(())
    }

    /// `TryParseResponse`: [`Self::parse_response`], reporting a failure to
    /// parse as an error instead.
    pub fn try_parse_response(
        &self,
        result: &mut UploadResult,
        response_info: Option<&ResponseInfo>,
        errors: &mut UploaderErrorManager,
        input: &CustomUploaderInput,
        is_shortened_url: bool,
    ) {
        if let Err(e) = self.parse_response(result, response_info, errors, input, is_shortened_url) {
            let host = url_helpers::get_host_name(self.request_url.as_deref().unwrap_or(""));
            let nl = crate::NEW_LINE;
            let text = match e.strip_prefix(JSON_READER_EXCEPTION) {
                Some(detail) => format!("{}{nl}{nl}Newtonsoft.Json.JsonReaderException: {detail}", strings::CustomUploaderItem_Invalid_JSON_response().replace("{0}", &host)),
                None => format!("{}{nl}{nl}System.Exception: {e}", strings::CustomUploaderItem_Unable_to_parse_response().replace("{0}", &host)),
            };
            errors.add_first(text);
        }
    }

    /// `CheckBackwardCompatibility()`: rejects uploaders older than 12.3.1
    /// and moves 13.7.1 and older to the current syntax.
    pub fn check_backward_compatibility(&mut self) -> Result<(), String> {
        let version = self.version.clone().unwrap_or_default();
        if version.is_empty() || compare_version(&version, "12.3.1").is_le() {
            return Err(format!("{}: {}", strings::CustomUploaderItem_Unsupported_custom_uploader(), self.display_name()));
        }
        self.check_request_url();
        if compare_version(&version, "13.7.1").is_le() {
            self.request_url = migrate_old_syntax(self.request_url.as_deref());
            for map in [&mut self.parameters, &mut self.headers, &mut self.arguments].into_iter().flatten() {
                for (_, v) in map.iter_mut() {
                    *v = migrate_old_syntax(v.as_deref());
                }
            }
            if let Some(data) = self.data.as_mut() {
                *data = replace_ignore_case(&replace_ignore_case(data, "$input$", "{input}"), "$filename$", "{filename}");
            }
            self.url = migrate_old_syntax(self.url.as_deref());
            self.thumbnail_url = migrate_old_syntax(self.thumbnail_url.as_deref());
            self.deletion_url = migrate_old_syntax(self.deletion_url.as_deref());
            self.error_message = migrate_old_syntax(self.error_message.as_deref());
            self.version = Some(sharex_settings::settings_base::application_version().to_owned());
        }
        Ok(())
    }

    /// `CheckRequestURL()`: moves a query string in the request URL into
    /// `Parameters`.
    fn check_request_url(&mut self) {
        let Some(url) = self.request_url.clone().filter(|u| !u.is_empty()) else { return };
        let Some(query) = url_helpers::parse_query_string(&url) else { return };
        if query.is_empty() {
            return;
        }
        let parameters = self.parameters.get_or_insert_with(Vec::new);
        for (key, values) in query {
            match key {
                None => {
                    for value in values {
                        if !parameters.iter().any(|(k, _)| *k == value) {
                            parameters.push((value, Some(String::new())));
                        }
                    }
                }
                Some(key) => {
                    if !parameters.iter().any(|(k, _)| *k == key) {
                        parameters.push((key, Some(values.join(","))));
                    }
                }
            }
        }
        self.request_url = Some(url_helpers::remove_query_string(&url));
    }
}

/// `MigrateOldSyntax`: `$name$` to `{name}`, existing braces escaped.
/// Like ShareX, a `\` drops itself and the character after it.
fn migrate_old_syntax(input: Option<&str>) -> Option<String> {
    let input = input?;
    if input.is_empty() {
        return Some(String::new());
    }
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut start = true;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '$' {
            out.push(if start { '{' } else { '}' });
            start = !start;
            i += 1;
            continue;
        } else if c == '\\' {
            i += 2;
            continue;
        } else if c == '{' || c == '}' {
            out.push('\\');
        }
        out.push(c);
        i += 1;
    }
    Some(out)
}

fn replace_ignore_case(text: &str, from: &str, to: &str) -> String {
    batch_replace_ignore_case(text, &[(from, to.to_owned())])
}

/// `StringExtensions.BatchReplace(replace, OrdinalIgnoreCase)`.
fn batch_replace_ignore_case(text: &str, replace: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let bytes = text.as_bytes();
    'outer: while i < text.len() {
        for (key, value) in replace {
            let k = key.as_bytes();
            if bytes.len() - i >= k.len() && bytes[i..i + k.len()].eq_ignore_ascii_case(k) {
                out.push_str(value);
                i += k.len();
                continue 'outer;
            }
        }
        let c = text[i..].chars().next().expect("in bounds");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Is `s` null or empty (`string.IsNullOrEmpty`).
pub fn null_or_empty(s: Option<&str>) -> bool {
    is_null_or_empty(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_exports_sxcu() {
        let sxcu = r#"{
  "Version": "15.0.0",
  "Name": "Example",
  "DestinationType": "ImageUploader, FileUploader",
  "RequestMethod": "POST",
  "RequestURL": "https://example.com/upload",
  "Headers": { "Authorization": "Bearer x" },
  "Body": "MultipartFormData",
  "FileFormName": "file",
  "URL": "{json:url}",
  "Future": 1
}"#;
        let item = CustomUploaderItem::from_json(sxcu).unwrap().unwrap();
        assert_eq!(item.destination_type, CustomUploaderDestinationType::ImageUploader | CustomUploaderDestinationType::FileUploader);
        assert_eq!(item.file_form_name.as_deref(), Some("file"));
        let exported = item.to_sxcu();
        assert!(exported.contains("\"DestinationType\": \"ImageUploader, FileUploader\""));
        assert!(!exported.contains("ThumbnailURL"));
        assert!(exported.contains("\"Future\": 1"));
        let config = item.to_value(SerializeMode::Config);
        assert_eq!(config["ThumbnailURL"], Value::Null);
    }

    #[test]
    fn migrates_old_syntax() {
        let mut item = CustomUploaderItem {
            version: Some("13.0.0".into()),
            request_url: Some("https://a/up?key=1&flag".into()),
            url: Some("$json:url$ {x}".into()),
            data: Some("{\"a\":\"$INPUT$\"}".into()),
            ..Default::default()
        };
        item.check_backward_compatibility().unwrap();
        assert_eq!(item.request_url.as_deref(), Some("https://a/up"));
        assert_eq!(item.parameters, Some(vec![("key".into(), Some("1".into())), ("flag".into(), Some(String::new()))]));
        assert_eq!(item.url.as_deref(), Some("{json:url} \\{x\\}"));
        assert_eq!(item.data.as_deref(), Some("{\"a\":\"{input}\"}"));
        let mut old = CustomUploaderItem { version: Some("12.3.1".into()), ..Default::default() };
        assert!(old.check_backward_compatibility().is_err());
    }

    #[test]
    fn request_url_and_data() {
        let item = CustomUploaderItem {
            request_url: Some("example.com/{filename}".into()),
            parameters: Some(vec![("q".into(), Some("{input}".into()))]),
            body: CustomUploaderBody::JSON,
            data: Some("{\"text\":\"{input}\",\"name\":\"{FileName}\"}".into()),
            ..Default::default()
        };
        let input = CustomUploaderInput::new("a b.txt", "x \"y\"");
        assert_eq!(item.get_request_url(&input).unwrap(), "https://example.com/a%20b.txt?q=x+%22y%22");
        assert_eq!(item.get_data(&input), "{\"text\":\"x \\\"y\\\"\",\"name\":\"a b.txt\"}");
    }
}
