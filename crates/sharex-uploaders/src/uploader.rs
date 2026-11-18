//! `BaseUploaders/Uploader.cs` and its subclasses (`GenericUploader`,
//! `FileUploader`, `ImageUploader`, `TextUploader`, `URLShortener`,
//! `URLSharer`): the request helpers every destination uses, error
//! reporting, progress and stopping.
//!
//! ShareX's helpers return `null` (and add an error) when a request fails;
//! here they return `Ok(None)`. An exception a destination throws is
//! [`UploadError::Exception`]; a stop is [`UploadError::Cancelled`], which
//! `RunOperationAsync` turns into a `null` result ([`run_operation`]).

use std::io::{Read, Seek};
use std::sync::Arc;
use std::time::Duration;

use sharex_helpers::mime_types;
use sharex_strings::uploaders_lib as strings;

use crate::enums::HttpMethod;
use crate::http::{
    self, CancellationToken, Content, HttpError, HttpRequest, MultipartPart, PartBody, ProgressCallback, RequestOptions, ResponseInfo,
    SharedStream, StreamPart,
};
use crate::result::{UploadResult, UploaderErrorManager};
use crate::url_helpers;

/// What a destination's code can end with besides a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadError {
    /// The upload was stopped.
    Cancelled,
    /// An exception: its message.
    Exception(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::Cancelled => f.write_str("The operation was canceled."),
            UploadError::Exception(m) => f.write_str(m),
        }
    }
}

impl From<String> for UploadError {
    fn from(s: String) -> Self {
        UploadError::Exception(s)
    }
}

impl From<&str> for UploadError {
    fn from(s: &str) -> Self {
        UploadError::Exception(s.to_owned())
    }
}

pub type UResult<T> = Result<T, UploadError>;

/// Builds an exception.
pub fn exception<T>(message: impl Into<String>) -> UResult<T> {
    Err(UploadError::Exception(message.into()))
}

/// `Dictionary<string, string>` arguments, in insertion order.
pub type Args = Vec<(String, String)>;

/// Builds [`Args`]: `args![("key", value), ...]`.
#[macro_export]
macro_rules! args {
    ($(($k:expr, $v:expr)),* $(,)?) => {
        vec![$(($k.to_string(), $v.to_string())),*]
    };
}

/// `NameValueCollection` of headers: names compared ignoring case, several
/// values per name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Headers(pub Vec<(String, Vec<String>)>);

