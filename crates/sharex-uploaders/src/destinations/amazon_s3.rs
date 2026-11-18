//! `FileUploaders/AmazonS3.cs`: Amazon S3 and S3-compatible storage
//! (DigitalOcean Spaces, Wasabi, MinIO, R2...), signed with AWS Signature
//! Version 4, as one PUT or a multipart upload (parts in parallel).

use std::sync::{Arc, Mutex};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use sharex_helpers::{NameParser, NameParserType, file_helpers, mime_types};
use sharex_strings::uploaders_lib as strings;

use super::uploader_base;
use crate::config::AmazonS3Settings;
use crate::custom::CustomUploaderParser;
use crate::enums::HttpMethod;
use crate::http::{self, Content, ProgressManager, ResponseInfo, SharedStream, StreamPart};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

const DEFAULT_REGION: &str = "us-east-1";
const SIGNATURE_ALGORITHM: &str = "AWS4-HMAC-SHA256";
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
const DEFAULT_PART_SIZE: u64 = 100 * 1024 * 1024;
const MINIMUM_PART_SIZE: u64 = 5 * 1024 * 1024;
const MAXIMUM_PART_SIZE: u64 = 5 * 1024 * 1024 * 1024;
const MAXIMUM_PART_COUNT: u64 = 10000;
/// `AmazonS3Settings.MinMultipartConcurrency` / `MaxMultipartConcurrency`.
const MIN_MULTIPART_CONCURRENCY: i32 = 1;
const MAX_MULTIPART_CONCURRENCY: i32 = 16;

/// `AmazonS3Endpoint`: the endpoints the settings offer.
pub struct AmazonS3Endpoint {
    pub name: &'static str,
    pub endpoint: &'static str,
    pub region: Option<&'static str>,
}

/// `AmazonS3.Endpoints`.
pub static ENDPOINTS: &[AmazonS3Endpoint] = &[
    AmazonS3Endpoint { name: "Asia Pacific (Hong Kong)", endpoint: "s3.ap-east-1.amazonaws.com", region: Some("ap-east-1") },
    AmazonS3Endpoint { name: "Asia Pacific (Mumbai)", endpoint: "s3.ap-south-1.amazonaws.com", region: Some("ap-south-1") },
    AmazonS3Endpoint { name: "Asia Pacific (Seoul)", endpoint: "s3.ap-northeast-2.amazonaws.com", region: Some("ap-northeast-2") },
    AmazonS3Endpoint { name: "Asia Pacific (Singapore)", endpoint: "s3.ap-southeast-1.amazonaws.com", region: Some("ap-southeast-1") },
    AmazonS3Endpoint { name: "Asia Pacific (Sydney)", endpoint: "s3.ap-southeast-2.amazonaws.com", region: Some("ap-southeast-2") },
    AmazonS3Endpoint { name: "Asia Pacific (Tokyo)", endpoint: "s3.ap-northeast-1.amazonaws.com", region: Some("ap-northeast-1") },
    AmazonS3Endpoint { name: "Canada (Central)", endpoint: "s3.ca-central-1.amazonaws.com", region: Some("ca-central-1") },
    AmazonS3Endpoint { name: "China (Beijing)", endpoint: "s3.cn-north-1.amazonaws.com.cn", region: Some("cn-north-1") },
    AmazonS3Endpoint { name: "China (Ningxia)", endpoint: "s3.cn-northwest-1.amazonaws.com.cn", region: Some("cn-northwest-1") },
    AmazonS3Endpoint { name: "EU (Frankfurt)", endpoint: "s3.eu-central-1.amazonaws.com", region: Some("eu-central-1") },
    AmazonS3Endpoint { name: "EU (Ireland)", endpoint: "s3.eu-west-1.amazonaws.com", region: Some("eu-west-1") },
    AmazonS3Endpoint { name: "EU (London)", endpoint: "s3.eu-west-2.amazonaws.com", region: Some("eu-west-2") },
    AmazonS3Endpoint { name: "EU (Paris)", endpoint: "s3.eu-west-3.amazonaws.com", region: Some("eu-west-3") },
    AmazonS3Endpoint { name: "EU (Stockholm)", endpoint: "s3.eu-north-1.amazonaws.com", region: Some("eu-north-1") },
    AmazonS3Endpoint { name: "Middle East (Bahrain)", endpoint: "s3.me-south-1.amazonaws.com", region: Some("me-south-1") },
    AmazonS3Endpoint { name: "South America (São Paulo)", endpoint: "s3.sa-east-1.amazonaws.com", region: Some("sa-east-1") },
    AmazonS3Endpoint { name: "US East (N. Virginia)", endpoint: "s3.amazonaws.com", region: Some("us-east-1") },
    AmazonS3Endpoint { name: "US East (Ohio)", endpoint: "s3.us-east-2.amazonaws.com", region: Some("us-east-2") },
    AmazonS3Endpoint { name: "US West (N. California)", endpoint: "s3.us-west-1.amazonaws.com", region: Some("us-west-1") },
    AmazonS3Endpoint { name: "US West (Oregon)", endpoint: "s3.us-west-2.amazonaws.com", region: Some("us-west-2") },
    AmazonS3Endpoint { name: "DreamObjects", endpoint: "objects-us-east-1.dream.io", region: None },
    AmazonS3Endpoint { name: "DigitalOcean (Amsterdam)", endpoint: "ams3.digitaloceanspaces.com", region: Some("ams3") },
    AmazonS3Endpoint { name: "DigitalOcean (New York)", endpoint: "nyc3.digitaloceanspaces.com", region: Some("nyc3") },
    AmazonS3Endpoint { name: "DigitalOcean (San Francisco)", endpoint: "sfo2.digitaloceanspaces.com", region: Some("sfo2") },
    AmazonS3Endpoint { name: "DigitalOcean (Singapore)", endpoint: "sgp1.digitaloceanspaces.com", region: Some("sgp1") },
    AmazonS3Endpoint { name: "Wasabi", endpoint: "s3.wasabisys.com", region: None },
];

