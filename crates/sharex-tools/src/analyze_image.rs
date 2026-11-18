//! Analyze image (`ShareX.Tools/Tools/AnalyzeImage`): the image and a
//! prompt sent to an AI provider, and the text it answers.
//!
//! - [`AIOptions`]: `AIOptions` (`TaskSettingsTools.AIOptions`), with the
//!   API keys `[JsonEncrypt]`ed as ShareX writes them.
//! - The four providers' requests, built as ShareX builds them
//!   ([`build_analyze_request`]): `OpenAIProvider` (Responses API, JPEG at
//!   quality 90, `System.Text.Json`), `OpenAILegacyProvider` (chat
//!   completions), `GeminiProvider` and `OpenRouterProvider` (PNG,
//!   Newtonsoft, a plain `HttpClient`), and their answers read as ShareX
//!   reads them ([`parse_analyze_response`]).
//! - `AnalyzeImageService`: [`analyze`], [`test_connection`], [`load_models`].
//!
//! Differences from ShareX, on purpose:
//! - The OpenAI providers set their `Authorization` header on the shared
//!   `HttpClientFactory` client, so ShareX goes on sending the OpenAI key
//!   with every later request through it, to any host. ShareXrs sets it on
//!   the request only.
//! - The user agent is ShareXrs's (`ShareXrs/<version>`), also on the
//!   Gemini and OpenRouter requests, which ShareX sends without one.
//! - PNGs (Gemini and OpenRouter, for an image that isn't a file) decode
//!   to Skia's pixels but aren't Skia's bytes (zlib), as everywhere in this
//!   crate (docs/TOOLS.md).

use std::path::Path;

use base64::Engine;
use serde_json::{Map, Value, json};
use sharex_strings::tools as s;
use sharex_uploaders::HttpMethod;
use sharex_uploaders::dpapi::Secret;
use sharex_uploaders::http::{self, Content, HttpRequest, RequestOptions};
use sharex_uploaders::url_helpers::combine_url;

use crate::imaging;

/// `AIProvider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIProvider {
    OpenAI,
    Gemini,
    OpenRouter,
    OpenAILegacy,
}

impl AIProvider {
    pub const ALL: [AIProvider; 4] = [AIProvider::OpenAI, AIProvider::Gemini, AIProvider::OpenRouter, AIProvider::OpenAILegacy];

    pub fn name(self) -> &'static str {
        match self {
            AIProvider::OpenAI => "OpenAI",
            AIProvider::Gemini => "Gemini",
            AIProvider::OpenRouter => "OpenRouter",
            AIProvider::OpenAILegacy => "OpenAILegacy",
        }
    }

    /// Newtonsoft's `StringEnumConverter` (names, any case) or the number.
    fn from_json(value: &Value) -> Option<Self> {
        match value {
            Value::String(name) => Self::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(name.trim())),
            Value::Number(n) => n.as_u64().and_then(|i| Self::ALL.get(i as usize).copied()),
            _ => None,
        }
    }
}

/// `AIOptions`.
#[derive(Debug, Clone, PartialEq)]
pub struct AIOptions {
    /// `Provider`; `None` for a value ShareX doesn't know (kept as read).
    pub provider: Option<AIProvider>,
    provider_raw: Value,
    pub openai_api_key: Secret,
    pub openai_model: Option<String>,
    pub openai_custom_url: Option<String>,
    pub openai_reasoning_effort: Option<String>,
    pub openai_verbosity: Option<String>,
    pub gemini_api_key: Secret,
    pub gemini_model: Option<String>,
    pub openrouter_api_key: Secret,
    pub openrouter_model: Option<String>,
    /// `Input`: the prompt.
    pub input: Option<String>,
    pub auto_start_region: bool,
    pub auto_start_analyze: bool,
    pub auto_copy_result: bool,
    /// Properties a newer ShareX added, kept as read.
    extra: Map<String, Value>,
}

impl Default for AIOptions {
    fn default() -> Self {
        Self {
            provider: Some(AIProvider::OpenAI),
            provider_raw: json!("OpenAI"),
            openai_api_key: Secret::null(),
            openai_model: Some("gpt-5-mini".into()),
            openai_custom_url: None,
            openai_reasoning_effort: Some("minimal".into()),
            openai_verbosity: Some("medium".into()),
            gemini_api_key: Secret::null(),
            gemini_model: Some("gemini-1.5-flash-latest".into()),
            openrouter_api_key: Secret::null(),
            openrouter_model: Some("google/gemini-flash-1.5".into()),
            input: Some(s::AIOptions_What_is_in_this_image().to_owned()),
            auto_start_region: true,
            auto_start_analyze: true,
            auto_copy_result: false,
            extra: Map::new(),
        }
    }
}

const KNOWN: [&str; 14] = [
    "Provider",
    "OpenAIAPIKey",
    "OpenAIModel",
    "OpenAICustomURL",
    "OpenAIReasoningEffort",
    "OpenAIVerbosity",
    "GeminiAPIKey",
    "GeminiModel",
    "OpenRouterAPIKey",
    "OpenRouterModel",
    "Input",
    "AutoStartRegion",
    "AutoStartAnalyze",
    "AutoCopyResult",
];

