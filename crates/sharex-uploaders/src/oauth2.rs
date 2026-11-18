//! OAuth 2 sign-in: `OAuth/IOAuth2Basic.cs`, `IOAuth2.cs`,
//! `IOauth2Loopback.cs` (as traits), `OAuth2ProofKey.cs` (PKCE),
//! `GoogleOAuth2.cs` and `OAuthListener.cs` (the loopback redirect a
//! local HTTP listener catches).
//!
//! The flows, as ShareX's Destination settings run them:
//! - **code paste** ([`OAuth2Client`]: Imgur's PIN, Dropbox, Box, GitHub
//!   Gist, OneDrive with PKCE, bit.ly): open [`OAuth2Client::authorization_url`]
//!   in the browser, the user pastes the code shown, then
//!   [`OAuth2Client::get_access_token`]; `refresh_access_token` for
//!   "Refresh token".
//! - **loopback** ([`OAuth2Loopback`]: Google Drive, Google Cloud Storage,
//!   YouTube): [`OAuthListener::bind`] a local port, set the redirect URI
//!   and state, open the page, [`OAuthListener::wait_for_code`], then the
//!   token and the user's name ([`connect`] does it all).
//!
//! Tokens are kept in the destination's `OAuth2Info` (as ShareX stores
//! them in `UploadersConfig.json`); the caller writes the client's
//! `auth_info()` back into the config.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};
use sharex_strings::uploaders_lib as strings;

use crate::args;
use crate::enums::HttpMethod;
use crate::http::CancellationToken;
use crate::json::{self, get_str};
use crate::oauth::{OAuth2Info, OAuth2Token, OAuthInfo, OAuthUserInfo};
use crate::uploader::{Headers, UResult, UploadError, Uploader};
use crate::url_helpers;

/// `OAuth2ChallengeMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuth2ChallengeMethod {
    Plain,
    Sha256,
}

/// `OAuth2ProofKey` (PKCE, RFC 7636): a random verifier and its challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuth2ProofKey {
    pub code_verifier: String,
    pub code_challenge: String,
    pub method: OAuth2ChallengeMethod,
}

fn clean_base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes).replace('+', "-").replace('/', "_").replace('=', "")
}

impl OAuth2ProofKey {
    /// `new OAuth2ProofKey(method)`: 32 random bytes.
    pub fn new(method: OAuth2ChallengeMethod) -> Self {
        let mut buffer = [0u8; 32];
        getrandom::fill(&mut buffer).expect("OS random numbers");
        Self::from_bytes(method, &buffer)
    }

    /// The proof key of given random bytes.
    pub fn from_bytes(method: OAuth2ChallengeMethod, buffer: &[u8]) -> Self {
        let code_verifier = clean_base64(buffer);
        let code_challenge = match method {
            OAuth2ChallengeMethod::Plain => code_verifier.clone(),
            OAuth2ChallengeMethod::Sha256 => clean_base64(&Sha256::digest(code_verifier.as_bytes())),
        };
        Self { code_verifier, code_challenge, method }
    }

    /// `ChallengeMethod`.
    pub fn challenge_method(&self) -> &'static str {
        match self.method {
            OAuth2ChallengeMethod::Plain => "plain",
            OAuth2ChallengeMethod::Sha256 => "S256",
        }
    }

    /// As `OAuth2Info.Proof` holds it in `UploadersConfig.json`.
    pub fn to_value(&self) -> Value {
        json::object([
            ("CodeVerifier", Value::String(self.code_verifier.clone())),
            ("CodeChallenge", Value::String(self.code_challenge.clone())),
            ("ChallengeMethod", Value::String(self.challenge_method().into())),
        ])
    }

    /// Reads `OAuth2Info.Proof`.
    pub fn from_value(v: &Value) -> Option<Self> {
        if !v.is_object() {
            return None;
        }
        let method = match get_str(v, "ChallengeMethod").as_deref() {
            Some("S256") => OAuth2ChallengeMethod::Sha256,
            _ => OAuth2ChallengeMethod::Plain,
        };
        Some(Self { code_verifier: get_str(v, "CodeVerifier").unwrap_or_default(), code_challenge: get_str(v, "CodeChallenge").unwrap_or_default(), method })
    }
}