/// `AmazonS3RequestData`.
#[derive(Debug, Clone)]
struct RequestData {
    scheme: String,
    host: String,
    canonical_uri: String,
}

/// `AmazonS3`.
pub struct AmazonS3 {
    pub base: Uploader,
    pub settings: AmazonS3Settings,
    /// The signing clock (UTC now); tests fix it.
    pub now: fn() -> chrono::DateTime<chrono::Utc>,
}

uploader_base!(AmazonS3, FileUploader);
impl WritesBackConfig for AmazonS3 {}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn exception<T>(e: impl ToString) -> UResult<T> {
    Err(UploadError::Exception(e.to_string()))
}

impl AmazonS3 {
    pub fn new(settings: AmazonS3Settings) -> Self {
        Self { base: Uploader::new(), settings, now: chrono::Utc::now }
    }

    /// `AmazonS3NewFileUploaderService.CheckConfig`.
    pub fn check_config(s: &AmazonS3Settings) -> bool {
        let ne = |o: &Option<String>| o.as_deref().is_some_and(|v| !v.is_empty());
        ne(&s.AccessKeyID) && !s.SecretAccessKey.is_empty() && ne(&s.Endpoint) && ne(&s.Bucket)
    }

    fn endpoint(&self) -> &str {
        self.settings.Endpoint.as_deref().unwrap_or("")
    }

    fn bucket(&self) -> &str {
        self.settings.Bucket.as_deref().unwrap_or("")
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let content_type = mime_types::get_mime_type_from_file_name(file_name);
        let upload_path = self.get_upload_path(file_name)?;
        let result_url = self.generate_url(&upload_path);
        self.base.early_url_copy_requested(&result_url);
        let request_data = self.create_request_data(&upload_path)?;
        let length = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let result = if self.settings.UseMultipartUpload && length > 0 {
            self.upload_multipart(data, length, &content_type, &result_url, &request_data)?
        } else {
            self.upload_single_request(data, length, &content_type, &result_url, &request_data)?
        };
        if result.is_none() {
            self.base.errors.add(strings::AmazonS3_Upload_failed());
        }
        Ok(result)
    }

    fn upload_single_request(
        &mut self,
        data: &SharedStream,
        length: u64,
        content_type: &str,
        result_url: &str,
        request_data: &RequestData,
    ) -> UResult<Option<UploadResult>> {
        let headers = self.create_object_headers();
        self.send_s3_request(HttpMethod::PUT, request_data, "", Some((data, 0, length)), Some(content_type), Some(headers))?;
        Ok(self.base.last_response_info.as_ref().is_some_and(ResponseInfo::is_success).then(|| create_upload_result(result_url)))
    }

    fn upload_multipart(
        &mut self,
        data: &SharedStream,
        length: u64,
        content_type: &str,
        result_url: &str,
        request_data: &RequestData,
    ) -> UResult<Option<UploadResult>> {
        let part_size = multipart_part_size(length);
        if part_size == 0 {
            return Ok(None);
        }
        let mut upload_id: Option<String> = None;
        let outcome = (|| -> UResult<Option<UploadResult>> {
            let headers = self.create_object_headers();
            let response = self.send_s3_request(HttpMethod::POST, request_data, "uploads=", None, Some(content_type), Some(headers))?;
            upload_id = get_upload_id(response.as_deref());
            let Some(id) = upload_id.clone().filter(|i| !i.is_empty()) else { return Ok(None) };
            let Some(parts) = self.upload_parts(data, length, part_size, content_type, request_data, &id)? else { return Ok(None) };
            let payload = complete_multipart_upload_payload(&parts);
            let complete = http::shared_stream(std::io::Cursor::new(payload.clone().into_bytes()));
            let query = format!("uploadId={}", url_helpers::url_encode(&id, false));
            let response = self.send_s3_request(
                HttpMethod::POST,
                request_data,
                &query,
                Some((&complete, 0, payload.len() as u64)),
                Some("application/xml"),
                None,
            )?;
            if !self.base.last_response_info.as_ref().is_some_and(ResponseInfo::is_success) || is_s3_error_response(response.as_deref()) {
                return Ok(None);
            }
            Ok(Some(create_upload_result(result_url)))
        })();
        let completed = matches!(outcome, Ok(Some(_)));
        if !completed && let Some(id) = upload_id.filter(|i| !i.is_empty()) {
            self.abort_multipart_upload(request_data, &id);
        }
        outcome
    }