impl AIOptions {
    /// Reads `TaskSettingsTools.AIOptions` as ShareX's settings reader does:
    /// missing properties keep their defaults, keys are decrypted.
    pub fn from_json(value: &Value) -> Self {
        let mut o = Self::default();
        let Some(map) = value.as_object() else { return o };
        let get = |name: &str| sharex_uploaders::json::get(value, name);
        let string = |name: &str, current: &Option<String>| match get(name) {
            None => current.clone(),
            Some(v) => sharex_uploaders::json::to_dotnet_string(v).ok().flatten(),
        };
        let secret = |name: &str| match get(name) {
            None => Secret::null(),
            Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
        };
        let boolean = |name: &str, current: bool| match get(name) {
            Some(Value::Bool(b)) => *b,
            _ => current,
        };
        if let Some(p) = get("Provider") {
            o.provider = AIProvider::from_json(p);
            o.provider_raw = p.clone();
        }
        o.openai_api_key = secret("OpenAIAPIKey");
        o.openai_model = string("OpenAIModel", &o.openai_model);
        o.openai_custom_url = string("OpenAICustomURL", &o.openai_custom_url);
        o.openai_reasoning_effort = string("OpenAIReasoningEffort", &o.openai_reasoning_effort);
        o.openai_verbosity = string("OpenAIVerbosity", &o.openai_verbosity);
        o.gemini_api_key = secret("GeminiAPIKey");
        o.gemini_model = string("GeminiModel", &o.gemini_model);
        o.openrouter_api_key = secret("OpenRouterAPIKey");
        o.openrouter_model = string("OpenRouterModel", &o.openrouter_model);
        o.input = string("Input", &o.input);
        o.auto_start_region = boolean("AutoStartRegion", o.auto_start_region);
        o.auto_start_analyze = boolean("AutoStartAnalyze", o.auto_start_analyze);
        o.auto_copy_result = boolean("AutoCopyResult", o.auto_copy_result);
        for (k, v) in map {
            if !KNOWN.iter().any(|n| n.eq_ignore_ascii_case(k)) {
                o.extra.insert(k.clone(), v.clone());
            }
        }
        o
    }

    /// Writes it back in ShareX's property order, keys encrypted as
    /// `[JsonEncrypt]` writes them (unchanged ones exactly as read).
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        let opt = |v: &Option<String>| v.clone().map_or(Value::Null, Value::String);
        let secret = |v: &Secret| serde_json::to_value(v).unwrap_or(Value::Null);
        let provider = match self.provider {
            Some(p) if AIProvider::from_json(&self.provider_raw) != Some(p) => json!(p.name()),
            _ => self.provider_raw.clone(),
        };
        map.insert("Provider".into(), provider);
        map.insert("OpenAIAPIKey".into(), secret(&self.openai_api_key));
        map.insert("OpenAIModel".into(), opt(&self.openai_model));
        map.insert("OpenAICustomURL".into(), opt(&self.openai_custom_url));
        map.insert("OpenAIReasoningEffort".into(), opt(&self.openai_reasoning_effort));
        map.insert("OpenAIVerbosity".into(), opt(&self.openai_verbosity));
        map.insert("GeminiAPIKey".into(), secret(&self.gemini_api_key));
        map.insert("GeminiModel".into(), opt(&self.gemini_model));
        map.insert("OpenRouterAPIKey".into(), secret(&self.openrouter_api_key));
        map.insert("OpenRouterModel".into(), opt(&self.openrouter_model));
        map.insert("Input".into(), opt(&self.input));
        map.insert("AutoStartRegion".into(), json!(self.auto_start_region));
        map.insert("AutoStartAnalyze".into(), json!(self.auto_start_analyze));
        map.insert("AutoCopyResult".into(), json!(self.auto_copy_result));
        for (k, v) in &self.extra {
            map.insert(k.clone(), v.clone());
        }
        Value::Object(map)
    }

    /// `HasAPIKey`.
    pub fn has_api_key(&self) -> bool {
        let set = |k: &Secret| k.get().is_some_and(|v| !v.trim().is_empty());
        match self.provider {
            Some(AIProvider::OpenAI | AIProvider::OpenAILegacy) => set(&self.openai_api_key),
            Some(AIProvider::Gemini) => set(&self.gemini_api_key),
            Some(AIProvider::OpenRouter) => set(&self.openrouter_api_key),
            None => false,
        }
    }

    /// The page where the provider's API keys are made
    /// (`AnalyzeImageOptionsViewModel`).
    pub fn api_key_page(&self) -> Option<&'static str> {
        match self.provider? {
            AIProvider::OpenAI | AIProvider::OpenAILegacy => Some("https://platform.openai.com/api-keys"),
            AIProvider::Gemini => Some("https://aistudio.google.com/app/apikey"),
            AIProvider::OpenRouter => Some("https://openrouter.ai/keys"),
        }
    }
}

/// `AnalyzeImageOptionsViewModel`'s lists.
pub const OPENAI_MODELS: [&str; 5] = ["gpt-5.2", "gpt-5.1", "gpt-5", "gpt-5-mini", "gpt-5-nano"];
pub const GEMINI_MODELS: [&str; 4] = ["gemini-2.0-flash", "gemini-2.0-flash-lite", "gemini-1.5-flash", "gemini-1.5-pro"];
pub const OPENROUTER_MODELS: [&str; 3] = ["openai/gpt-4o", "anthropic/claude-3.5-sonnet", "google/gemini-pro-1.5"];
pub const REASONING_EFFORTS: [&str; 4] = ["minimal", "low", "medium", "high"];
pub const VERBOSITY_LEVELS: [&str; 3] = ["low", "medium", "high"];

/// `AnalyzeImageViewModel.PresetPrompts`.
pub fn preset_prompts() -> [&'static str; 4] {
    [
        s::AnalyzeImageViewModel_What_is_in_this_image(),
        s::AnalyzeImageViewModel_Thoroughly_describe_this_image(),
        s::AnalyzeImageViewModel_Transcribe_image_text(),
        s::AnalyzeImageViewModel_Translate_text_into_English(),
    ]
}