impl Headers {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Add(name, value)`.
    pub fn add(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self.0.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
            Some((_, values)) => values.push(value),
            None => self.0.push((name.to_owned(), vec![value])),
        }
    }

    /// `this[name] = value`.
    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self.0.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
            Some((_, values)) => *values = vec![value],
            None => self.0.push((name.to_owned(), vec![value])),
        }
    }

    /// `this[name]`: the values joined with `,`.
    pub fn get(&self, name: &str) -> Option<String> {
        self.0.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.join(","))
    }

    pub fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.add(name, value);
        self
    }
}

/// `RequestHelpers.CreateAuthenticationHeader`: HTTP basic authentication.
pub fn create_authentication_header(username: &str, password: &str) -> Headers {
    let auth = text_to_base64(&format!("{username}:{password}"));
    Headers::new().with("Authorization", format!("Basic {auth}"))
}

/// `TranslatorHelper.TextToBase64`.
pub fn text_to_base64(text: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

/// `Uploader.EarlyURLCopyRequested`.
pub type EarlyUrlCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// `Uploader`: the state and request helpers every destination shares.
pub struct Uploader {
    /// `Errors`, titled `DefaultTitle`.
    pub errors: UploaderErrorManager,
    /// `BufferSize` (ShareX sets 2^`BufferSizePower` KB per task).
    pub buffer_size: usize,
    /// `RequestTimeout` (`None`: infinite).
    pub request_timeout: Option<Duration>,
    pub allow_auto_redirect: bool,
    /// `AllowReportProgress`.
    pub allow_report_progress: bool,
    /// `ReturnResponseOnError`.
    pub return_response_on_error: bool,
    /// `LastResponseInfo`.
    pub last_response_info: Option<ResponseInfo>,
    progress: Option<ProgressCallback>,
    early_url: Option<EarlyUrlCallback>,
    cancel: CancellationToken,
    is_uploading: bool,
}

impl Default for Uploader {
    fn default() -> Self {
        Self {
            errors: UploaderErrorManager::default(),
            buffer_size: 8192,
            request_timeout: None,
            allow_auto_redirect: true,
            allow_report_progress: true,
            return_response_on_error: false,
            last_response_info: None,
            progress: None,
            early_url: None,
            cancel: CancellationToken::new(),
            is_uploading: false,
        }
    }
}

/// `CreateStatusCodeException`'s message.
pub fn status_code_message(status: u16, reason: &str) -> String {
    format!("Response status code does not indicate success: {status} ({reason}).")
}

impl Uploader {
    pub fn new() -> Self {
        Self::default()
    }

    /// `IsUploading`.
    pub fn is_uploading(&self) -> bool {
        self.is_uploading
    }

    /// `IsError`.
    pub fn is_error(&self) -> bool {
        !self.cancel.is_cancelled() && !self.errors.is_empty()
    }

    /// `ToErrorString()`.
    pub fn to_error_string(&self) -> String {
        if self.is_error() { self.errors.errors.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join(crate::NEW_LINE) } else { String::new() }
    }

    /// `ProgressChanged +=`.
    pub fn on_progress(&mut self, callback: ProgressCallback) {
        self.progress = Some(callback);
    }

    /// The progress callback, for destinations that report progress
    /// themselves (S3's multipart upload).
    pub fn progress_callback(&self) -> Option<ProgressCallback> {
        if self.allow_report_progress { self.progress.clone() } else { None }
    }

    /// A second uploader for requests that run beside this one's (S3's
    /// parallel parts): same settings and stop handle, its own errors.
    pub fn fork(&self) -> Uploader {
        Uploader {
            buffer_size: self.buffer_size,
            request_timeout: self.request_timeout,
            allow_auto_redirect: self.allow_auto_redirect,
            allow_report_progress: self.allow_report_progress,
            return_response_on_error: self.return_response_on_error,
            cancel: self.cancel.clone(),
            is_uploading: true,
            ..Uploader::default()
        }
    }

    /// `EarlyURLCopyRequested +=`.
    pub fn on_early_url_copy(&mut self, callback: EarlyUrlCallback) {
        self.early_url = Some(callback);
    }

    /// `OnEarlyURLCopyRequested`.
    pub fn early_url_copy_requested(&self, url: &str) {
        if !url.is_empty()
            && let Some(cb) = &self.early_url
        {
            cb(url);
        }
    }

    /// A handle that stops this uploader from any thread (`StopUpload()`).
    pub fn stop_handle(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// `StopUploadRequested`.
    pub fn stop_requested(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn options(&self, read_body: bool) -> RequestOptions {
        RequestOptions {
            allow_auto_redirect: self.allow_auto_redirect,
            timeout: self.request_timeout,
            cancel: self.cancel.clone(),
            progress: if self.allow_report_progress { self.progress.clone() } else { None },
            read_body,
            factory_client: true,
        }
    }

    /// `CreateStreamContent`: a slice of `data` sent with progress.
    pub fn stream_content(&self, data: &SharedStream, position: u64, length: u64, content_type: Option<&str>) -> UResult<Content> {
        let total = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        Ok(Content::Stream {
            part: StreamPart { source: data.clone(), position, length, total_length: total, buffer_size: self.buffer_size, on_read: None },
            content_type: content_type.filter(|c| !c.trim().is_empty()).map(str::to_owned),
            headers: Vec::new(),
        })
    }

    /// `CreateMultipartFormDataContent(args)` parts.
    fn form_parts(args: &[(String, String)]) -> Vec<MultipartPart> {
        args.iter()
            .filter(|(k, _)| !k.is_empty())
            .map(|(k, v)| MultipartPart {
                headers: vec![("Content-Disposition".into(), format!("form-data; name={}", quote_header_value(k)))],
                body: PartBody::Bytes(v.as_bytes().to_vec()),
            })
            .collect()
    }

    /// `CreateMultipartFormDataContent(args)`.
    pub fn multipart_form_data(args: &[(String, String)]) -> Content {
        Content::Multipart {
            content_type: http::CONTENT_TYPE_MULTIPART_FORM_DATA.into(),
            boundary: http::create_boundary(),
            parts: Self::form_parts(args),
            headers: Vec::new(),
        }
    }

    /// `CreateFileMultipartContent`: the arguments, then the file.
    pub fn file_multipart_content(&self, data: &SharedStream, file_name: &str, file_form_name: &str, args: &[(String, String)]) -> UResult<Content> {
        let mut parts = Self::form_parts(args);
        let total = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let mime = mime_types::get_mime_type_from_file_name(file_name);
        parts.push(MultipartPart {
            headers: vec![
                ("Content-Type".into(), mime),
                (
                    "Content-Disposition".into(),
                    format!("form-data; name={}; filename={}", quote_header_value(file_form_name), quote_header_value(file_name)),
                ),
            ],
            body: PartBody::Stream(StreamPart { source: data.clone(), position: 0, length: total, total_length: total, buffer_size: self.buffer_size, on_read: None }),
        });
        Ok(Content::Multipart { content_type: http::CONTENT_TYPE_MULTIPART_FORM_DATA.into(), boundary: http::create_boundary(), parts, headers: Vec::new() })
    }

    /// `CreateRelatedMultipartContent`: JSON metadata, then the file.
    pub fn related_multipart_content(&self, data: &SharedStream, file_name: &str, related_data: &str) -> UResult<Content> {
        let total = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let parts = vec![
            MultipartPart {
                headers: vec![("Content-Type".into(), "application/json; charset=UTF-8".into())],
                body: PartBody::Bytes(related_data.as_bytes().to_vec()),
            },
            MultipartPart {
                headers: vec![("Content-Type".into(), mime_types::get_mime_type_from_file_name(file_name))],
                body: PartBody::Stream(StreamPart { source: data.clone(), position: 0, length: total, total_length: total, buffer_size: self.buffer_size, on_read: None }),
            },
        ];
        Ok(Content::Multipart { content_type: "multipart/related".into(), boundary: http::create_boundary(), parts, headers: Vec::new() })
    }

    /// `GetResponseAsync(method, url, content, args, headers, cookies,
    /// allowNon2xxResponses)`: the response, or `None` after adding the error.
    #[allow(clippy::too_many_arguments)]
    pub fn get_response(
        &mut self,
        method: HttpMethod,
        url: &str,
        content: Option<Content>,
        args: &[(String, String)],
        headers: Option<&Headers>,
        cookies: &[(String, String)],
        allow_non_2xx: bool,
    ) -> UResult<Option<ResponseInfo>> {
        self.get_response_ex(method, url, content, args, headers, cookies, allow_non_2xx, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_response_ex(
        &mut self,
        method: HttpMethod,
        url: &str,
        content: Option<Content>,
        args: &[(String, String)],
        headers: Option<&Headers>,
        cookies: &[(String, String)],
        allow_non_2xx: bool,
        read_body: bool,
    ) -> UResult<Option<ResponseInfo>> {
        let url = url_helpers::create_query_string_url(url, args, false);
        let request = create_request(method, &url, content, None, headers, cookies);
        self.last_response_info = None;
        let was_uploading = self.is_uploading;
        if !was_uploading {
            self.is_uploading = true;
        }
        let result = http::send(request, self.options(read_body));
        if !was_uploading {
            self.is_uploading = false;
        }
        match result {
            Ok(info) => {
                self.last_response_info = Some(info.clone());
                if !info.is_success() && !allow_non_2xx {
                    let message = status_code_message(info.status_code, &info.status_description);
                    self.process_error(&message, &url, Some(&info));
                    return Ok(None);
                }
                Ok(Some(info))
            }
            Err(HttpError::Cancelled) => Err(UploadError::Cancelled),
            Err(HttpError::Timeout(m)) | Err(HttpError::Failed(m)) => {
                self.process_error(&m, &url, None);
                Ok(None)
            }
        }
    }

    /// `SendRequestAsync(method, url, args, headers, cookies)`: no body.
    pub fn send_request(&mut self, method: HttpMethod, url: &str, args: &[(String, String)], headers: Option<&Headers>) -> UResult<Option<String>> {
        Ok(self.get_response(method, url, None, args, headers, &[], false)?.and_then(|r| r.response_text))
    }

    /// `SendRequestAsync(method, url, string content, contentType, args, headers, cookies)`.
    pub fn send_request_text(
        &mut self,
        method: HttpMethod,
        url: &str,
        content: &str,
        content_type: Option<&str>,
        args: &[(String, String)],
        headers: Option<&Headers>,
    ) -> UResult<Option<String>> {
        let body = Content::bytes(content.as_bytes().to_vec(), content_type);
        Ok(self.get_response(method, url, Some(body), args, headers, &[], false)?.and_then(|r| r.response_text))
    }

    /// `SendRequestAsync(method, url, Stream data, contentType, args, headers, cookies)`.
    pub fn send_request_stream(
        &mut self,
        method: HttpMethod,
        url: &str,
        data: &SharedStream,
        content_type: Option<&str>,
        args: &[(String, String)],
        headers: Option<&Headers>,
    ) -> UResult<Option<String>> {
        let len = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let body = self.stream_content(data, 0, len, content_type)?;
        Ok(self.get_response(method, url, Some(body), args, headers, &[], false)?.and_then(|r| r.response_text))
    }

    /// `SendRequestURLEncodedAsync`.
    pub fn send_request_url_encoded(&mut self, method: HttpMethod, url: &str, args: &[(String, String)], headers: Option<&Headers>) -> UResult<Option<String>> {
        let query = url_helpers::create_query_string(args, false);
        self.send_request_text(method, url, &query, Some(http::CONTENT_TYPE_URL_ENCODED), &[], headers)
    }

    /// `SendRequestMultiPartAsync`.
    pub fn send_request_multipart(&mut self, url: &str, args: &[(String, String)], headers: Option<&Headers>, method: HttpMethod) -> UResult<Option<String>> {
        let content = Self::multipart_form_data(args);
        Ok(self.get_response(method, url, Some(content), &[], headers, &[], false)?.and_then(|r| r.response_text))
    }

    /// `SendRequestFileAsync`: a multipart upload of `data` (or, with
    /// `related_data`, a `multipart/related` one).
    #[allow(clippy::too_many_arguments)]
    pub fn send_request_file(
        &mut self,
        url: &str,
        data: &SharedStream,
        file_name: &str,
        file_form_name: &str,
        args: &[(String, String)],
        headers: Option<&Headers>,
        method: HttpMethod,
        content_type: &str,
        related_data: Option<&str>,
    ) -> UResult<UploadResult> {
        let mut content = match related_data {
            None => self.file_multipart_content(data, file_name, file_form_name, args)?,
            Some(r) => self.related_multipart_content(data, file_name, r)?,
        };
        if let Content::Multipart { content_type: ct, .. } = &mut content {
            *ct = content_type.to_owned();
        }
        let response = self.get_response(method, url, Some(content), &[], headers, &[], false)?;
        let mut result = UploadResult {
            response_info: response.clone().or_else(|| self.last_response_info.clone()),
            response: response.as_ref().and_then(|r| r.response_text.clone()),
            ..Default::default()
        };
        if response.is_some() {
            result.success = true;
        } else if self.return_response_on_error {
            result.response = self.last_response_info.as_ref().and_then(|r| r.response_text.clone());
        }
        Ok(result)
    }

    /// [`Self::send_request_file`] with ShareX's defaults (POST,
    /// `multipart/form-data`).
    pub fn send_request_file_simple(
        &mut self,
        url: &str,
        data: &SharedStream,
        file_name: &str,
        file_form_name: &str,
        args: &[(String, String)],
        headers: Option<&Headers>,
    ) -> UResult<UploadResult> {
        self.send_request_file(url, data, file_name, file_form_name, args, headers, HttpMethod::POST, http::CONTENT_TYPE_MULTIPART_FORM_DATA, None)
    }

    /// `SendRequestFileRangeAsync`: part of a stream with a `Content-Range`.
    #[allow(clippy::too_many_arguments)]
    pub fn send_request_file_range(
        &mut self,
        url: &str,
        data: &SharedStream,
        file_name: &str,
        content_position: u64,
        content_length: Option<u64>,
        args: &[(String, String)],
        headers: Option<&Headers>,
        method: HttpMethod,
    ) -> UResult<UploadResult> {
        let data_length = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        if content_position > data_length {
            return exception("Specified argument was out of the range of valid values. (Parameter 'contentPosition')");
        }
        let length = content_length.unwrap_or(data_length - content_position).min(data_length - content_position);
        let mime = mime_types::get_mime_type_from_file_name(file_name);
        let mut content = self.stream_content(data, content_position, length, Some(&mime))?;
        if length > 0 {
            content
                .extra_headers_mut()
                .push(("Content-Range".into(), format!("bytes {}-{}/{}", content_position, content_position + length - 1, data_length)));
        }
        let response = self.get_response(method, url, Some(content), args, headers, &[], false)?;
        let mut result = UploadResult {
            success: response.is_some(),
            response_info: response.clone().or_else(|| self.last_response_info.clone()),
            response: response.as_ref().and_then(|r| r.response_text.clone()),
            ..Default::default()
        };
        if response.is_none() && self.return_response_on_error {
            result.response = self.last_response_info.as_ref().and_then(|r| r.response_text.clone());
        }
        Ok(result)
    }

    /// `GetResponseAsync(method, url, Stream data, contentType, args,
    /// headers, cookies, allowNon2xxResponses)`: the stream (if any) sent
    /// whole with progress.
    #[allow(clippy::too_many_arguments)]
    pub fn get_response_stream(
        &mut self,
        method: HttpMethod,
        url: &str,
        data: Option<&SharedStream>,
        content_type: Option<&str>,
        args: &[(String, String)],
        headers: Option<&Headers>,
        allow_non_2xx: bool,
    ) -> UResult<Option<ResponseInfo>> {
        let content = match data {
            Some(d) => {
                let len = http::stream_length(d).map_err(|e| UploadError::Exception(e.to_string()))?;
                Some(self.stream_content(d, 0, len, content_type)?)
            }
            None => None,
        };
        self.get_response(method, url, content, args, headers, &[], allow_non_2xx)
    }

    /// `CreateRangeStreamContent`: a checked slice of the stream.
    fn range_stream_content(&self, data: &SharedStream, position: u64, length: u64, content_type: Option<&str>) -> UResult<Content> {
        let total = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        if position > total {
            return exception("The requested upload position must be within the stream. (Parameter 'position')");
        }
        if length > total - position {
            return exception("The requested upload range must be within the stream. (Parameter 'length')");
        }
        self.stream_content(data, position, length, content_type)
    }

    /// `SendRequestAsync(method, url, Stream data, position, length,
    /// contentType, args, headers)`.
    #[allow(clippy::too_many_arguments)]
    pub fn send_request_stream_range(
        &mut self,
        method: HttpMethod,
        url: &str,
        data: &SharedStream,
        position: u64,
        length: u64,
        content_type: Option<&str>,
        args: &[(String, String)],
        headers: Option<&Headers>,
    ) -> UResult<Option<String>> {
        let content = self.range_stream_content(data, position, length, content_type)?;
        Ok(self.get_response(method, url, Some(content), args, headers, &[], false)?.and_then(|r| r.response_text))
    }

    /// `SendRequestForResponseAsync(method, url, data, position, length,
    /// contentType, headers)`.
    #[allow(clippy::too_many_arguments)]
    pub fn send_request_for_response_range(
        &mut self,
        method: HttpMethod,
        url: &str,
        data: &SharedStream,
        position: u64,
        length: u64,
        content_type: Option<&str>,
        headers: Option<&Headers>,
    ) -> UResult<Option<ResponseInfo>> {
        let content = self.range_stream_content(data, position, length, content_type)?;
        self.get_response(method, url, Some(content), &[], headers, &[], false)
    }

    /// `TransferData(dataStream, destinationStream, dataPosition,
    /// dataLength)`: copies the stream in `BufferSize` blocks with progress;
    /// `false` when stopped or short.
    pub fn transfer_data(&mut self, data: &SharedStream, destination: &mut dyn std::io::Write, position: u64, length: Option<u64>) -> UResult<bool> {
        let io = |e: std::io::Error| UploadError::Exception(e.to_string());
        let source_length = http::stream_length(data).map_err(io)?;
        if position >= source_length {
            return Ok(true);
        }
        let length = length.unwrap_or(source_length - position).min(source_length - position);
        let mut progress = http::ProgressManager::new(source_length, position);
        let mut buffer = vec![0u8; (self.buffer_size as u64).min(length).max(1) as usize];
        let mut remaining = length;
        let callback = self.progress_callback();
        let mut stream = data.lock().expect("upload stream");
        stream.seek(std::io::SeekFrom::Start(position)).map_err(io)?;
        while !self.stop_requested() && remaining > 0 {
            let want = (buffer.len() as u64).min(remaining) as usize;
            let read = stream.read(&mut buffer[..want]).map_err(io)?;
            if read == 0 {
                break;
            }
            destination.write_all(&buffer[..read]).map_err(io)?;
            remaining -= read as u64;
            if progress.update_progress(read as u64)
                && let Some(cb) = &callback
            {
                cb(&progress);
            }
        }
        Ok(!self.stop_requested() && remaining == 0)
    }

    /// `ProcessError`: adds the error text ShareX shows in its error window.
    pub fn process_error(&mut self, message: &str, request_url: &str, response: Option<&ResponseInfo>) {
        let nl = crate::NEW_LINE;
        let mut sb = String::new();
        let mut line = |s: &str| {
            sb.push_str(s);
            sb.push_str(nl);
        };
        line(strings::Uploader_Error_message());
        line(message);
        if !request_url.is_empty() {
            line("");
            line(strings::Uploader_Request_URL());
            line(request_url);
        }
        if let Some(r) = response {
            line("");
            line(&format!("{}:", strings::ResponseWindow_Status_code()));
            line(&format!("({}) {}", r.status_code, r.status_description));
            if let Some(u) = r.response_url.as_deref().filter(|u| !u.is_empty() && *u != request_url) {
                line("");
                line(&format!("{}:", strings::ResponseWindow_Response_URL()));
                line(u);
            }
            line("");
            line(&format!("{}:", strings::ResponseWindow_Headers()));
            line(r.headers_to_string().trim_end());
            line("");
            line(&format!("{}:", strings::ResponseWindow_Response_text()));
            line(r.response_text.as_deref().unwrap_or(""));
        }
        line("");
        sb.push_str(strings::Uploader_Stack_trace());
        sb.push_str(nl);
        log::write(&format!("Error:\r\n{sb}"));
        self.errors.add(sb);
    }
}

/// `QuoteHeaderValue`.
pub fn quote_header_value(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"").replace('\r', "%0D").replace('\n', "%0A"))
}

/// `Uploader.CreateRequest`: the content (an empty one when only content
/// headers are given), the headers sorted into request and content
/// headers, and the cookies.
pub fn create_request(
    method: HttpMethod,
    url: &str,
    mut content: Option<Content>,
    content_type: Option<&str>,
    headers: Option<&Headers>,
    cookies: &[(String, String)],
) -> HttpRequest {
    let has_content_headers = headers.is_some_and(|h| h.0.iter().any(|(k, _)| k.len() >= 8 && k[..8].eq_ignore_ascii_case("Content-")));
    if content.is_none() && (content_type.is_some_and(|c| !c.is_empty()) || has_content_headers) {
        content = Some(Content::bytes(Vec::new(), None));
    }
    if let (Some(c), Some(ct)) = (content.as_mut(), content_type.filter(|c| !c.trim().is_empty())) {
        set_content_type(c, ct);
    }
    let mut request_headers: Vec<(String, String)> = Vec::new();
    if let Some(h) = headers {
        for (name, values) in &h.0 {
            if name.is_empty() || name.eq_ignore_ascii_case("Cookie") {
                continue;
            }
            let value = values.join(", ");
            if http::is_content_header(name) {
                if let Some(c) = content.as_mut() {
                    add_content_header(c, name, &value);
                }
            } else if is_valid_header_name(name) {
                request_headers.push((name.clone(), value));
            }
        }
    }
    let mut cookie_values: Vec<String> = Vec::new();
    if let Some(c) = headers.and_then(|h| h.get("Cookie")).filter(|c| !c.trim().is_empty()) {
        cookie_values.push(c);
    }
    cookie_values.extend(cookies.iter().map(|(k, v)| format!("{k}={v}")));
    if !cookie_values.is_empty() {
        request_headers.push(("Cookie".into(), cookie_values.join("; ")));
    }
    HttpRequest { method, url: url.to_owned(), headers: request_headers, content }
}

fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_graphic() && !b"()<>@,;:\\\"/[]?={}".contains(&b))
}