    /// `UploadPartsAsync`: the parts in parallel (up to
    /// `MultipartConcurrency`), progress over the whole file.
    fn upload_parts(
        &mut self,
        data: &SharedStream,
        length: u64,
        part_size: u64,
        content_type: &str,
        request_data: &RequestData,
        upload_id: &str,
    ) -> UResult<Option<Vec<(u32, String)>>> {
        let part_count = length.div_ceil(part_size) as usize;
        let concurrency = (self.settings.MultipartConcurrency.clamp(MIN_MULTIPART_CONCURRENCY, MAX_MULTIPART_CONCURRENCY) as usize).min(part_count);
        let report = self.base.allow_report_progress;
        let progress_cb = self.base.progress_callback();
        let progress = Arc::new(Mutex::new(ProgressManager::new(length, 0)));
        let on_read: Arc<dyn Fn(u64) + Send + Sync> = Arc::new(move |n| {
            if !report {
                return;
            }
            let mut pm = progress.lock().expect("progress");
            if pm.update_progress(n)
                && let Some(cb) = &progress_cb
            {
                cb(&pm);
            }
        });
        let next = Mutex::new(0usize);
        let failed = std::sync::atomic::AtomicBool::new(false);
        let parts: Mutex<Vec<Option<(u32, String)>>> = Mutex::new(vec![None; part_count]);
        let forks: Mutex<Vec<Uploader>> = Mutex::new(Vec::new());
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            for _ in 0..concurrency {
                let mut fork = self.base.fork();
                fork.allow_report_progress = false;
                let this = &*self;
                let (next, failed, parts, forks, cancelled, on_read) = (&next, &failed, &parts, &forks, &cancelled, on_read.clone());
                scope.spawn(move || {
                    loop {
                        if failed.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                        let index = {
                            let mut n = next.lock().expect("next part");
                            let i = *n;
                            *n += 1;
                            i
                        };
                        if index >= part_count {
                            break;
                        }
                        let position = index as u64 * part_size;
                        let len = part_size.min(length - position);
                        match this.upload_part(&mut fork, data, index, position, len, content_type, request_data, upload_id, on_read.clone()) {
                            Ok(Some(part)) => parts.lock().expect("parts")[index] = Some(part),
                            Ok(None) => failed.store(true, std::sync::atomic::Ordering::SeqCst),
                            Err(UploadError::Cancelled) => {
                                cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
                                failed.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                            Err(_) => failed.store(true, std::sync::atomic::Ordering::SeqCst),
                        }
                    }
                    forks.lock().expect("forks").push(fork);
                });
            }
        });
        for fork in forks.into_inner().expect("forks") {
            self.base.errors.extend(&fork.errors);
            if fork.last_response_info.is_some() {
                self.base.last_response_info = fork.last_response_info;
            }
        }
        if cancelled.into_inner() && self.base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let parts = parts.into_inner().expect("parts");
        Ok(if failed.into_inner() || parts.iter().any(Option::is_none) { None } else { Some(parts.into_iter().flatten().collect()) })
    }

    #[allow(clippy::too_many_arguments)]
    fn upload_part(
        &self,
        fork: &mut Uploader,
        data: &SharedStream,
        index: usize,
        position: u64,
        length: u64,
        content_type: &str,
        request_data: &RequestData,
        upload_id: &str,
        on_read: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> UResult<Option<(u32, String)>> {
        let part_number = index as u32 + 1;
        let query = format!("partNumber={part_number}&uploadId={}", url_helpers::url_encode(upload_id, false));
        let hashed_payload = self.payload_hash(data, position, length)?;
        let headers = self.create_signed_request_headers(&HttpMethod::PUT, request_data, &query, &hashed_payload, Some(content_type), None);
        let url = get_request_url(request_data, &query);
        let total = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let content = Content::Stream {
            part: StreamPart { source: data.clone(), position, length, total_length: total, buffer_size: fork.buffer_size, on_read: Some(on_read) },
            content_type: Some(content_type.to_owned()),
            headers: Vec::new(),
        };
        let Some(response) = fork.get_response(HttpMethod::PUT, &url, Some(content), &[], Some(&headers), &[], false)? else { return Ok(None) };
        if !response.is_success() {
            return Ok(None);
        }
        match response.header("ETag").filter(|e| !e.trim().is_empty()) {
            Some(etag) => Ok(Some((part_number, etag.trim().to_owned()))),
            None => Ok(None),
        }
    }

    /// `AbortMultipartUploadAsync` (keeps `LastResponseInfo`).
    fn abort_multipart_upload(&mut self, request_data: &RequestData, upload_id: &str) {
        let last = self.base.last_response_info.clone();
        let saved_timeout = self.base.request_timeout;
        self.base.request_timeout = Some(std::time::Duration::from_secs(30));
        let query = format!("uploadId={}", url_helpers::url_encode(upload_id, false));
        let _ = self.send_s3_request(HttpMethod::DELETE, request_data, &query, None, None, None);
        self.base.request_timeout = saved_timeout;
        self.base.last_response_info = last;
    }

    /// `DeleteObjectAsync`.
    pub fn delete_object(&mut self, object_key: &str) -> UResult<bool> {
        if object_key.is_empty() {
            return exception("The value cannot be an empty string. (Parameter 'objectKey')");
        }
        let request_data = self.create_request_data(object_key)?;
        self.send_s3_request(HttpMethod::DELETE, &request_data, "", None, None, None)?;
        Ok(self.base.last_response_info.as_ref().is_some_and(ResponseInfo::is_success))
    }

    /// `ListObjectsAsync(prefix, delimiter, maxKeys)`: every page
    /// (`list-type=2`, keys URL-encoded), `None` when a request or its XML
    /// fails.
    pub fn list_objects(&mut self, prefix: &str, delimiter: Option<&str>, max_keys: Option<i32>) -> UResult<Option<Vec<AmazonS3ObjectInfo>>> {
        let prefix = normalize_object_key(prefix);
        let mut objects = Vec::new();
        let mut continuation: Option<String> = None;
        loop {
            let mut query: Vec<(&str, String)> = vec![("encoding-type", "url".into()), ("list-type", "2".into())];
            if !prefix.is_empty() {
                query.push(("prefix", prefix.clone()));
            }
            if let Some(d) = delimiter.filter(|d| !d.is_empty()) {
                query.push(("delimiter", d.to_owned()));
            }
            if let Some(m) = max_keys.filter(|m| *m > 0) {
                query.push(("max-keys", m.to_string()));
            }
            if let Some(t) = continuation.as_deref().filter(|t| !t.is_empty()) {
                query.push(("continuation-token", t.to_owned()));
            }
            query.sort_by(|a, b| a.0.cmp(b.0));
            let canonical_query = query.iter().map(|(k, v)| format!("{}={}", url_helpers::url_encode(k, false), url_helpers::url_encode(v, false))).collect::<Vec<_>>().join("&");
            let request_data = self.create_request_data("")?;
            let headers = self.create_signed_request_headers(&HttpMethod::GET, &request_data, &canonical_query, &self.empty_payload_hash(), None, None);
            let response = self.base.send_request(HttpMethod::GET, &get_request_url(&request_data, &canonical_query), &[], Some(&headers))?;
            let ok = self.base.last_response_info.as_ref().is_some_and(ResponseInfo::is_success);
            let Some(response) = response.filter(|r| ok && !r.trim().is_empty()) else { return Ok(None) };
            let Some(page) = parse_list_objects_response(&response) else { return Ok(None) };
            objects.extend(page.objects);
            if let Some(m) = max_keys.filter(|m| *m > 0)
                && objects.len() >= m as usize
            {
                break;
            }
            continuation = if page.is_truncated { page.next_continuation_token } else { None };
            if page.is_truncated && continuation.as_deref().is_none_or(str::is_empty) {
                return Ok(None);
            }
            if continuation.as_deref().is_none_or(str::is_empty) {
                break;
            }
        }
        Ok(Some(objects))
    }

    /// `DownloadObjectAsync`: the object's bytes into `destination`.
    pub fn download_object(&mut self, object_key: &str, destination: &mut dyn std::io::Write) -> UResult<bool> {
        if object_key.is_empty() {
            return exception("The value cannot be an empty string. (Parameter 'objectKey')");
        }
        let request_data = self.create_request_data(&normalize_object_key(object_key))?;
        let headers = self.create_signed_request_headers(&HttpMethod::GET, &request_data, "", &self.empty_payload_hash(), None, None);
        let url = get_request_url(&request_data, "");
        let Some(response) = self.base.get_response(HttpMethod::GET, &url, None, &[], Some(&headers), &[], false)? else { return Ok(false) };
        if let Err(e) = destination.write_all(response.response_data.as_deref().unwrap_or(&[])) {
            self.base.process_error(&e.to_string(), &url, None);
            return Ok(false);
        }
        Ok(true)
    }

    /// `UploadObjectAsync`: one object (multipart when the settings say
    /// so), its key as given.
    pub fn upload_object(&mut self, data: &SharedStream, object_key: &str) -> UResult<bool> {
        if object_key.is_empty() {
            return exception("The value cannot be an empty string. (Parameter 'objectKey')");
        }
        let object_key = normalize_object_key(object_key);
        let content_type = mime_types::get_mime_type_from_file_name(&object_key);
        let request_data = self.create_request_data(&object_key)?;
        let url = self.generate_url(&object_key);
        let length = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let result = if self.settings.UseMultipartUpload && length > 0 {
            self.upload_multipart(data, length, &content_type, &url, &request_data)?
        } else {
            self.upload_single_request(data, length, &content_type, &url, &request_data)?
        };
        Ok(result.is_some())
    }

    /// `CopyObjectAsync`: a `PUT` with `x-amz-copy-source`.
    pub fn copy_object(&mut self, source_key: &str, destination_key: &str) -> UResult<bool> {
        if source_key.is_empty() {
            return exception("The value cannot be an empty string. (Parameter 'sourceObjectKey')");
        }
        if destination_key.is_empty() {
            return exception("The value cannot be an empty string. (Parameter 'destinationObjectKey')");
        }
        let source = normalize_object_key(source_key);
        let request_data = self.create_request_data(&normalize_object_key(destination_key))?;
        let mut headers = self.create_object_headers();
        headers.set("x-amz-copy-source", format!("/{}", url_helpers::url_encode(&url_helpers::combine_url(self.bucket(), &source), true)));
        let response = self.send_s3_request(HttpMethod::PUT, &request_data, "", None, None, Some(headers))?;
        Ok(self.base.last_response_info.as_ref().is_some_and(ResponseInfo::is_success) && !is_s3_error_response(response.as_deref()))
    }

    /// `SendAmazonS3RequestAsync`: an empty body is `None`.
    fn send_s3_request(
        &mut self,
        method: HttpMethod,
        request_data: &RequestData,
        canonical_query_string: &str,
        body: Option<(&SharedStream, u64, u64)>,
        content_type: Option<&str>,
        request_headers: Option<Headers>,
    ) -> UResult<Option<String>> {
        let hashed_payload = match body {
            Some((stream, position, length)) => self.payload_hash(stream, position, length)?,
            None => self.empty_payload_hash(),
        };
        let headers = self.create_signed_request_headers(&method, request_data, canonical_query_string, &hashed_payload, content_type, request_headers);
        let url = get_request_url(request_data, canonical_query_string);
        // SendRequestAsync(method, url, stream, position, length, contentType): always a body, maybe empty.
        let content = match body {
            Some((stream, position, length)) => self.base.stream_content(stream, position, length, content_type)?,
            None => Content::bytes(Vec::new(), content_type),
        };
        Ok(self.base.get_response(method, &url, Some(content), &[], Some(&headers), &[], false)?.and_then(|r| r.response_text))
    }

    /// `CreateSignedRequestHeaders`: SigV4.
    fn create_signed_request_headers(
        &self,
        method: &HttpMethod,
        request_data: &RequestData,
        canonical_query_string: &str,
        hashed_payload: &str,
        content_type: Option<&str>,
        request_headers: Option<Headers>,
    ) -> Headers {
        let now = (self.now)();
        let credential_date = now.format("%Y%m%d").to_string();
        let time_stamp = now.format("%Y%m%dT%H%M%SZ").to_string();
        let region = self.region();
        let scope = url_helpers::combine_urls(&[&credential_date, &region, "s3", "aws4_request"]);
        let credential = url_helpers::combine_url(self.settings.AccessKeyID.as_deref().unwrap_or(""), &scope);
        let mut headers = request_headers.unwrap_or_default();
        headers.set("Host", request_data.host.clone());
        if let Some(ct) = content_type.filter(|c| !c.is_empty()) {
            headers.set("Content-Type", ct);
        }
        headers.set("x-amz-date", time_stamp.clone());
        headers.set("x-amz-content-sha256", hashed_payload);
        let mut keys: Vec<&(String, Vec<String>)> = headers.0.iter().collect();
        keys.sort_by(|a, b| a.0.cmp(&b.0));
        let canonical_headers: String = keys.iter().map(|(k, v)| format!("{}:{}\n", k.to_lowercase(), v.join(",").trim())).collect();
        let signed_headers = keys.iter().map(|(k, _)| k.to_lowercase()).collect::<Vec<_>>().join(";");
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.name(),
            request_data.canonical_uri,
            canonical_query_string,
            canonical_headers,
            signed_headers,
            hashed_payload
        );
        let string_to_sign = format!("{SIGNATURE_ALGORITHM}\n{time_stamp}\n{scope}\n{}", sha256_hex(canonical_request.as_bytes()));
        let date_key = hmac_sha256(format!("AWS4{}", self.settings.SecretAccessKey.as_str()).as_bytes(), &credential_date);
        let date_region_key = hmac_sha256(&date_key, &region);
        let date_region_service_key = hmac_sha256(&date_region_key, "s3");
        let signing_key = hmac_sha256(&date_region_service_key, "aws4_request");
        let signature = hex::encode(hmac_sha256(&signing_key, &string_to_sign));
        headers.set("Authorization", format!("{SIGNATURE_ALGORITHM} Credential={credential},SignedHeaders={signed_headers},Signature={signature}"));
        headers.0.retain(|(k, _)| !k.eq_ignore_ascii_case("Host") && !k.eq_ignore_ascii_case("Content-Type"));
        headers
    }

    /// `CreateRequestData`.
    fn create_request_data(&self, upload_path: &str) -> UResult<RequestData> {
        let bucket = self.settings.Bucket.as_deref().ok_or(UploadError::Exception(super::NULL_REFERENCE.into()))?;
        let is_path_style = self.settings.UsePathStyle || bucket.contains('.');
        let endpoint_setting = self.settings.Endpoint.as_deref().ok_or(UploadError::Exception(super::NULL_REFERENCE.into()))?;
        let scheme = url_helpers::get_prefix(endpoint_setting).unwrap_or("").to_owned();
        let endpoint = url_helpers::remove_prefixes(endpoint_setting).trim_end_matches('/').to_owned();
        let host = if is_path_style { endpoint.clone() } else { format!("{bucket}.{endpoint}") };
        let canonical_uri = if is_path_style { url_helpers::combine_url(bucket, upload_path) } else { upload_path.to_owned() };
        let canonical_uri = url_helpers::url_encode(&url_helpers::add_slash_prefix(&canonical_uri), true);
        Ok(RequestData { scheme, host, canonical_uri })
    }

    /// `CreateObjectHeaders`.
    fn create_object_headers(&self) -> Headers {
        let mut headers = Headers::new().with("x-amz-storage-class", self.settings.StorageClass.name().to_owned());
        if self.settings.SetPublicACL {
            headers.set("x-amz-acl", "public-read");
        }
        headers
    }

    /// `GetPayloadHash`.
    fn payload_hash(&self, stream: &SharedStream, position: u64, length: u64) -> UResult<String> {
        if !self.settings.SignedPayload {
            return Ok(UNSIGNED_PAYLOAD.to_owned());
        }
        use std::io::{Read, Seek, SeekFrom};
        let mut s = stream.lock().expect("upload stream");
        let original = s.stream_position().map_err(|e| UploadError::Exception(e.to_string()))?;
        let mut hasher = Sha256::new();
        let result = (|| -> std::io::Result<()> {
            s.seek(SeekFrom::Start(position))?;
            let mut remaining = length;
            let mut buf = vec![0u8; self.base.buffer_size.max(1)];
            while remaining > 0 {
                if self.base.stop_requested() {
                    return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "canceled"));
                }
                let want = (buf.len() as u64).min(remaining) as usize;
                let n = s.read(&mut buf[..want])?;
                if n == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "The upload stream ended before the payload hash was calculated."));
                }
                hasher.update(&buf[..n]);
                remaining -= n as u64;
            }
            Ok(())
        })();
        let _ = s.seek(SeekFrom::Start(original));
        match result {
            Ok(()) => Ok(hex::encode(hasher.finalize())),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Err(UploadError::Cancelled),
            Err(e) => exception(e),
        }
    }

    /// `GetEmptyPayloadHash`.
    fn empty_payload_hash(&self) -> String {
        if self.settings.SignedPayload { sha256_hex(b"") } else { UNSIGNED_PAYLOAD.to_owned() }
    }

    /// `GetRegion`: the setting, else from an `*.amazonaws.com` endpoint, else
    /// `us-east-1`.
    pub fn region(&self) -> String {
        if let Some(r) = self.settings.Region.as_deref().filter(|r| !r.is_empty()) {
            return r.to_owned();
        }
        let mut url = self.endpoint();
        if let Some(i) = url.find("//") {
            url = &url[i + 2..];
        }
        let url = url.strip_suffix('/').unwrap_or(url);
        let Some(aws) = url.find(".amazonaws.com") else { return DEFAULT_REGION.to_owned() };
        let mut service_and_region = url[..aws].to_owned();
        if let Some(rest) = service_and_region.strip_prefix("s3-") {
            service_and_region = format!("s3.{rest}");
        }
        match service_and_region.rfind('.') {
            Some(i) => service_and_region[i + 1..].to_owned(),
            None => DEFAULT_REGION.to_owned(),
        }
    }

    /// `GetUploadPath`: the object prefix (a name pattern) and the file name.
    pub fn get_upload_path(&self, file_name: &str) -> UResult<String> {
        let prefix = self.settings.ObjectPrefix.as_deref().ok_or(UploadError::Exception(super::NULL_REFERENCE.into()))?;
        let path = NameParser::new(NameParserType::FilePath).parse(prefix.trim_matches('/'));
        let s = &self.settings;
        let mut name = file_name.to_owned();
        if (s.RemoveExtensionImage && file_helpers::is_image_file(file_name))
            || (s.RemoveExtensionText && file_helpers::is_text_file(file_name))
            || (s.RemoveExtensionVideo && file_helpers::is_video_file(file_name))
        {
            name = file_name_without_extension(file_name);
        }
        Ok(url_helpers::combine_url(&path, &name))
    }

    /// `GenerateURL`: where the object will be.
    pub fn generate_url(&self, upload_path: &str) -> String {
        if !self.endpoint().is_empty() && !self.bucket().is_empty() {
            let path = url_helpers::url_encode_with_options(upload_path, true);
            return url_helpers::combine_url(&self.object_base_url(), &path);
        }
        String::new()
    }

    /// `GetObjectBaseURL`: the custom domain (it may use the syntax), else
    /// endpoint/bucket.
    fn object_base_url(&self) -> String {
        let url = match self.settings.CustomDomain.as_deref().filter(|d| self.settings.UseCustomCNAME && !d.is_empty()) {
            Some(domain) => CustomUploaderParser::default().parse(domain).unwrap_or_default(),
            None => url_helpers::combine_url(self.endpoint(), self.bucket()),
        };
        url_helpers::fix_prefix(&url)
    }

    /// `GetPreviewURL`.
    pub fn preview_url(&self) -> String {
        self.get_upload_path("example.png").map(|p| self.generate_url(&p)).unwrap_or_default()
    }
}