/// What's analysed: `AnalyzeImageService.AnalyzeAsync(imagePath, imageData, options)`.
#[derive(Debug, Clone, Copy)]
pub enum AnalyzeInput<'a> {
    /// An image file (`provider.AnalyzeImage(imagePath, ...)`).
    File(&'a Path),
    /// Encoded image bytes, e.g. a captured region (`ByteArrayToBitmap`).
    Data(&'a [u8]),
}

/// A request ready to send.
#[derive(Debug, Clone, PartialEq)]
pub struct AIRequest {
    pub method: HttpMethod,
    pub url: String,
    /// Request headers (the user agent and `Cache-Control` are added when sent).
    pub headers: Vec<(String, String)>,
    /// The body and its content type.
    pub body: Option<(Vec<u8>, String)>,
    /// Through `HttpClientFactory`'s client (`Cache-Control: no-cache`), or a
    /// plain `new HttpClient()` (Gemini, OpenRouter).
    pub factory_client: bool,
}

/// `StringContent(json, Encoding.UTF8, "application/json")`'s content type.
const JSON_UTF8: &str = "application/json; charset=utf-8";

/// The OpenAI providers' headers: their `Authorization` goes on the
/// client's default headers after the factory's user agent and
/// `Cache-Control`, so it's sent after them (ShareXrs keeps it to this
/// request; see the module docs).
fn openai_default_headers(key: Option<&str>) -> Vec<(String, String)> {
    vec![("User-Agent".to_owned(), http::user_agent()), ("Cache-Control".to_owned(), "no-cache".to_owned()), bearer(key)]
}

/// `new AuthenticationHeaderValue("Bearer", key)`.
fn bearer(key: Option<&str>) -> (String, String) {
    let value = match key {
        Some(k) if !k.is_empty() => format!("Bearer {k}"),
        _ => "Bearer".to_owned(),
    };
    ("Authorization".to_owned(), value)
}

/// The prompt the providers send: `Input`, else (OpenAI) the default.
fn prompt_or_default(input: Option<&str>) -> String {
    match input {
        Some(p) if !p.is_empty() => p.to_owned(),
        _ => s::AnalyzeImageViewModel_What_is_in_this_image().to_owned(),
    }
}

/// The base 64 of the image a provider sends: OpenAI a JPEG at quality 90
/// of the decoded image; Gemini and OpenRouter the file's own bytes, or a
/// PNG of decoded bytes.
fn image_base64(provider: AIProvider, input: AnalyzeInput) -> Result<String, String> {
    let b64 = |data: &[u8]| base64::engine::general_purpose::STANDARD.encode(data);
    match (provider, input) {
        (AIProvider::OpenAI | AIProvider::OpenAILegacy, AnalyzeInput::File(path)) => {
            // `SkiaImageHelpers.LoadImage` (null on failure, which then throws).
            let bmp = imaging::load_image(path).ok_or_else(|| "Value cannot be null. (Parameter 'bitmap')".to_owned())?;
            Ok(b64(&imaging::save_jpeg(&bmp, 90, imaging::JpegDownsample::Downsample420)))
        }
        (AIProvider::OpenAI | AIProvider::OpenAILegacy, AnalyzeInput::Data(data)) => {
            let bmp = decode(data)?;
            Ok(b64(&imaging::save_jpeg(&bmp, 90, imaging::JpegDownsample::Downsample420)))
        }
        // `SkiaImageHelpers.ImageFileToBase64`: `File.ReadAllBytes`.
        (_, AnalyzeInput::File(path)) => std::fs::read(path).map(|d| b64(&d)).map_err(|e| file_error(path, &e)),
        // `ImageToBase64(image, ImageFileFormat.Png)`.
        (_, AnalyzeInput::Data(data)) => Ok(b64(&imaging::encode_png(&decode(data)?))),
    }
}

/// `SkiaImageHelpers.ByteArrayToBitmap` (EXIF orientation applied).
fn decode(data: &[u8]) -> Result<imaging::Bitmap, String> {
    imaging::decode(data, true).map_err(|_| "The image format is not supported.".to_owned())
}

fn file_error(path: &Path, e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => format!("Could not find file '{}'.", path.display()),
        _ => e.to_string(),
    }
}

/// The request `AnalyzeImageService.AnalyzeAsync` makes; `Ok(None)` when
/// there's nothing to analyse (no path, empty data: the result is "").
pub fn build_analyze_request(options: &AIOptions, input: AnalyzeInput) -> Result<Option<AIRequest>, String> {
    // `AIProviderFactory.GetProvider`.
    let provider = options.provider.ok_or_else(|| "Specified argument was out of the range of valid values.".to_owned())?;
    if let AnalyzeInput::Data(data) = input
        && data.is_empty()
    {
        return Ok(None);
    }
    let image = image_base64(provider, input)?;
    let request = match provider {
        // `Model.StartsWith(...)` on a null model.
        AIProvider::OpenAI if options.openai_model.is_none() => return Err(NULL_REFERENCE.to_owned()),
        AIProvider::OpenAI => openai_request(options, &image),
        AIProvider::OpenAILegacy => openai_legacy_request(options, &image),
        AIProvider::Gemini => gemini_request(options, &image),
        AIProvider::OpenRouter => openrouter_request(options, &image),
    };
    Ok(Some(request))
}