/// `IOAuth2Basic` / `IOAuth2`: a destination's code-paste sign-in.
pub trait OAuth2Client {
    /// `AuthInfo`: the client and, once signed in, the token.
    fn auth_info(&self) -> &OAuth2Info;
    /// `GetAuthorizationURLAsync`: the page to open in the browser.
    fn authorization_url(&mut self) -> UResult<String>;
    /// `GetAccessTokenAsync(code)`: exchanges the code (PIN) the page showed.
    fn get_access_token(&mut self, code: &str) -> UResult<bool>;
    /// `RefreshAccessTokenAsync` (`IOAuth2`; `false` for `IOAuth2Basic`).
    fn refresh_access_token(&mut self) -> UResult<bool> {
        Ok(false)
    }
    /// Whether ShareX's class is `IOAuth2` (has refresh) or only `IOAuth2Basic`.
    fn can_refresh(&self) -> bool {
        false
    }
    /// `CheckAuthorizationAsync`: signed in, refreshing an expired token
    /// (most services' errors; Imgur and Box name the service).
    fn check_authorization(&mut self) -> UResult<bool> {
        let info = self.auth_info().clone();
        if OAuth2Info::check(Some(&info)) {
            let expired = info.Token.as_ref().is_none_or(OAuth2Token::is_expired);
            if expired && !self.refresh_access_token()? {
                self.uploader().errors.add(strings::UploaderErrors_Refresh_access_token_failed());
                return Ok(false);
            }
            Ok(true)
        } else {
            self.uploader().errors.add(strings::UploaderErrors_Login_is_required());
            Ok(false)
        }
    }
    /// The uploader the requests go through (its errors, stop handle).
    fn uploader(&mut self) -> &mut Uploader;
}

/// `IOAuth2Loopback`: Google's sign-in with a local redirect.
pub trait OAuth2Loopback: OAuth2Client {
    fn set_redirect_uri(&mut self, uri: &str);
    fn set_state(&mut self, state: &str);
    /// `GetUserInfoAsync`.
    fn get_user_info(&mut self) -> UResult<Option<OAuthUserInfo>>;
}

/// `IOAuth`: OAuth 1.0a sign-in (Flickr, Photobucket): open the page, the
/// user pastes the verification code.
pub trait OAuth1Client {
    fn auth_info(&self) -> &OAuthInfo;
    /// `GetAuthorizationURLAsync`: gets a request token, gives the page.
    fn authorization_url(&mut self) -> UResult<Option<String>>;
    /// `GetAccessTokenAsync(verificationCode)`.
    fn get_access_token(&mut self, verification_code: &str) -> UResult<bool>;
    fn uploader(&mut self) -> &mut Uploader;
}

/// Reads a token response as ShareX does (`JsonConvert.DeserializeObject<OAuth2Token>`):
/// a token with an access token, or `None`.
pub fn parse_token(response: Option<&str>) -> Option<OAuth2Token> {
    let response = response.filter(|r| !r.is_empty())?;
    OAuth2Token::from_response(response).filter(|t| !t.access_token.is_empty())
}

/// `OAuth2Info.CheckOAuth` then the expiry check most `CheckAuthorizationAsync`
/// implementations share: refresh an expired token, else add ShareX's error.
pub fn check_authorization(
    base: &mut Uploader,
    info: &OAuth2Info,
    login_required_error: &str,
    refresh: impl FnOnce(&mut Uploader) -> UResult<bool>,
) -> UResult<bool> {
    if OAuth2Info::check(Some(info)) {
        let expired = info.Token.as_ref().is_none_or(OAuth2Token::is_expired);
        if expired && !refresh(base)? {
            base.errors.add(strings::UploaderErrors_Refresh_access_token_failed());
            return Ok(false);
        }
        Ok(true)
    } else {
        base.errors.add(login_required_error);
        Ok(false)
    }
}

/// `Authorization: Bearer <token>`.
pub fn bearer_headers(info: &OAuth2Info) -> Headers {
    Headers::new().with("Authorization", format!("Bearer {}", info.access_token()))
}

const GOOGLE_AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER_INFO_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

/// `GoogleOAuth2`: the loopback flow for Google's services; its requests go
/// through the destination's uploader.
#[derive(Debug, Clone)]
pub struct GoogleOAuth2 {
    pub auth_info: OAuth2Info,
    pub redirect_uri: String,
    pub state: String,
    pub scope: String,
}

impl GoogleOAuth2 {
    pub fn new(auth_info: OAuth2Info, scope: &str) -> Self {
        Self { auth_info, redirect_uri: String::new(), state: String::new(), scope: scope.to_owned() }
    }

    fn client_id(&self) -> String {
        self.auth_info.Client_ID.clone().unwrap_or_default()
    }

    fn client_secret(&self) -> String {
        self.auth_info.Client_Secret.clone().unwrap_or_default()
    }