/// `Path.GetFileNameWithoutExtension`.
fn file_name_without_extension(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(i) => name[..i].to_owned(),
        None => name.to_owned(),
    }
}

/// `AmazonS3ObjectInfo`: a key of a listing (a folder is a common prefix).
#[derive(Debug, Clone, PartialEq)]
pub struct AmazonS3ObjectInfo {
    pub key: String,
    pub is_folder: bool,
    pub size: Option<i64>,
    pub last_modified: Option<chrono::DateTime<chrono::FixedOffset>>,
}

struct ListPage {
    objects: Vec<AmazonS3ObjectInfo>,
    is_truncated: bool,
    next_continuation_token: Option<String>,
}

/// `NormalizeObjectKey`: `\` as `/`, no leading slash.
pub fn normalize_object_key(key: &str) -> String {
    key.replace('\\', "/").trim_start_matches('/').to_owned()
}

/// `XElement.Value`: the text of the element and its descendants.
fn element_value(node: roxmltree::Node) -> String {
    node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect()
}

/// `GetElementValue`: the first child element with that local name.
fn child_value(parent: roxmltree::Node, local_name: &str) -> Option<String> {
    parent.children().find(|c| c.is_element() && c.tag_name().name() == local_name).map(element_value)
}

/// `DateTimeOffset.TryParse(s, InvariantCulture, AssumeUniversal)` for the
/// ISO dates S3 sends.
fn parse_date_time_offset(s: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let s = s.trim();
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(d);
    }
    for f in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%dT%H:%M", "%Y-%m-%d"] {
        if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s, f) {
            return Some(n.and_utc().fixed_offset());
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, f) {
            return Some(d.and_hms_opt(0, 0, 0)?.and_utc().fixed_offset());
        }
    }
    chrono::DateTime::parse_from_rfc2822(s).ok()
}