/// `OpenAIProvider.AnalyzeImageInternal`'s request.
fn openai_request(options: &AIOptions, image: &str) -> AIRequest {
    let model = options.openai_model.as_deref();
    let mut effort = options.openai_reasoning_effort.clone();
    if model.is_some_and(|m| starts_with_ignore_case(m, "gpt-5.")) {
        if effort.as_deref().is_none_or(|e| e.eq_ignore_ascii_case("minimal")) {
            effort = Some("none".into());
        }
    } else if effort.is_none() {
        effort = Some("minimal".into());
    }
    let verbosity = options.openai_verbosity.clone().unwrap_or_else(|| "medium".into());
    let input = prompt_or_default(options.input.as_deref());
    let data_uri = format!("data:image/jpeg;base64,{image}");
    // `ChatGPTRequest` through `JsonSerializer.Serialize`: declared order,
    // `text`/`image_url` left out when null.
    let mut w = StjWriter::default();
    w.begin_object();
    w.prop_str("model", model);
    w.name("reasoning");
    w.begin_object();
    w.prop_str("effort", effort.as_deref());
    w.end_object();
    w.name("input");
    w.begin_array();
    w.begin_object();
    w.prop_str("role", Some("user"));
    w.name("content");
    w.begin_array();
    w.begin_object();
    w.prop_str("type", Some("input_text"));
    w.prop_str("text", Some(&input));
    w.end_object();
    w.begin_object();
    w.prop_str("type", Some("input_image"));
    w.prop_str("image_url", Some(&data_uri));
    w.end_object();
    w.end_array();
    w.end_object();
    w.end_array();
    w.name("text");
    w.begin_object();
    w.prop_str("verbosity", Some(&verbosity));
    w.end_object();
    w.name("store");
    w.raw("false");
    w.end_object();
    AIRequest {
        method: HttpMethod::POST,
        url: openai_url(options.openai_custom_url.as_deref(), "/v1/responses"),
        headers: openai_default_headers(options.openai_api_key.get()),
        body: Some((w.finish().into_bytes(), JSON_UTF8.to_owned())),
        factory_client: true,
    }
}

/// `OpenAILegacyProvider.AnalyzeImageInternal`'s request.
fn openai_legacy_request(options: &AIOptions, image: &str) -> AIRequest {
    let input = prompt_or_default(options.input.as_deref());
    let data_uri = format!("data:image/jpeg;base64,{image}");
    let mut w = StjWriter::default();
    w.begin_object();
    w.prop_str("model", options.openai_model.as_deref());
    w.name("messages");
    w.begin_array();
    w.begin_object();
    w.prop_str("role", Some("user"));
    w.name("content");
    w.begin_array();
    w.begin_object();
    w.prop_str("type", Some("text"));
    w.prop_str("text", Some(&input));
    w.end_object();
    w.begin_object();
    w.prop_str("type", Some("image_url"));
    w.name("image_url");
    w.begin_object();
    w.prop_str("url", Some(&data_uri));
    w.end_object();
    w.end_object();
    w.end_array();
    w.end_object();
    w.end_array();
    w.end_object();
    AIRequest {
        method: HttpMethod::POST,
        url: openai_url(options.openai_custom_url.as_deref(), "/v1/chat/completions"),
        headers: openai_default_headers(options.openai_api_key.get()),
        body: Some((w.finish().into_bytes(), JSON_UTF8.to_owned())),
        factory_client: true,
    }
}

/// `CustomURL` (else `https://api.openai.com`), with `path` added unless
/// it already ends with it.
fn openai_url(custom: Option<&str>, path: &str) -> String {
    let url = match custom {
        Some(u) if !u.is_empty() => u,
        _ => "https://api.openai.com",
    };
    if url.ends_with(path) { url.to_owned() } else { combine_url(url, path) }
}

/// `GeminiProvider.AnalyzeImageInternal`'s request: the key in the query,
/// as given.
fn gemini_request(options: &AIOptions, image: &str) -> AIRequest {
    let payload = json!({
        "contents": [{
            "parts": [
                { "text": options.input },
                { "inline_data": { "mime_type": "image/png", "data": image } }
            ]
        }]
    });
    AIRequest {
        method: HttpMethod::POST,
        url: format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            options.gemini_model.as_deref().unwrap_or(""),
            options.gemini_api_key.as_str()
        ),
        headers: Vec::new(),
        body: Some((sharex_uploaders::json::to_string(&payload).into_bytes(), JSON_UTF8.to_owned())),
        factory_client: false,
    }
}

/// `OpenRouterProvider.AnalyzeImageInternal`'s request.
fn openrouter_request(options: &AIOptions, image: &str) -> AIRequest {
    let payload = json!({
        "model": options.openrouter_model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": options.input },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{image}") } }
            ]
        }],
        "max_tokens": 1024
    });
    AIRequest {
        method: HttpMethod::POST,
        url: "https://openrouter.ai/api/v1/chat/completions".to_owned(),
        headers: vec![bearer(options.openrouter_api_key.get())],
        body: Some((sharex_uploaders::json::to_string(&payload).into_bytes(), JSON_UTF8.to_owned())),
        factory_client: false,
    }
}

/// A response as the providers see it.
#[derive(Debug, Clone, PartialEq)]
pub struct AIResponse {
    pub status_code: u16,
    /// `ReasonPhrase`.
    pub reason: String,
    pub text: String,
}

impl AIResponse {
    fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// `EnsureSuccessStatusCode`'s exception message.
    fn ensure_success(&self) -> Result<(), String> {
        if self.is_success() {
            Ok(())
        } else {
            Err(format!("Response status code does not indicate success: {} ({}).", self.status_code, self.reason))
        }
    }
}