fn set_content_type(content: &mut Content, value: &str) {
    match content {
        Content::Bytes { content_type, .. } | Content::Stream { content_type, .. } => *content_type = Some(value.to_owned()),
        Content::Multipart { content_type, .. } => *content_type = value.to_owned(),
    }
}

/// `request.Content.Headers.TryAddWithoutValidation(name, values)`.
fn add_content_header(content: &mut Content, name: &str, value: &str) {
    if name.eq_ignore_ascii_case("Content-Length") {
        // HttpClient sends the content's real length.
        return;
    }
    if name.eq_ignore_ascii_case("Content-Type") {
        match content {
            Content::Bytes { content_type, .. } | Content::Stream { content_type, .. } => match content_type {
                Some(ct) => {
                    ct.push_str(", ");
                    ct.push_str(value);
                }
                None => *content_type = Some(value.to_owned()),
            },
            Content::Multipart { .. } => {}
        }
        return;
    }
    content.extra_headers_mut().push((name.to_owned(), value.to_owned()));
}

/// `RunOperationAsync`: runs an upload, a shortening or a share; a stop
/// gives `Ok(None)`, an exception `Err` with its message.
pub fn run_operation<T>(uploader: &mut Uploader, op: impl FnOnce(&mut Uploader) -> UResult<T>) -> Result<Option<T>, String> {
    uploader.cancel.reset();
    uploader.is_uploading = true;
    let result = op(uploader);
    uploader.is_uploading = false;
    match result {
        Ok(v) => Ok(Some(v)),
        Err(UploadError::Cancelled) => Ok(None),
        Err(UploadError::Exception(_)) if uploader.cancel.is_cancelled() => Ok(None),
        Err(UploadError::Exception(m)) => Err(m),
    }
}

