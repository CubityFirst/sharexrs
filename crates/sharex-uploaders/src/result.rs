//! `UploadResult.cs` and `Helpers/UploaderErrorManager.cs`.

use sharex_strings::uploaders_lib as strings;

use crate::http::ResponseInfo;
use crate::url_helpers;

/// `UploaderErrorInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploaderErrorInfo {
    pub title: String,
    pub text: String,
}

/// `UploaderErrorManager`: the errors an upload collected, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UploaderErrorManager {
    pub errors: Vec<UploaderErrorInfo>,
    /// `DefaultTitle` (`None`: "Error"); `WorkerTask` names the service.
    pub default_title: Option<String>,
}

impl UploaderErrorManager {
    fn title(&self) -> String {
        self.default_title.clone().unwrap_or_else(|| strings::Common_Error().to_owned())
    }

    /// `Errors.Count`.
    pub fn count(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// `Add(text)`, titled `DefaultTitle` ("Error").
    pub fn add(&mut self, text: impl Into<String>) {
        let title = self.title();
        self.errors.push(UploaderErrorInfo { title, text: text.into() });
    }

    /// `AddFirst(text)`.
    pub fn add_first(&mut self, text: impl Into<String>) {
        let title = self.title();
        self.errors.insert(0, UploaderErrorInfo { title, text: text.into() });
    }

    /// `Add(UploaderErrorManager)`.
    pub fn extend(&mut self, other: &UploaderErrorManager) {
        self.errors.extend(other.errors.iter().cloned());
    }
}

impl std::fmt::Display for UploaderErrorManager {
    /// `ToString()`: the texts separated by a blank line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let nl = crate::NEW_LINE;
        let texts: Vec<&str> = self.errors.iter().map(|e| e.text.as_str()).collect();
        f.write_str(&texts.join(&format!("{nl}{nl}")))
    }
}

/// `UploadResult`: what an upload, a shortening or a share produced.
#[derive(Debug, Clone, PartialEq)]
pub struct UploadResult {
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub deletion_url: Option<String>,
    pub shortened_url: Option<String>,
    /// The `IsSuccess` setter's value; read through [`UploadResult::is_success`].
    pub success: bool,
    pub response: Option<String>,
    pub errors: UploaderErrorManager,
    pub is_url_expected: bool,
    pub response_info: Option<ResponseInfo>,
}

impl Default for UploadResult {
    fn default() -> Self {
        Self {
            url: None,
            thumbnail_url: None,
            deletion_url: None,
            shortened_url: None,
            success: false,
            response: None,
            errors: UploaderErrorManager::default(),
            is_url_expected: true,
            response_info: None,
        }
    }
}

fn non_empty(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

impl UploadResult {
    /// `new UploadResult(source, url)`.
    pub fn with_response(response: Option<String>, url: Option<String>) -> Self {
        Self { response, url, ..Default::default() }
    }

    /// `IsSuccess`: set, and a non-empty response.
    pub fn is_success(&self) -> bool {
        self.success && non_empty(&self.response)
    }

    /// `IsError`.
    pub fn is_error(&self) -> bool {
        !self.errors.is_empty() && (!self.is_url_expected || !non_empty(&self.url))
    }

    /// `ForceHTTPS()`.
    pub fn force_https(&mut self) {
        for u in [&mut self.url, &mut self.thumbnail_url, &mut self.deletion_url, &mut self.shortened_url] {
            if let Some(s) = u.as_mut() {
                *s = url_helpers::force_prefix(s, "https://");
            }
        }
    }

    /// `ErrorsToString()`.
    pub fn errors_to_string(&self) -> Option<String> {
        self.is_error().then(|| self.errors.to_string())
    }

    /// `ToSummaryString()`.
    pub fn to_summary_string(&self) -> String {
        let nl = crate::NEW_LINE;
        let v = |s: &Option<String>| s.clone().unwrap_or_default();
        format!(
            "{}: {}{nl}{}: {}{nl}{}: {}{nl}{}: {}{nl}",
            strings::ResponseWindow_URL(),
            v(&self.url),
            strings::ResponseWindow_Thumbnail_URL(),
            v(&self.thumbnail_url),
            strings::ResponseWindow_Shortened_URL(),
            v(&self.shortened_url),
            strings::ResponseWindow_Deletion_URL(),
            v(&self.deletion_url),
        )
    }
}

impl std::fmt::Display for UploadResult {
    /// `ToString()`: the shortened URL, else the URL.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if non_empty(&self.shortened_url) {
            f.write_str(self.shortened_url.as_deref().unwrap_or(""))
        } else {
            f.write_str(self.url.as_deref().unwrap_or(""))
        }
    }
}