/// Sends a request built here (with ShareXrs's user agent).
pub fn send(request: &AIRequest) -> Result<AIResponse, String> {
    let content = request.body.as_ref().map(|(data, ct)| {
        let mut c = Content::bytes(data.clone(), None);
        c.extra_headers_mut().push(("Content-Type".into(), ct.clone()));
        c
    });
    let http_request = HttpRequest { method: request.method.clone(), url: request.url.clone(), headers: request.headers.clone(), content };
    let options = RequestOptions {
        // `HttpClient.Timeout`'s default, as both kinds of client have it.
        timeout: Some(std::time::Duration::from_secs(100)),
        factory_client: request.factory_client,
        ..Default::default()
    };
    let response = http::send(http_request, options).map_err(|e| e.to_string())?;
    Ok(AIResponse {
        status_code: response.status_code,
        reason: response.status_description,
        text: response.response_text.unwrap_or_default(),
    })
}

/// What a provider makes of its response: the text, or the exception's message.
/// `None` is a null string (Gemini and OpenRouter can return one).
pub fn parse_analyze_response(provider: AIProvider, response: &AIResponse) -> Result<Option<String>, String> {
    match provider {
        AIProvider::OpenAI => {
            response.ensure_success()?;
            let root = stj_parse(&response.text)?;
            let result = stj_object(&root, &[("id", Kind::Str), ("output", Kind::Array(&OUTPUT))])?;
            let Some(result) = result else { return Err(NULL_REFERENCE.to_owned()) };
            for item in result.get("output").and_then(Value::as_array).into_iter().flatten() {
                let Some(item) = item.as_object() else { return Err(NULL_REFERENCE.to_owned()) };
                if let Some(content) = item.get("content").and_then(Value::as_array)
                    && !content.is_empty()
                {
                    let Some(first) = content[0].as_object() else { return Err(NULL_REFERENCE.to_owned()) };
                    if let Some(text) = first.get("text").and_then(Value::as_str)
                        && !text.is_empty()
                    {
                        return Ok(Some(text.to_owned()));
                    }
                }
            }
            Ok(Some(String::new()))
        }
        AIProvider::OpenAILegacy => {
            response.ensure_success()?;
            let root = stj_parse(&response.text)?;
            let result = stj_object(&root, &[("id", Kind::Str), ("choices", Kind::Array(&CHOICE))])?;
            let Some(result) = result else { return Err(NULL_REFERENCE.to_owned()) };
            if let Some(choices) = result.get("choices").and_then(Value::as_array)
                && !choices.is_empty()
            {
                let Some(choice) = choices[0].as_object() else { return Err(NULL_REFERENCE.to_owned()) };
                if let Some(content) = choice.get("message").and_then(|m| m.get("content")).and_then(Value::as_str) {
                    return Ok(Some(content.to_owned()));
                }
            }
            Ok(Some(String::new()))
        }
        AIProvider::Gemini | AIProvider::OpenRouter => {
            if !response.is_success() {
                let format = if provider == AIProvider::Gemini {
                    s::GeminiProvider_API_error()
                } else {
                    s::OpenRouterProvider_API_error()
                };
                return Err(format.replace("{0}", &response.text));
            }
            let root = sharex_uploaders::json::parse(&response.text)?;
            let path: &[Step] = if provider == AIProvider::Gemini {
                &[Step::Member("candidates"), Step::Index(0), Step::Member("content"), Step::Member("parts"), Step::Index(0), Step::Member("text")]
            } else {
                &[Step::Member("choices"), Step::Index(0), Step::Member("message"), Step::Member("content")]
            };
            dynamic_string(&root, path)
        }
    }
}

/// `NullReferenceException.Message`.
const NULL_REFERENCE: &str = "Object reference not set to an instance of an object.";

/// `AnalyzeImageService.AnalyzeAsync`: builds, sends and reads the answer
/// (`None`: a null string, which the window shows as nothing).
pub fn analyze(options: &AIOptions, input: AnalyzeInput) -> Result<Option<String>, String> {
    let Some(request) = build_analyze_request(options, input)? else { return Ok(Some(String::new())) };
    let provider = options.provider.expect("built for a provider");
    let response = send(&request)?;
    parse_analyze_response(provider, &response)
}

/// What the window shows: `result?.ReplaceLineEndings("\r\n") ?? ""`.
pub fn result_text(result: Option<&str>) -> String {
    let result = result.unwrap_or("");
    let mut out = String::with_capacity(result.len());
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\r\n");
            }
            '\n' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push_str("\r\n"),
            c => out.push(c),
        }
    }
    out
}

/// `AnalyzeImageService.CreateModelsRequest`: the request, or why there isn't one.
pub fn build_models_request(options: &AIOptions) -> Result<AIRequest, String> {
    let blank = |k: &Secret| k.get().is_none_or(|v| v.trim().is_empty());
    let get = |url: String, headers: Vec<(String, String)>| AIRequest {
        method: HttpMethod::GET,
        url,
        headers,
        body: None,
        factory_client: true,
    };
    match options.provider {
        Some(AIProvider::OpenAI | AIProvider::OpenAILegacy) => {
            if blank(&options.openai_api_key) {
                return Err(s::AnalyzeImageService_Missing_OpenAI_API_key().to_owned());
            }
            let base = match options.openai_custom_url.as_deref() {
                Some(u) if !u.trim().is_empty() => u,
                _ => "https://api.openai.com",
            };
            Ok(get(combine_url(base, "v1/models"), vec![bearer(Some(options.openai_api_key.as_str().trim()))]))
        }
        Some(AIProvider::Gemini) => {
            if blank(&options.gemini_api_key) {
                return Err(s::AnalyzeImageService_Missing_Gemini_API_key().to_owned());
            }
            Ok(get(
                format!(
                    "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                    escape_data_string(options.gemini_api_key.as_str().trim())
                ),
                Vec::new(),
            ))
        }
        Some(AIProvider::OpenRouter) => {
            if blank(&options.openrouter_api_key) {
                return Err(s::AnalyzeImageService_Missing_OpenRouter_API_key().to_owned());
            }
            Ok(get(
                "https://openrouter.ai/api/v1/models".to_owned(),
                vec![bearer(Some(options.openrouter_api_key.as_str().trim()))],
            ))
        }
        None => Err(s::AnalyzeImageService_Select_provider_first().to_owned()),
    }
}

/// `AnalyzeImageConnectionResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionResult {
    pub success: bool,
    pub message: String,
}

/// `TestConnectionAsync`'s result for a response.
pub fn connection_result(response: &AIResponse) -> ConnectionResult {
    if response.is_success() {
        return ConnectionResult { success: true, message: s::AnalyzeImageService_Connection_OK().to_owned() };
    }
    let mut summary =
        if response.reason.trim().is_empty() { response.status_code.to_string() } else { response.reason.clone() };
    if !response.text.trim().is_empty() {
        summary.push_str(": ");
        let units: Vec<u16> = response.text.encode_utf16().collect();
        if units.len() > 200 {
            summary.push_str(&String::from_utf16_lossy(&units[..200]));
            summary.push_str("...");
        } else {
            summary.push_str(&response.text);
        }
    }
    ConnectionResult { success: false, message: summary }
}

/// `AnalyzeImageService.TestConnectionAsync`: `Err` when the request
/// couldn't be sent (the exception the options window shows).
pub fn test_connection(options: &AIOptions) -> Result<ConnectionResult, String> {
    let request = match build_models_request(options) {
        Ok(r) => r,
        Err(e) => return Ok(ConnectionResult { success: false, message: e }),
    };
    Ok(connection_result(&send(&request)?))
}

/// `LoadModelsAsync`'s reading of a response: the `id`s under `data`, blank
/// ones left out, sorted ignoring case.
pub fn parse_models(response: &AIResponse) -> Result<Vec<String>, String> {
    response.ensure_success()?;
    let root = stj_parse(&response.text)?;
    let data = element_property(&root, "data")?;
    let Value::Array(items) = data else {
        return Err(element_type_error("Array", data));
    };
    let mut ids = Vec::new();
    for item in items {
        let id = element_property(item, "id")?;
        match id {
            Value::String(s) if !s.trim().is_empty() => ids.push(s.clone()),
            Value::String(_) | Value::Null => {}
            other => return Err(element_type_error("String", other)),
        }
    }
    // `OrderBy(x => x, StringComparer.OrdinalIgnoreCase)`: stable.
    ids.sort_by(|a, b| ordinal_ignore_case(a, b));
    Ok(ids)
}

/// `AnalyzeImageService.LoadModelsAsync`: OpenAI's models (none for the
/// other providers, without a request).
pub fn load_models(options: &AIOptions) -> Result<Vec<String>, String> {
    if !matches!(options.provider, Some(AIProvider::OpenAI | AIProvider::OpenAILegacy)) {
        return Ok(Vec::new());
    }
    let request = build_models_request(options)?;
    parse_models(&send(&request)?)
}

// ---------------------------------------------------------------------------
// System.Text.Json as the OpenAI providers and the service use it.

/// `Utf8JsonWriter` with the default `JavaScriptEncoder`: compact, every
/// character outside Basic Latin and the HTML-sensitive ones escaped.
#[derive(Default)]
struct StjWriter {
    out: String,
    /// Whether the current container already has a value.
    stack: Vec<bool>,
    after_name: bool,
}

impl StjWriter {
    fn separator(&mut self) {
        if self.after_name {
            self.after_name = false;
            return;
        }
        if let Some(has) = self.stack.last_mut() {
            if *has {
                self.out.push(',');
            }
            *has = true;
        }
    }

    fn begin_object(&mut self) {
        self.separator();
        self.out.push('{');
        self.stack.push(false);
    }

    fn end_object(&mut self) {
        self.stack.pop();
        self.out.push('}');
    }

    fn begin_array(&mut self) {
        self.separator();
        self.out.push('[');
        self.stack.push(false);
    }

    fn end_array(&mut self) {
        self.stack.pop();
        self.out.push(']');
    }

    fn name(&mut self, name: &str) {
        self.separator();
        self.out.push_str(&stj_string(name));
        self.out.push(':');
        self.after_name = true;
    }

    fn raw(&mut self, value: &str) {
        self.separator();
        self.out.push_str(value);
    }

    fn prop_str(&mut self, name: &str, value: Option<&str>) {
        self.name(name);
        match value {
            Some(v) => self.raw(&stj_string(v)),
            None => self.raw("null"),
        }
    }

    fn finish(self) -> String {
        self.out
    }
}

/// A JSON string as `JavaScriptEncoder.Default` writes it.
pub fn stj_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\\' => out.push_str("\\\\"),
            ' ' | '!' | '#'..='%' | '('..='*' | ','..=';' | '=' | '?'..='[' | ']'..='_' | 'a'..='~' => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{u:04X}"));
                }
            }
        }
    }
    out.push('"');
    out
}

