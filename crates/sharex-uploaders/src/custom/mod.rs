//! Custom uploaders (`.sxcu`): [`CustomUploaderItem`] and the five
//! uploaders that run one (`CustomImageUploader`, `CustomTextUploader`,
//! `CustomFileUploader`, `CustomURLShortener`, `CustomURLSharer`).

pub mod item;
pub mod parser;

pub use item::{CustomUploaderInput, CustomUploaderItem, SerializeMode};
pub use parser::{CustomUploaderParser, ParserUi, set_parser_ui};

use sharex_helpers::mime_types;
use sharex_strings::uploaders_lib as strings;

use crate::enums::CustomUploaderBody;
use crate::http::{self, SharedStream};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, FileUploader, TextUploader, UResult, UploadError, Uploader, UrlSharer, UrlShortener, WritesBackConfig};

fn unsupported(body: &CustomUploaderBody) -> UploadError {
    UploadError::Exception(strings::CustomUploader_Unsupported_request_format().replace("{0}", body.name()))
}

/// `CustomImageUploader` and `CustomFileUploader` (the same code).
pub struct CustomFileUploader {
    pub base: Uploader,
    pub item: CustomUploaderItem,
}

impl WritesBackConfig for CustomFileUploader {}

impl CustomFileUploader {
    pub fn new(item: CustomUploaderItem) -> Self {
        Self { base: Uploader::new(), item }
    }
}

impl FileUploader for CustomFileUploader {
    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }

    fn upload_core(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let input = CustomUploaderInput::new(file_name, "");
        let item = &self.item;
        let mut result = UploadResult::default();
        match item.body {
            CustomUploaderBody::MultipartFormData => {
                let url = item.get_request_url(&input)?;
                let form_name = item.get_file_form_name()?;
                let args = item.get_arguments(&input)?;
                let headers = item.get_headers(&input)?;
                result = self.base.send_request_file(
                    &url,
                    data,
                    file_name,
                    &form_name,
                    &args,
                    headers.as_ref(),
                    item.request_method.clone(),
                    http::CONTENT_TYPE_MULTIPART_FORM_DATA,
                    None,
                )?;
            }
            CustomUploaderBody::Binary => {
                let url = item.get_request_url(&input)?;
                let mime = mime_types::get_mime_type_from_file_name(file_name);
                let headers = item.get_headers(&input)?;
                result.response = self.base.send_request_stream(item.request_method.clone(), &url, data, Some(&mime), &[], headers.as_ref())?;
            }
            ref other => return Err(unsupported(other)),
        }
        let last = self.base.last_response_info.clone();
        item.try_parse_response(&mut result, last.as_ref(), &mut self.base.errors, &input, false);
        Ok(Some(result))
    }
}

/// `CustomTextUploader`.
pub struct CustomTextUploader {
    pub base: Uploader,
    pub item: CustomUploaderItem,
}

impl WritesBackConfig for CustomTextUploader {}

impl CustomTextUploader {
    pub fn new(item: CustomUploaderItem) -> Self {
        Self { base: Uploader::new(), item }
    }
}

impl TextUploader for CustomTextUploader {
    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }

    fn upload_text_core(&mut self, text: &str, file_name: &str) -> CoreResult {
        let input = CustomUploaderInput::new(file_name, text);
        let item = &self.item;
        let method = item.request_method.clone();
        let mut result = UploadResult::default();
        match item.body {
            CustomUploaderBody::None => {
                let url = item.get_request_url(&input)?;
                let headers = item.get_headers(&input)?;
                result.response = self.base.send_request(method, &url, &[], headers.as_ref())?;
            }
            CustomUploaderBody::MultipartFormData => {
                if item.file_form_name.as_deref().is_none_or(str::is_empty) {
                    let url = item.get_request_url(&input)?;
                    let args = item.get_arguments(&input)?;
                    let headers = item.get_headers(&input)?;
                    result.response = self.base.send_request_multipart(&url, &args, headers.as_ref(), method)?;
                } else {
                    let stream = http::shared_stream(std::io::Cursor::new(text.as_bytes().to_vec()));
                    let url = item.get_request_url(&input)?;
                    let form_name = item.get_file_form_name()?;
                    let args = item.get_arguments(&input)?;
                    let headers = item.get_headers(&input)?;
                    result = self.base.send_request_file(
                        &url,
                        &stream,
                        file_name,
                        &form_name,
                        &args,
                        headers.as_ref(),
                        method,
                        http::CONTENT_TYPE_MULTIPART_FORM_DATA,
                        None,
                    )?;
                }
            }
            CustomUploaderBody::FormURLEncoded => {
                let url = item.get_request_url(&input)?;
                let args = item.get_arguments(&input)?;
                let headers = item.get_headers(&input)?;
                result.response = self.base.send_request_url_encoded(method, &url, &args, headers.as_ref())?;
            }
            CustomUploaderBody::JSON | CustomUploaderBody::XML => {
                let url = item.get_request_url(&input)?;
                let data = item.get_data(&input);
                let headers = item.get_headers(&input)?;
                result.response = self.base.send_request_text(method, &url, &data, item.get_content_type(), &[], headers.as_ref())?;
            }
            CustomUploaderBody::Binary => {
                let stream = http::shared_stream(std::io::Cursor::new(text.as_bytes().to_vec()));
                let url = item.get_request_url(&input)?;
                let mime = mime_types::get_mime_type_from_file_name(file_name);
                let headers = item.get_headers(&input)?;
                result.response = self.base.send_request_stream(method, &url, &stream, Some(&mime), &[], headers.as_ref())?;
            }
            ref other => return Err(unsupported(other)),
        }
        let last = self.base.last_response_info.clone();
        item.try_parse_response(&mut result, last.as_ref(), &mut self.base.errors, &input, false);
        Ok(Some(result))
    }
}

