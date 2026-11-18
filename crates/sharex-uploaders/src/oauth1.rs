//! `OAuth/OAuthManager.cs` and the OAuth 1.0a helpers of `Uploader.cs`
//! (`GetAuthorizationURLAsync`, `GetAccessTokenExAsync`): HMAC-SHA1 signed
//! requests for Flickr and Photobucket.

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sharex_helpers::random;
use sharex_strings::uploaders_lib as strings;

use crate::enums::HttpMethod;
use crate::oauth::{OAuthInfo, OAuthInfoSignatureMethod};
use crate::uploader::{Args, UResult, UploadError, Uploader, exception};
use crate::url_helpers;

/// A parsed `application/x-www-form-urlencoded` response (`NameValueCollection`).
pub type QueryValues = Vec<(Option<String>, Vec<String>)>;

const PARAMETER_CONSUMER_KEY: &str = "oauth_consumer_key";
const PARAMETER_SIGNATURE_METHOD: &str = "oauth_signature_method";
const PARAMETER_SIGNATURE: &str = "oauth_signature";
const PARAMETER_TIMESTAMP: &str = "oauth_timestamp";
const PARAMETER_NONCE: &str = "oauth_nonce";
const PARAMETER_VERSION: &str = "oauth_version";
const PARAMETER_TOKEN: &str = "oauth_token";
const PARAMETER_TOKEN_SECRET: &str = "oauth_token_secret";
const PARAMETER_VERIFIER: &str = "oauth_verifier";
/// `OAuthManager.ParameterCallback`.
pub const PARAMETER_CALLBACK: &str = "oauth_callback";

fn ne(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

/// `Dictionary<string, string>` with `this[key] = value` (replaces in place).
fn set(params: &mut Args, key: &str, value: String) {
    match params.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value,
        None => params.push((key.to_owned(), value)),
    }
}

/// `OAuthManager.GenerateQuery(url, args, httpMethod, oauth, out parameters)`:
/// the signed URL, and every parameter in the order ShareX adds them (the
/// signature last), for uploads that send them as form fields.
pub fn generate_query(url: &str, args: &[(String, String)], method: HttpMethod, oauth: &OAuthInfo) -> UResult<(String, Args)> {
    let nonce = random_alphanumeric(12);
    let timestamp = timestamp();
    generate_query_with(url, args, method, oauth, &nonce, &timestamp)
}

/// [`generate_query`] with a given nonce and timestamp.
pub fn generate_query_with(url: &str, args: &[(String, String)], method: HttpMethod, oauth: &OAuthInfo, nonce: &str, timestamp: &str) -> UResult<(String, Args)> {
    if !ne(&oauth.ConsumerKey)
        || (oauth.SignatureMethod == OAuthInfoSignatureMethod::HMAC_SHA1 && !ne(&oauth.ConsumerSecret))
        || (oauth.SignatureMethod == OAuthInfoSignatureMethod::RSA_SHA1 && !ne(&oauth.ConsumerPrivateKey))
    {
        return exception(strings::OAuthManager_Consumer_credentials_are_empty());
    }
    let mut parameters: Args = vec![
        (PARAMETER_VERSION.into(), oauth.OAuthVersion.clone().unwrap_or_default()),
        (PARAMETER_NONCE.into(), nonce.to_owned()),
        (PARAMETER_TIMESTAMP.into(), timestamp.to_owned()),
        (PARAMETER_CONSUMER_KEY.into(), oauth.ConsumerKey.clone().unwrap_or_default()),
    ];
    match oauth.SignatureMethod {
        OAuthInfoSignatureMethod::HMAC_SHA1 => parameters.push((PARAMETER_SIGNATURE_METHOD.into(), "HMAC-SHA1".into())),
        OAuthInfoSignatureMethod::RSA_SHA1 => parameters.push((PARAMETER_SIGNATURE_METHOD.into(), "RSA-SHA1".into())),
        _ => return exception(strings::OAuthManager_Unsupported_signature_method()),
    }
    let mut secret: Option<String> = None;
    if ne(&oauth.UserToken) && !oauth.UserSecret.is_empty() {
        secret = Some(oauth.UserSecret.as_str().to_owned());
        parameters.push((PARAMETER_TOKEN.into(), oauth.UserToken.clone().unwrap_or_default()));
    } else if ne(&oauth.AuthToken) && !oauth.AuthSecret.is_empty() {
        secret = Some(oauth.AuthSecret.as_str().to_owned());
        parameters.push((PARAMETER_TOKEN.into(), oauth.AuthToken.clone().unwrap_or_default()));
        if !oauth.AuthVerifier.is_empty() {
            parameters.push((PARAMETER_VERIFIER.into(), oauth.AuthVerifier.as_str().to_owned()));
        }
    }
    for (k, v) in args {
        set(&mut parameters, k, v.clone());
    }
    let normalized_url = normalize_url(url);
    let normalized_parameters = normalize_parameters(&parameters);
    let signature_base = format!(
        "{}&{}&{}",
        method.name(),
        url_helpers::url_encode(&normalized_url, false),
        url_helpers::url_encode(&normalized_parameters, false)
    );
    let signature = match oauth.SignatureMethod {
        OAuthInfoSignatureMethod::HMAC_SHA1 => {
            hmac_sha1_signature(&signature_base, oauth.ConsumerSecret.as_deref().unwrap_or(""), secret.as_deref())
        }
        // ShareX signs a SHA-1 hash as MD5 here, which .NET refuses; no
        // service ShareX supports uses RSA-SHA1.
        _ => return exception("Specified hash algorithm is not supported."),
    };
    set(&mut parameters, PARAMETER_SIGNATURE, signature.clone());
    Ok((format!("{normalized_url}?{normalized_parameters}&{PARAMETER_SIGNATURE}={}", url_helpers::url_encode(&signature, false)), parameters))
}