/// `JsonSerializer.Deserialize`/`JsonDocument.Parse` input: strict JSON.
fn stj_parse(text: &str) -> Result<Value, String> {
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Err("The input does not contain any JSON tokens. Expected the input to start with a valid JSON token, while reading the first token. Path: $ | LineNumber: 0 | BytePositionInLine: 0.".to_owned());
    }
    serde_json::from_str(text).map_err(|e| format!("{e}"))
}

/// The shape of a class `JsonSerializer` fills: each known property must
/// be null or of its type, or the read throws.
#[derive(Clone, Copy)]
enum Kind {
    Str,
    Object(&'static [(&'static str, Kind)]),
    Array(&'static Kind),
}

const CONTENT_ITEM: Kind = Kind::Object(&[("type", Kind::Str), ("text", Kind::Str)]);
const CONTENT: Kind = Kind::Array(&CONTENT_ITEM);
const OUTPUT_ITEM: Kind = Kind::Object(&[("id", Kind::Str), ("type", Kind::Str), ("content", CONTENT)]);
const OUTPUT: Kind = OUTPUT_ITEM;
const MESSAGE: Kind = Kind::Object(&[("role", Kind::Str), ("content", Kind::Str)]);
const CHOICE: Kind = Kind::Object(&[("message", MESSAGE)]);

/// Checks `value` against `kind`; the exception's message when it doesn't fit.
fn stj_check(value: &Value, kind: Kind, path: &str) -> Result<(), String> {
    let fail = |t: &str| Err(format!("The JSON value could not be converted to {t}. Path: {path}"));
    match (kind, value) {
        (_, Value::Null) => Ok(()),
        (Kind::Str, Value::String(_)) => Ok(()),
        (Kind::Str, _) => fail("System.String"),
        (Kind::Object(props), Value::Object(map)) => {
            for (name, kind) in props {
                // Property names are matched exactly; the last duplicate wins.
                if let Some(v) = map.get(*name) {
                    stj_check(v, *kind, &format!("{path}.{name}"))?;
                }
            }
            Ok(())
        }
        (Kind::Object(_), _) => fail("an object"),
        (Kind::Array(item), Value::Array(items)) => {
            for (i, v) in items.iter().enumerate() {
                stj_check(v, *item, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        (Kind::Array(_), _) => fail("an array"),
    }
}

/// Deserialises the root object; `None` for `null`.
fn stj_object<'a>(root: &'a Value, props: &'static [(&'static str, Kind)]) -> Result<Option<&'a Map<String, Value>>, String> {
    stj_check(root, Kind::Object(props), "$")?;
    Ok(root.as_object())
}

/// `JsonElement.GetProperty(name)`.
fn element_property<'a>(element: &'a Value, name: &str) -> Result<&'a Value, String> {
    match element {
        Value::Object(map) => map.get(name).ok_or_else(|| format!("The given key '{name}' was not present in the dictionary.")),
        other => Err(element_type_error("Object", other)),
    }
}

/// `JsonElement`'s `InvalidOperationException` for the wrong kind.
fn element_type_error(wanted: &str, got: &Value) -> String {
    let kind = match got {
        Value::Null => "Null",
        Value::Bool(true) => "True",
        Value::Bool(false) => "False",
        Value::Number(_) => "Number",
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
    };
    format!("The requested operation requires an element of type '{wanted}', but the target element has type '{kind}'.")
}

// ---------------------------------------------------------------------------
// Newtonsoft's `dynamic` as Gemini's and OpenRouter's providers read answers.

enum Step {
    Member(&'static str),
    Index(usize),
}

/// `responseObject.a[0].b...` on a `JToken` through `dynamic`, then the
/// conversion to `string`.
fn dynamic_string(root: &Value, path: &[Step]) -> Result<Option<String>, String> {
    let type_name = |v: &Value| match v {
        Value::Object(_) => "Newtonsoft.Json.Linq.JObject",
        Value::Array(_) => "Newtonsoft.Json.Linq.JArray",
        _ => "Newtonsoft.Json.Linq.JValue",
    };
    let mut current = Some(root);
    for step in path {
        let Some(value) = current else {
            return Err("Cannot perform runtime binding on a null reference".to_owned());
        };
        current = match (step, value) {
            // A JSON null is a `JValue` whose members can't be bound either.
            (_, Value::Null) => return Err("Cannot perform runtime binding on a null reference".to_owned()),
            (Step::Member(name), Value::Object(map)) => map.get(*name),
            (Step::Member(name), other) => {
                return Err(format!("'{}' does not contain a definition for '{name}'", type_name(other)));
            }
            (Step::Index(i), Value::Array(items)) => Some(items.get(*i).ok_or_else(|| {
                "Index was out of range. Must be non-negative and less than the size of the collection. (Parameter 'index')"
                    .to_owned()
            })?),
            (Step::Index(i), Value::Object(_)) => {
                return Err(format!("Accessed JObject values with invalid key value: {i}. Object property name expected."));
            }
            (Step::Index(_), other) => return Err(format!("Cannot access child value on {}.", type_name(other))),
        };
    }
    match current {
        None | Some(Value::Null) => Ok(None),
        Some(v @ (Value::String(_) | Value::Number(_) | Value::Bool(_))) => {
            sharex_uploaders::json::to_dotnet_string(v).map_err(|e| e.to_string())
        }
        Some(other) => Err(format!(
            "Cannot implicitly convert type '{}' to 'string'. An explicit conversion exists (are you missing a cast?)",
            type_name(other)
        )),
    }
}

// ---------------------------------------------------------------------------
// Strings.

/// `string.StartsWith(value, StringComparison.OrdinalIgnoreCase)`.
fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = prefix.chars().collect();
    p.len() <= t.len() && t.iter().zip(&p).all(|(a, b)| upper(*a) == upper(*b))
}