/// What a destination's `*CoreAsync` returns: a result, or `None` where
/// ShareX returns `null`.
pub type CoreResult = UResult<Option<UploadResult>>;

/// ShareX's uploaders share settings objects with `UploadersConfig` and
/// change them (a refreshed OAuth token, a login's user key, a fixed host).
/// After an upload, the caller writes those changes back with this.
pub trait WritesBackConfig {
    fn write_back(&self, _config: &mut crate::UploadersConfig) {}
}

/// A file or image uploader (`FileUploader`, `ImageUploader`).
pub trait FileUploader: Send + WritesBackConfig {
    fn uploader(&mut self) -> &mut Uploader;
    /// `UploadCoreAsync(stream, fileName)`.
    fn upload_core(&mut self, data: &SharedStream, file_name: &str) -> CoreResult;
}

/// A text uploader (`TextUploader`).
pub trait TextUploader: Send + WritesBackConfig {
    fn uploader(&mut self) -> &mut Uploader;
    /// `UploadTextCoreAsync(text, fileName)`.
    fn upload_text_core(&mut self, text: &str, file_name: &str) -> CoreResult;
}

/// A URL shortener (`URLShortener`).
pub trait UrlShortener: Send + WritesBackConfig {
    fn uploader(&mut self) -> &mut Uploader;
    /// `ShortenURLCoreAsync(url)`.
    fn shorten_url_core(&mut self, url: &str) -> CoreResult;
}