/// `GenerateSignature`: HMAC-SHA1 keyed with the escaped secrets, base 64.
pub fn hmac_sha1_signature(signature_base: &str, consumer_secret: &str, user_secret: Option<&str>) -> String {
    use base64::Engine;
    let escape = crate::destinations::cloud_storage::uri_escape_data_string;
    let key = format!("{}&{}", escape(consumer_secret), user_secret.filter(|s| !s.is_empty()).map(escape).unwrap_or_default());
    let mut mac = Hmac::<Sha1>::new_from_slice(key.as_bytes()).expect("HMAC takes any key");
    mac.update(signature_base.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// `GenerateTimestamp`: seconds since 1970 (`Convert.ToInt64` rounds).
fn timestamp() -> String {
    let now = chrono::Utc::now();
    let seconds = now.timestamp() as f64 + now.timestamp_subsec_nanos() as f64 / 1e9;
    (seconds.round_ties_even() as i64).to_string()
}

/// `Helpers.GetRandomAlphanumeric(length)`.
pub fn random_alphanumeric(length: usize) -> String {
    (0..length).map(|_| random::char_from(random::ALPHANUMERIC)).collect()
}

/// `NormalizeUrl`: scheme, host, a non-default port and the path.
pub fn normalize_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_owned() };
    let scheme = scheme.to_ascii_lowercase();
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let path = rest[end..].split(['?', '#']).next().unwrap_or("");
    let path = if path.is_empty() { "/" } else { path };
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => (h, p.parse::<u16>().ok()),
        _ => (host_port, None),
    };
    let default = match scheme.as_str() {
        "http" => Some(80),
        "https" => Some(443),
        "ftp" => Some(21),
        _ => None,
    };
    let port = port.or(default);
    let show_port = match scheme.as_str() {
        "http" => port != Some(80),
        "https" => port != Some(443),
        // ShareX compares FTP with port 20.
        "ftp" => port != Some(20),
        _ => false,
    };
    let port = if show_port { port.map(|p| format!(":{p}")).unwrap_or_default() } else { String::new() };
    let path = crate::http::dotnet_absolute_uri(&format!("{scheme}://h{path}"))
        .ok()
        .and_then(|u| u.split_once("://h").map(|(_, p)| p.split('?').next().unwrap_or("").to_owned()))
        .unwrap_or_else(|| path.to_owned());
    format!("{scheme}://{}{port}{path}", host.to_ascii_lowercase())
}

/// `NormalizeParameters`: sorted by key then value, values URL-encoded.
pub fn normalize_parameters(parameters: &[(String, String)]) -> String {
    let mut sorted: Vec<&(String, String)> = parameters.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    sorted.iter().map(|(k, v)| format!("{k}={}", url_helpers::url_encode(v, false))).collect::<Vec<_>>().join("&")
}

/// `HttpUtility.ParseQueryString(text)[name]`: the values joined with `,`.
fn query_value(pairs: &[(Option<String>, Vec<String>)], name: &str) -> Option<String> {
    pairs.iter().find(|(k, _)| k.as_deref().is_some_and(|k| k.eq_ignore_ascii_case(name))).map(|(_, v)| v.join(","))
}

/// `OAuthManager.GetAuthorizationURL`: keeps the request token (and its
/// secret) and gives the authorization page.
pub fn get_authorization_url(request_token_response: &str, oauth: &mut OAuthInfo, authorize_url: &str, callback: Option<&str>) -> Option<String> {
    let pairs = url_helpers::parse_query(request_token_response);
    let token = query_value(&pairs, PARAMETER_TOKEN)?;
    oauth.AuthToken = Some(token.clone());
    let mut url = format!("{authorize_url}?{PARAMETER_TOKEN}={token}");
    if let Some(c) = callback.filter(|c| !c.is_empty()) {
        url.push_str(&format!("&{PARAMETER_CALLBACK}={}", url_helpers::url_encode(c, false)));
    }
    if let Some(secret) = query_value(&pairs, PARAMETER_TOKEN_SECRET) {
        oauth.AuthSecret.set(Some(secret));
    }
    Some(url)
}