fn upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// `StringComparer.OrdinalIgnoreCase.Compare`.
fn ordinal_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let a: Vec<u16> = a.chars().map(upper).collect::<String>().encode_utf16().collect();
    let b: Vec<u16> = b.chars().map(upper).collect::<String>().encode_utf16().collect();
    a.cmp(&b)
}

/// `Uri.EscapeDataString`: everything but the unreserved characters, as
/// upper-case `%XX` of UTF-8.
pub fn escape_data_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_strings_as_system_text_json() {
        assert_eq!(stj_string("a\"b\\c<>&'+`\n\u{7f}\u{e9}\u{1f600}/"), "\"a\\u0022b\\\\c\\u003C\\u003E\\u0026\\u0027\\u002B\\u0060\\n\\u007F\\u00E9\\uD83D\\uDE00/\"");
    }

    #[test]
    fn options_round_trip() {
        let v = json!({
            "Provider": "Gemini",
            "OpenAIAPIKey": null,
            "OpenAIModel": "gpt-5.1",
            "GeminiAPIKey": "plain-key",
            "Future": 1
        });
        let o = AIOptions::from_json(&v);
        assert_eq!(o.provider, Some(AIProvider::Gemini));
        assert_eq!(o.gemini_api_key.get(), Some("plain-key"));
        assert!(o.has_api_key());
        assert_eq!(o.openai_verbosity.as_deref(), Some("medium"));
        let back = o.to_json();
        let keys: Vec<&str> = back.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(&keys[..3], ["Provider", "OpenAIAPIKey", "OpenAIModel"]);
        assert_eq!(keys.last(), Some(&"Future"));
        assert_eq!(back["GeminiAPIKey"], "plain-key");
        assert_eq!(AIOptions::from_json(&json!({})).to_json(), AIOptions::default().to_json());
        let unknown = AIOptions::from_json(&json!({"Provider": "Claude"}));
        assert_eq!(unknown.provider, None);
        assert_eq!(unknown.to_json()["Provider"], "Claude");
        assert!(build_analyze_request(&unknown, AnalyzeInput::Data(b"x")).is_err());
    }

    #[test]
    fn urls() {
        assert_eq!(openai_url(None, "/v1/responses"), "https://api.openai.com/v1/responses");
        assert_eq!(openai_url(Some("http://x/"), "/v1/responses"), "http://x/v1/responses");
        assert_eq!(openai_url(Some("http://x/v1/responses"), "/v1/responses"), "http://x/v1/responses");
        assert_eq!(escape_data_string("a b+c/é~"), "a%20b%2Bc%2F%C3%A9~");
    }

    #[test]
    fn reads_answers() {
        let ok = |text: &str| AIResponse { status_code: 200, reason: "OK".into(), text: text.into() };
        let r = parse_analyze_response(AIProvider::OpenAI, &ok(r#"{"output":[{"type":"reasoning","content":[]},{"content":[{"type":"output_text","text":"A cat."}]}]}"#));
        assert_eq!(r.unwrap().as_deref(), Some("A cat."));
        assert_eq!(parse_analyze_response(AIProvider::OpenAI, &ok(r#"{"output":null}"#)).unwrap().as_deref(), Some(""));
        assert!(parse_analyze_response(AIProvider::OpenAI, &ok("null")).is_err());
        assert!(parse_analyze_response(AIProvider::OpenAI, &ok(r#"{"id":5}"#)).is_err());
        let unauthorized = AIResponse { status_code: 401, reason: "Unauthorized".into(), text: "{}".into() };
        assert_eq!(
            parse_analyze_response(AIProvider::OpenAI, &unauthorized).unwrap_err(),
            "Response status code does not indicate success: 401 (Unauthorized)."
        );
        assert_eq!(parse_analyze_response(AIProvider::Gemini, &unauthorized).unwrap_err(), "Error from Gemini API: {}");
        assert_eq!(
            parse_analyze_response(AIProvider::Gemini, &ok(r#"{"candidates":[{"content":{"parts":[{"text":"Hi"}]}}]}"#)).unwrap().as_deref(),
            Some("Hi")
        );
        assert_eq!(
            parse_analyze_response(AIProvider::Gemini, &ok(r#"{}"#)).unwrap_err(),
            "Cannot perform runtime binding on a null reference"
        );
        assert_eq!(
            parse_analyze_response(AIProvider::OpenRouter, &ok(r#"{"choices":[{"message":{"content":"x"}}]}"#)).unwrap().as_deref(),
            Some("x")
        );
        assert_eq!(result_text(Some("a\nb\r\nc\rd")), "a\r\nb\r\nc\r\nd");
        assert_eq!(result_text(None), "");
    }

    #[test]
    fn connection_summaries() {
        let r = AIResponse { status_code: 401, reason: "Unauthorized".into(), text: "x".repeat(250) };
        let c = connection_result(&r);
        assert!(!c.success);
        assert_eq!(c.message, format!("Unauthorized: {}...", "x".repeat(200)));
        let models = AIResponse { status_code: 200, reason: "OK".into(), text: r#"{"data":[{"id":"b"},{"id":"A"},{"id":" "},{"id":null},{"id":"a"}]}"#.into() };
        assert_eq!(parse_models(&models).unwrap(), ["A", "a", "b"]);
        let o = AIOptions::default();
        assert_eq!(build_models_request(&o).unwrap_err(), "Missing OpenAI API key.");
    }
}
