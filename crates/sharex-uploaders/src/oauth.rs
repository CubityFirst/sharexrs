//! `OAuth/OAuth2Info.cs`, `OAuth2Token.cs`, `OAuthInfo.cs`,
//! `OAuthUserInfo.cs`: the OAuth accounts `UploadersConfig` keeps.
//!
//! Only the stored shapes and token checks are ported; the sign-in flows
//! (`OAuthManager`, `OAuthListener`, `GoogleOAuth2`, the PIN and loopback
//! windows) aren't yet. A token ShareX saved works as is.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sharex_settings::{dotnet_enum, settings};

use crate::dpapi::Secret;

settings! {
    /// `OAuth2Token`.
    pub struct OAuth2Token {
        access_token: Secret = Secret::null(),
        refresh_token: Secret = Secret::null(),
        expires_in: i64 = 0,
        token_type: Option<String> = None,
        scope: Option<String> = None,
        /// `DateTime`, as Newtonsoft writes it (`DateTimeZoneHandling.Utc`).
        ExpireDate: Value = Value::String("0001-01-01T00:00:00".into()),
    }
}

impl OAuth2Token {
    /// `IsExpired`: no expiry date, or past it.
    pub fn is_expired(&self) -> bool {
        match self.expire_date() {
            None => true,
            Some(d) => Utc::now() > d,
        }
    }

    fn expire_date(&self) -> Option<DateTime<Utc>> {
        let s = self.ExpireDate.as_str()?;
        if s.starts_with("0001-01-01T00:00:00") {
            return None;
        }
        DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .ok()
            .or_else(|| NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S%.f").ok().map(|n| n.and_utc()))
    }

    /// `UpdateExpireDate()`: now plus `expires_in` less 10 seconds.
    pub fn update_expire_date(&mut self) {
        let date = Utc::now() + chrono::Duration::seconds(self.expires_in - 10);
        self.ExpireDate = Value::String(date.format("%Y-%m-%dT%H:%M:%S%.7fZ").to_string());
    }

    /// Reads a token response (`JsonConvert.DeserializeObject<OAuth2Token>`).
    pub fn from_response(text: &str) -> Option<Self> {
        let v = crate::json::deserialize(text).ok()??;
        let s = |k: &str| crate::json::get_str(&v, k);
        Some(OAuth2Token {
            access_token: s("access_token").map(Secret::new).unwrap_or_default(),
            refresh_token: s("refresh_token").map(Secret::new).unwrap_or_default(),
            expires_in: crate::json::get_i64(&v, "expires_in"),
            token_type: s("token_type"),
            scope: s("scope"),
            ..Default::default()
        })
    }
}

settings! {
    /// `OAuth2Info`.
    pub struct OAuth2Info {
        Client_ID: Option<String> = None,
        Client_Secret: Option<String> = None,
        Token: Option<OAuth2Token> = None,
        /// `OAuth2ProofKey` (PKCE), only during sign-in.
        Proof: Value = Value::Null,
    }
}

impl OAuth2Info {
    /// `new OAuth2Info(client_id, client_secret)`.
    pub fn new(client_id: &str, client_secret: &str) -> Self {
        Self { Client_ID: Some(client_id.to_owned()), Client_Secret: Some(client_secret.to_owned()), ..Default::default() }
    }

    /// `OAuth2Info.CheckOAuth`: client and an access token.
    pub fn check(oauth: Option<&OAuth2Info>) -> bool {
        oauth.is_some_and(|o| {
            o.Client_ID.as_deref().is_some_and(|s| !s.is_empty())
                && o.Client_Secret.as_deref().is_some_and(|s| !s.is_empty())
                && o.Token.as_ref().is_some_and(|t| !t.access_token.is_empty())
        })
    }

    /// The access token (empty without one).
    pub fn access_token(&self) -> &str {
        self.Token.as_ref().map(|t| t.access_token.as_str()).unwrap_or("")
    }
}

dotnet_enum! {
    /// `OAuthInfo.OAuthInfoSignatureMethod`.
    pub enum OAuthInfoSignatureMethod { HMAC_SHA1, RSA_SHA1 }
}

settings! {
    /// `OAuthInfo` (OAuth 1.0: Flickr, Photobucket).
    pub struct OAuthInfo {
        Description: Option<String> = None,
        OAuthVersion: Option<String> = Some("1.0".into()),
        ConsumerKey: Option<String> = None,
        ConsumerSecret: Option<String> = None,
        ConsumerPrivateKey: Option<String> = None,
        SignatureMethod: OAuthInfoSignatureMethod = OAuthInfoSignatureMethod::HMAC_SHA1,
        AuthToken: Option<String> = None,
        AuthSecret: Secret = Secret::null(),
        AuthVerifier: Secret = Secret::null(),
        UserToken: Option<String> = None,
        UserSecret: Secret = Secret::null(),
    }
}

impl OAuthInfo {
    /// `OAuthInfo.CheckOAuth`.
    pub fn check(oauth: Option<&OAuthInfo>) -> bool {
        let ne = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.is_empty());
        oauth.is_some_and(|o| {
            ne(&o.ConsumerKey)
                && ((o.SignatureMethod == OAuthInfoSignatureMethod::HMAC_SHA1 && ne(&o.ConsumerSecret))
                    || (o.SignatureMethod == OAuthInfoSignatureMethod::RSA_SHA1 && ne(&o.ConsumerPrivateKey)))
                && ne(&o.UserToken)
                && !o.UserSecret.is_empty()
        })
    }
}

settings! {
    /// `OAuthUserInfo`.
    pub struct OAuthUserInfo {
        sub: Option<String> = None,
        name: Option<String> = None,
        given_name: Option<String> = None,
        picture: Option<String> = None,
        locale: Option<String> = None,
    }
}