/// A URL sharing service (`URLSharer`).
pub trait UrlSharer: Send + WritesBackConfig {
    fn uploader(&mut self) -> &mut Uploader;
    /// `ShareURLCoreAsync(url)`.
    fn share_url_core(&mut self, url: &str) -> CoreResult;
}

/// `GenericUploader`: anything that uploads a stream (image, text and file
/// destinations).
pub enum GenericUploader {
    File(Box<dyn FileUploader>),
    Text(Box<dyn TextUploader>),
}

impl GenericUploader {
    pub fn uploader(&mut self) -> &mut Uploader {
        match self {
            GenericUploader::File(u) => u.uploader(),
            GenericUploader::Text(u) => u.uploader(),
        }
    }

    /// `UploadAsync(stream, fileName)`.
    pub fn upload(&mut self, data: &SharedStream, file_name: &str) -> Result<Option<UploadResult>, String> {
        self.upload_inner(data, file_name)
    }

    /// Writes changed settings back (see [`WritesBackConfig`]).
    pub fn write_back(&self, config: &mut crate::UploadersConfig) {
        match self {
            GenericUploader::File(u) => u.write_back(config),
            GenericUploader::Text(u) => u.write_back(config),
        }
    }

    fn upload_inner(&mut self, data: &SharedStream, file_name: &str) -> Result<Option<UploadResult>, String> {
        match self {
            GenericUploader::File(u) => {
                let cancel = u.uploader().cancel.clone();
                cancel.reset();
                u.uploader().is_uploading = true;
                let r = u.upload_core(data, file_name);
                u.uploader().is_uploading = false;
                finish(r, &cancel)
            }
            GenericUploader::Text(u) => {
                // TextUploader.UploadCoreAsync: the stream as UTF-8 text
                // (a byte order mark picks another encoding).
                let mut bytes = Vec::new();
                {
                    let mut s = data.lock().expect("upload stream");
                    let _ = s.rewind();
                    s.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                }
                let text = http::decode_text(&bytes, None);
                upload_text(u.as_mut(), &text, file_name)
            }
        }
    }
}