/// `ParseListObjectsResponse`.
fn parse_list_objects_response(response: &str) -> Option<ListPage> {
    let doc = roxmltree::Document::parse(response).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "ListBucketResult" {
        return None;
    }
    let is_truncated = child_value(root, "IsTruncated").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    let mut page = ListPage { objects: Vec::new(), is_truncated, next_continuation_token: child_value(root, "NextContinuationToken") };
    let decode = |v: Option<String>| v.map(|v| if v.is_empty() { v } else { url_helpers::url_decode(&v, 1) });
    for prefix in root.children().filter(|c| c.is_element() && c.tag_name().name() == "CommonPrefixes") {
        if let Some(key) = decode(child_value(prefix, "Prefix")).filter(|k| !k.is_empty()) {
            page.objects.push(AmazonS3ObjectInfo { key, is_folder: true, size: None, last_modified: None });
        }
    }
    for content in root.children().filter(|c| c.is_element() && c.tag_name().name() == "Contents") {
        let Some(key) = decode(child_value(content, "Key")).filter(|k| !k.is_empty()) else { continue };
        let size = child_value(content, "Size").and_then(|s| {
            let t = s.trim();
            t.strip_prefix('+').unwrap_or(t).parse().ok()
        });
        let last_modified = child_value(content, "LastModified").and_then(|d| parse_date_time_offset(&d));
        page.objects.push(AmazonS3ObjectInfo { key, is_folder: false, size, last_modified });
    }
    Some(page)
}