    /// `GetAuthorizationURLAsync`.
    pub fn authorization_url(&self) -> String {
        let args = args![
            ("response_type", "code"),
            ("client_id", self.client_id()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("state", self.state.clone()),
            ("scope", self.scope.clone()),
        ];
        url_helpers::create_query_string_url(GOOGLE_AUTHORIZATION_ENDPOINT, &args, false)
    }

    /// `GetAccessTokenAsync(code)`.
    pub fn get_access_token(&mut self, base: &mut Uploader, code: &str) -> UResult<bool> {
        let args = args![
            ("code", code),
            ("client_id", self.client_id()),
            ("client_secret", self.client_secret()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("grant_type", "authorization_code"),
        ];
        let response = base.send_request_url_encoded(HttpMethod::POST, GOOGLE_TOKEN_ENDPOINT, &args, None)?;
        if let Some(mut token) = parse_token(response.as_deref()) {
            token.update_expire_date();
            self.auth_info.Token = Some(token);
            return Ok(true);
        }
        Ok(false)
    }

    /// `RefreshAccessTokenAsync`: a new access token, the refresh token kept.
    pub fn refresh_access_token(&mut self, base: &mut Uploader) -> UResult<bool> {
        let refresh = self.auth_info.Token.as_ref().map(|t| t.refresh_token.as_str().to_owned()).unwrap_or_default();
        if OAuth2Info::check(Some(&self.auth_info)) && !refresh.is_empty() {
            let args = args![
                ("refresh_token", refresh.clone()),
                ("client_id", self.client_id()),
                ("client_secret", self.client_secret()),
                ("grant_type", "refresh_token"),
            ];
            let response = base.send_request_url_encoded(HttpMethod::POST, GOOGLE_TOKEN_ENDPOINT, &args, None)?;
            if let Some(mut token) = parse_token(response.as_deref()) {
                token.update_expire_date();
                token.refresh_token = crate::dpapi::Secret::new(refresh);
                self.auth_info.Token = Some(token);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `CheckAuthorizationAsync`.
    pub fn check_authorization(&mut self, base: &mut Uploader) -> UResult<bool> {
        let info = self.auth_info.clone();
        check_authorization(base, &info, strings::UploaderErrors_Login_is_required(), |b| self.refresh_access_token(b))
    }

    /// `GetAuthHeaders`.
    pub fn auth_headers(&self) -> Headers {
        bearer_headers(&self.auth_info)
    }

    /// `GetUserInfoAsync`.
    pub fn get_user_info(&self, base: &mut Uploader) -> UResult<Option<OAuthUserInfo>> {
        let headers = self.auth_headers();
        let response = base.send_request(HttpMethod::GET, GOOGLE_USER_INFO_ENDPOINT, &[], Some(&headers))?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let v = crate::destinations::deserialize_object(Some(&r))?;
            return Ok(v.map(|v| OAuthUserInfo {
                sub: get_str(&v, "sub"),
                name: get_str(&v, "name"),
                given_name: get_str(&v, "given_name"),
                picture: get_str(&v, "picture"),
                locale: get_str(&v, "locale"),
                ..Default::default()
            }));
        }
        Ok(None)
    }
}

/// The page the browser shows after the redirect (`Resources/OAuthCallbackPage.html`,
/// under ShareXrs's name).
pub const OAUTH_CALLBACK_PAGE: &str = include_str!("oauth_callback_page.html");

/// `OAuthListener`: a one-request HTTP server on 127.0.0.1 that catches
/// the authorization redirect.
pub struct OAuthListener {
    listener: TcpListener,
    /// `http://127.0.0.1:<port>/`.
    pub redirect_uri: String,
    /// `Helpers.GetRandomAlphanumeric(32)`.
    pub state: String,
}

/// What the redirect carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCallback {
    pub code: Option<String>,
    pub state: Option<String>,
}

impl OAuthListener {
    /// Binds a free port on 127.0.0.1 (`WebHelpers.GetRandomUnusedPort`)
    /// and makes the state.
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        Ok(Self { listener, redirect_uri: format!("http://127.0.0.1:{port}/"), state: crate::oauth1::random_alphanumeric(32) })
    }

    /// Waits for the browser's redirect, answers it with the callback page
    /// and gives the code if the state matches (`None` when stopped or the
    /// state or code is wrong).
    pub fn wait_for_code(&self, cancel: &CancellationToken) -> std::io::Result<Option<String>> {
        self.listener.set_nonblocking(true)?;
        loop {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let Some(callback) = Self::read_request(&stream)? else { continue };
                    let status = if callback.state.as_deref() != Some(self.state.as_str()) {
                        strings::OAuthListener_Invalid_state_parameter()
                    } else if callback.code.as_deref().is_some_and(|c| !c.is_empty()) {
                        strings::OAuthListener_Authorization_completed_successfully()
                    } else {
                        strings::OAuthListener_Authorization_did_not_succeed()
                    };
                    Self::respond(stream, &OAUTH_CALLBACK_PAGE.replace("{0}", status))?;
                    return Ok(if callback.state.as_deref() == Some(self.state.as_str()) { callback.code.filter(|c| !c.is_empty()) } else { None });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(e),
            }
        }
    }

    /// Reads the request line's query (`HttpListenerRequest.QueryString`).
    fn read_request(stream: &TcpStream) -> std::io::Result<Option<OAuthCallback>> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        // Drain the headers.
        let mut header = String::new();
        while reader.read_line(&mut header)? > 2 {
            header.clear();
        }
        let target = line.split_whitespace().nth(1).unwrap_or("");
        if target.is_empty() {
            return Ok(None);
        }
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
        let pairs = url_helpers::parse_query(query);
        let value = |name: &str| pairs.iter().find(|(k, _)| k.as_deref() == Some(name)).map(|(_, v)| v.join(","));
        Ok(Some(OAuthCallback { code: value("code"), state: value("state") }))
    }