/// `OAuthManager.ParseAccessTokenResponse`: keeps the user token and
/// secret; the response's values, or `None`.
pub fn parse_access_token_response(response: &str, oauth: &mut OAuthInfo) -> Option<QueryValues> {
    let pairs = url_helpers::parse_query(response);
    let token = query_value(&pairs, PARAMETER_TOKEN)?;
    oauth.UserToken = Some(token);
    let secret = query_value(&pairs, PARAMETER_TOKEN_SECRET)?;
    oauth.UserSecret.set(Some(secret));
    Some(pairs)
}

/// `Uploader.GetAuthorizationURLAsync(requestTokenURL, authorizeURL,
/// authInfo, customParameters, httpMethod)`.
pub fn request_authorization_url(
    base: &mut Uploader,
    request_token_url: &str,
    authorize_url: &str,
    oauth: &mut OAuthInfo,
    custom_parameters: &[(String, String)],
    method: HttpMethod,
) -> UResult<Option<String>> {
    let (url, _) = generate_query(request_token_url, custom_parameters, method.clone(), oauth)?;
    let response = base.send_request(method, &url, &[], None)?;
    Ok(response.filter(|r| !r.is_empty()).and_then(|r| get_authorization_url(&r, oauth, authorize_url, None)))
}

/// `Uploader.GetAccessTokenExAsync`: the access token response's values.
pub fn request_access_token(
    base: &mut Uploader,
    access_token_url: &str,
    oauth: &mut OAuthInfo,
    method: HttpMethod,
) -> UResult<Option<QueryValues>> {
    if !ne(&oauth.AuthToken) || oauth.AuthSecret.is_empty() {
        return Err(UploadError::Exception(strings::Uploader_Authorization_information_missing().into()));
    }
    let (url, _) = generate_query(access_token_url, &[], method.clone(), oauth)?;
    let response = base.send_request(method, &url, &[], None)?;
    Ok(response.filter(|r| !r.is_empty()).and_then(|r| parse_access_token_response(&r, oauth)))
}

/// A value of an access token response.
pub fn response_value(values: &[(Option<String>, Vec<String>)], name: &str) -> Option<String> {
    query_value(values, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dpapi::Secret;

    #[test]
    fn signs_like_rfc5849() {
        // RFC 5849 section 1.2's example (with ShareX's parameter set).
        let oauth = OAuthInfo {
            ConsumerKey: Some("dpf43f3p2l4k3l03".into()),
            ConsumerSecret: Some("kd94hf93k423kf44".into()),
            UserToken: Some("nnch734d00sl2jdk".into()),
            UserSecret: Secret::new("pfkkdhi9sl3r4s00"),
            ..Default::default()
        };
        let args = vec![("file".to_owned(), "vacation.jpg".to_owned()), ("size".to_owned(), "original".to_owned())];
        let (url, params) = generate_query_with("http://photos.example.net/photos", &args, HttpMethod::GET, &oauth, "kllo9940pd9333jh", "1191242096").unwrap();
        assert_eq!(params.last().unwrap(), &("oauth_signature".to_owned(), "tR3+Ty81lMeYAr/Fid0kMTYa/WM=".to_owned()));
        assert!(url.starts_with("http://photos.example.net/photos?file=vacation.jpg&oauth_consumer_key="), "{url}");
        assert!(url.ends_with("&oauth_signature=tR3%2BTy81lMeYAr%2FFid0kMTYa%2FWM%3D"), "{url}");
        assert_eq!(normalize_url("https://Example.com:443/a b?x=1"), "https://example.com/a%20b");
        assert_eq!(normalize_url("http://example.com:8080/"), "http://example.com:8080/");
    }

    #[test]
    fn reads_token_responses() {
        let mut oauth = OAuthInfo::default();
        let url = get_authorization_url("oauth_token=abc&oauth_token_secret=def&oauth_callback_confirmed=true", &mut oauth, "https://x/authorize", None);
        assert_eq!(url.as_deref(), Some("https://x/authorize?oauth_token=abc"));
        assert_eq!(oauth.AuthSecret.get(), Some("def"));
        let values = parse_access_token_response("oauth_token=u&oauth_token_secret=s&username=me", &mut oauth).unwrap();
        assert_eq!(response_value(&values, "username").as_deref(), Some("me"));
        assert_eq!(oauth.UserSecret.get(), Some("s"));
    }
}