/// `GetRequestURL`.
fn get_request_url(request_data: &RequestData, canonical_query_string: &str) -> String {
    let url = url_helpers::fix_prefix(&url_helpers::combine_url(&format!("{}{}", request_data.scheme, request_data.host), &request_data.canonical_uri));
    if canonical_query_string.is_empty() { url } else { format!("{url}?{canonical_query_string}") }
}

/// `GetMultipartPartSize`.
fn multipart_part_size(length: u64) -> u64 {
    if length == 0 {
        return 0;
    }
    let required = length.div_ceil(MAXIMUM_PART_COUNT);
    let mut part_size = DEFAULT_PART_SIZE.max(MINIMUM_PART_SIZE.max(required));
    let remainder = part_size % (1024 * 1024);
    if remainder != 0 {
        part_size += 1024 * 1024 - remainder;
    }
    if part_size <= MAXIMUM_PART_SIZE { part_size } else { 0 }
}

/// Elements with a local name, in document order (`DescendantsAndSelf`).
fn xml_find<'a>(doc: &'a roxmltree::Document<'a>, local_name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    doc.root_element().descendants().find(|n| n.is_element() && n.tag_name().name() == local_name)
}

/// `GetUploadID`.
fn get_upload_id(response: Option<&str>) -> Option<String> {
    let response = response.filter(|r| !r.trim().is_empty())?;
    let doc = roxmltree::Document::parse(response).ok()?;
    xml_find(&doc, "UploadId").map(|n| n.descendants().filter(|d| d.is_text()).map(|d| d.text().unwrap_or("")).collect::<String>().trim().to_owned())
}