    fn respond(mut stream: TcpStream, page: &str) -> std::io::Result<()> {
        let body = page.as_bytes();
        let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        stream.write_all(head.as_bytes())?;
        stream.write_all(body)?;
        stream.flush()?;
        let _ = stream.shutdown(Shutdown::Write);
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest);
        Ok(())
    }
}

/// `OAuthListener.ConnectAsync` + the listener window's user info
/// (`OAuthListenerWindow.ConnectAsync`): binds the listener, opens the
/// authorization page with `open_url`, waits for the code, exchanges it and
/// reads the user's name. `None` when stopped or refused.
pub fn connect(oauth: &mut dyn OAuth2Loopback, open_url: &dyn Fn(&str), cancel: &CancellationToken) -> UResult<Option<Option<OAuthUserInfo>>> {
    let listener = OAuthListener::bind().map_err(|e| UploadError::Exception(e.to_string()))?;
    oauth.set_redirect_uri(&listener.redirect_uri);
    oauth.set_state(&listener.state);
    let url = oauth.authorization_url()?;
    if url.is_empty() {
        return Ok(None);
    }
    open_url(&url);
    let code = listener.wait_for_code(cancel).map_err(|e| UploadError::Exception(e.to_string()))?;
    drop(listener);
    let Some(code) = code else { return Ok(None) };
    if !oauth.get_access_token(&code)? {
        return Ok(None);
    }
    Ok(Some(oauth.get_user_info()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_keys_follow_rfc7636() {
        // RFC 7636 appendix B.
        let bytes = [
            116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212, 37, 77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132,
            141, 121,
        ];
        let p = OAuth2ProofKey::from_bytes(OAuth2ChallengeMethod::Sha256, &bytes);
        assert_eq!(p.code_verifier, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(p.code_challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(OAuth2ProofKey::from_value(&p.to_value()), Some(p));
    }

    #[test]
    fn listener_takes_the_redirect() {
        let listener = OAuthListener::bind().unwrap();
        let url = format!("{}?state={}&code=4%2F0abc", listener.redirect_uri, listener.state);
        let t = std::thread::spawn(move || {
            let addr = url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
            let mut s = TcpStream::connect(addr).unwrap();
            let path = &url[url.find("/?").unwrap()..];
            write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut page = String::new();
            s.read_to_string(&mut page).unwrap();
            page
        });
        let code = listener.wait_for_code(&CancellationToken::new()).unwrap();
        assert_eq!(code.as_deref(), Some("4/0abc"));
        let page = t.join().unwrap();
        assert!(page.starts_with("HTTP/1.1 200 OK"), "{page}");
        assert!(page.contains(strings::OAuthListener_Authorization_completed_successfully()));
    }

    #[test]
    fn listener_refuses_a_wrong_state() {
        let listener = OAuthListener::bind().unwrap();
        let addr = listener.redirect_uri.trim_start_matches("http://").trim_end_matches('/').to_owned();
        let t = std::thread::spawn(move || {
            let mut s = TcpStream::connect(addr).unwrap();
            write!(s, "GET /?state=nope&code=x HTTP/1.1\r\n\r\n").unwrap();
            let mut page = String::new();
            s.read_to_string(&mut page).unwrap();
            page
        });
        assert_eq!(listener.wait_for_code(&CancellationToken::new()).unwrap(), None);
        assert!(t.join().unwrap().contains(strings::OAuthListener_Invalid_state_parameter()));
        let stop = CancellationToken::new();
        stop.cancel();
        assert_eq!(OAuthListener::bind().unwrap().wait_for_code(&stop).unwrap(), None);
    }
}