fn finish(r: CoreResult, cancel: &CancellationToken) -> Result<Option<UploadResult>, String> {
    match r {
        Ok(v) => Ok(v),
        Err(UploadError::Cancelled) => Ok(None),
        Err(UploadError::Exception(_)) if cancel.is_cancelled() => Ok(None),
        Err(UploadError::Exception(m)) => Err(m),
    }
}

/// `TextUploader.UploadTextAsync(text, fileName)`.
pub fn upload_text(u: &mut dyn TextUploader, text: &str, file_name: &str) -> Result<Option<UploadResult>, String> {
    let cancel = u.uploader().cancel.clone();
    cancel.reset();
    u.uploader().is_uploading = true;
    let r = u.upload_text_core(text, file_name);
    u.uploader().is_uploading = false;
    finish(r, &cancel)
}

/// `URLShortener.ShortenURLAsync(url)`.
pub fn shorten_url(u: &mut dyn UrlShortener, url: &str) -> Result<Option<UploadResult>, String> {
    let cancel = u.uploader().cancel.clone();
    cancel.reset();
    u.uploader().is_uploading = true;
    let r = u.shorten_url_core(url);
    u.uploader().is_uploading = false;
    finish(r, &cancel)
}

/// `URLSharer.ShareURLAsync(url)`.
pub fn share_url(u: &mut dyn UrlSharer, url: &str) -> Result<Option<UploadResult>, String> {
    let cancel = u.uploader().cancel.clone();
    cancel.reset();
    u.uploader().is_uploading = true;
    let r = u.share_url_core(url);
    u.uploader().is_uploading = false;
    finish(r, &cancel)
}

mod log {
    /// `DebugHelper.WriteLine` (stderr until ShareXrs has its debug log).
    pub fn write(text: &str) {
        if std::env::var_os("SHAREXRS_UPLOAD_DEBUG").is_some() {
            eprintln!("{text}");
        }
    }
}