/// `IsAmazonS3ErrorResponse`: an `Error` element, or a response that isn't XML.
fn is_s3_error_response(response: Option<&str>) -> bool {
    let Some(response) = response.filter(|r| !r.trim().is_empty()) else { return false };
    match roxmltree::Document::parse(response) {
        Ok(doc) => xml_find(&doc, "Error").is_some(),
        Err(_) => true,
    }
}

/// `CreateCompleteMultipartUploadPayload` (`XElement.ToString(DisableFormatting)`).
fn complete_multipart_upload_payload(parts: &[(u32, String)]) -> String {
    let mut s = String::from("<CompleteMultipartUpload>");
    for (number, etag) in parts {
        s.push_str(&format!("<Part><PartNumber>{number}</PartNumber><ETag>{}</ETag></Part>", xml_text(etag)));
    }
    s.push_str("</CompleteMultipartUpload>");
    s
}

/// Text content as `XmlWriter` escapes it.
fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `CreateUploadResult`.
fn create_upload_result(url: &str) -> UploadResult {
    UploadResult { success: true, url: Some(url.to_owned()), ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> AmazonS3Settings {
        AmazonS3Settings {
            AccessKeyID: Some("AKIDEXAMPLE".into()),
            SecretAccessKey: crate::dpapi::Secret::new("wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY"),
            Endpoint: Some("s3.eu-west-2.amazonaws.com".into()),
            Bucket: Some("my-bucket".into()),
            ObjectPrefix: Some("/shots/".into()),
            ..Default::default()
        }
    }

    #[test]
    fn regions_and_urls() {
        let mut s3 = AmazonS3::new(settings());
        assert_eq!(s3.region(), "eu-west-2");
        s3.settings.Endpoint = Some("https://s3-us-west-1.amazonaws.com/".into());
        assert_eq!(s3.region(), "us-west-1");
        s3.settings.Endpoint = Some("nyc3.digitaloceanspaces.com".into());
        assert_eq!(s3.region(), "us-east-1");
        let s3 = AmazonS3::new(settings());
        assert_eq!(s3.get_upload_path("a b.png").unwrap(), "shots/a b.png");
        assert_eq!(s3.generate_url("shots/a b.png"), "https://s3.eu-west-2.amazonaws.com/my-bucket/shots/a%20b.png");
        let rd = s3.create_request_data("shots/a b.png").unwrap();
        assert_eq!(rd.host, "my-bucket.s3.eu-west-2.amazonaws.com");
        assert_eq!(rd.canonical_uri, "/shots/a%20b.png");
        assert_eq!(get_request_url(&rd, "uploads="), "https://my-bucket.s3.eu-west-2.amazonaws.com/shots/a%20b.png?uploads=");
        assert_eq!(multipart_part_size(1), DEFAULT_PART_SIZE);
        assert_eq!(get_upload_id(Some("<InitiateMultipartUploadResult><UploadId> abc </UploadId></InitiateMultipartUploadResult>")).as_deref(), Some("abc"));
        assert!(is_s3_error_response(Some("<Error><Code>x</Code></Error>")));
        assert!(is_s3_error_response(Some("not xml")));
        assert!(!is_s3_error_response(Some("<CompleteMultipartUploadResult/>")));
    }

    #[test]
    fn signs_like_aws_sigv4() {
        // AWS's documented GET Object example (Signature Version 4, header-based auth).
        let mut s3 = AmazonS3::new(AmazonS3Settings {
            AccessKeyID: Some("AKIAIOSFODNN7EXAMPLE".into()),
            SecretAccessKey: crate::dpapi::Secret::new("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
            Endpoint: Some("s3.amazonaws.com".into()),
            Bucket: Some("examplebucket".into()),
            SignedPayload: true,
            ..Default::default()
        });
        s3.now = || chrono::DateTime::parse_from_rfc3339("2013-05-24T00:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let rd = s3.create_request_data("test.txt").unwrap();
        let headers = s3.create_signed_request_headers(
            &HttpMethod::GET,
            &rd,
            "",
            &s3.empty_payload_hash(),
            None,
            Some(Headers::new().with("Range", "bytes=0-9")),
        );
        let auth = headers.get("Authorization").unwrap();
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request,SignedHeaders=host;range;x-amz-content-sha256;x-amz-date,Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }
}