/// The body-less, form, URL-encoded and JSON/XML requests the URL
/// shortener and sharer send (they share this code in ShareX).
fn send_url_request(base: &mut Uploader, item: &CustomUploaderItem, input: &CustomUploaderInput) -> UResult<Option<String>> {
    let method = item.request_method.clone();
    match item.body {
        CustomUploaderBody::None => {
            let url = item.get_request_url(input)?;
            let headers = item.get_headers(input)?;
            base.send_request(method, &url, &[], headers.as_ref())
        }
        CustomUploaderBody::MultipartFormData => {
            let url = item.get_request_url(input)?;
            let args = item.get_arguments(input)?;
            let headers = item.get_headers(input)?;
            base.send_request_multipart(&url, &args, headers.as_ref(), method)
        }
        CustomUploaderBody::FormURLEncoded => {
            let url = item.get_request_url(input)?;
            let args = item.get_arguments(input)?;
            let headers = item.get_headers(input)?;
            base.send_request_url_encoded(method, &url, &args, headers.as_ref())
        }
        CustomUploaderBody::JSON | CustomUploaderBody::XML => {
            let url = item.get_request_url(input)?;
            let data = item.get_data(input);
            let headers = item.get_headers(input)?;
            base.send_request_text(method, &url, &data, item.get_content_type(), &[], headers.as_ref())
        }
        ref other => Err(unsupported(other)),
    }
}

/// `CustomURLShortener`.
pub struct CustomUrlShortener {
    pub base: Uploader,
    pub item: CustomUploaderItem,
}

impl WritesBackConfig for CustomUrlShortener {}

impl CustomUrlShortener {
    pub fn new(item: CustomUploaderItem) -> Self {
        Self { base: Uploader::new(), item }
    }
}

impl UrlShortener for CustomUrlShortener {
    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }

    fn shorten_url_core(&mut self, url: &str) -> CoreResult {
        let mut result = UploadResult { url: Some(url.to_owned()), ..Default::default() };
        let input = CustomUploaderInput::new("", url);
        result.response = send_url_request(&mut self.base, &self.item, &input)?;
        let last = self.base.last_response_info.clone();
        self.item.try_parse_response(&mut result, last.as_ref(), &mut self.base.errors, &input, true);
        Ok(Some(result))
    }
}

/// `CustomURLSharer`.
pub struct CustomUrlSharer {
    pub base: Uploader,
    pub item: CustomUploaderItem,
}

impl WritesBackConfig for CustomUrlSharer {}

impl CustomUrlSharer {
    pub fn new(item: CustomUploaderItem) -> Self {
        Self { base: Uploader::new(), item }
    }
}

impl UrlSharer for CustomUrlSharer {
    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }

    fn share_url_core(&mut self, url: &str) -> CoreResult {
        let mut result = UploadResult { url: Some(url.to_owned()), is_url_expected: false, ..Default::default() };
        let input = CustomUploaderInput::new("", url);
        result.response = send_url_request(&mut self.base, &self.item, &input)?;
        let last = self.base.last_response_info.clone();
        self.item.try_parse_response(&mut result, last.as_ref(), &mut self.base.errors, &input, false);
        Ok(Some(result))
    }
}
