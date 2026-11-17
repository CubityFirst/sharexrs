//! URL sharing services that open a share page (`SharingServices/
//! SimpleURLSharingService.cs` and its subclasses: Facebook, Reddit,
//! Pinterest, Tumblr, LinkedIn, StumbleUpon, Delicious, VK, Google Lens,
//! Bing visual search).

use std::sync::{Arc, LazyLock, RwLock};

use super::uploader_base;
use crate::enums::URLSharingServices;
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Uploader, WritesBackConfig};
use crate::url_helpers;

/// Opens a URL in the browser (`URLHelpers.OpenURL`).
pub type UrlOpener = Arc<dyn Fn(&str) + Send + Sync>;

static OPENER: LazyLock<RwLock<Option<UrlOpener>>> = LazyLock::new(|| RwLock::new(None));

/// Sets how URLs are opened (the app sets the system browser, honouring
/// `HelpersOptions.BrowserPath`). Without one, nothing is opened.
pub fn set_url_opener(opener: Option<UrlOpener>) {
    *OPENER.write().expect("url opener") = opener;
}

/// `URLHelpers.OpenURL`.
pub fn open_url(url: &str) {
    if url.is_empty() {
        return;
    }
    if let Some(opener) = OPENER.read().expect("url opener").clone() {
        opener(url);
    }
}

/// The share page of a simple sharing service (`URLFormatString`).
pub fn url_format_string(service: &URLSharingServices) -> Option<&'static str> {
    Some(match service {
        URLSharingServices::Facebook => "https://www.facebook.com/sharer/sharer.php?u={0}",
        URLSharingServices::Reddit => "http://www.reddit.com/submit?url={0}",
        URLSharingServices::Pinterest => "http://pinterest.com/pin/create/button/?url={0}&media={0}",
        URLSharingServices::Tumblr => "https://www.tumblr.com/share?v=3&u={0}",
        URLSharingServices::LinkedIn => "https://www.linkedin.com/shareArticle?url={0}",
        URLSharingServices::StumbleUpon => "http://www.stumbleupon.com/submit?url={0}",
        URLSharingServices::Delicious => "https://delicious.com/save?v=5&url={0}",
        URLSharingServices::VK => "http://vk.com/share.php?url={0}",
        URLSharingServices::GoogleImageSearch => "https://lens.google.com/uploadbyurl?url={0}",
        URLSharingServices::BingVisualSearch => "https://www.bing.com/images/search?view=detailv2&iss=sbi&q=imgurl:{0}",
        _ => return None,
    })
}

/// `SimpleURLSharer`.
pub struct SimpleUrlSharer {
    pub base: Uploader,
    pub url_format_string: &'static str,
}

uploader_base!(SimpleUrlSharer, UrlSharer);
impl WritesBackConfig for SimpleUrlSharer {}

impl SimpleUrlSharer {
    pub fn new(url_format_string: &'static str) -> Self {
        Self { base: Uploader::new(), url_format_string }
    }

    /// The page it opens for `url`.
    pub fn share_page(&self, url: &str) -> String {
        self.url_format_string.replace("{0}", &url_helpers::url_encode(url, false))
    }

    fn share(&mut self, url: &str) -> CoreResult {
        let result = UploadResult { url: Some(url.to_owned()), is_url_expected: false, ..Default::default() };
        open_url(&self.share_page(url));
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_pages_encode_the_url() {
        let s = SimpleUrlSharer::new(url_format_string(&URLSharingServices::Pinterest).unwrap());
        assert_eq!(s.share_page("https://a.b/c d"), "http://pinterest.com/pin/create/button/?url=https%3A%2F%2Fa.b%2Fc%20d&media=https%3A%2F%2Fa.b%2Fc%20d");
    }
}
