//! `ShareX.UploadersLib/UploaderFilter.cs` and the generic uploader
//! services it picks from (`UploaderFactory.AllGenericUploaderServices`).

use serde_json::Value;
use sharex_helpers::file_helpers::get_file_name_extension;
use sharex_uploaders::factory::{self, Ported, TaskReferenceHelper};
use sharex_uploaders::{FileDestination, GenericUploader, ImageDestination, TextDestination, UploadersConfig};

/// An image, text or file uploader service (`IGenericUploaderService`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Service {
    Image(ImageDestination),
    Text(TextDestination),
    File(FileDestination),
}

impl Service {
    /// `ServiceName`.
    pub fn name(&self) -> String {
        match self {
            Service::Image(d) => factory::service_name("ImageDestination", d.name()),
            Service::Text(d) => factory::service_name("TextDestination", d.name()),
            Service::File(d) => factory::service_name("FileDestination", d.name()),
        }
    }

    /// `CheckConfig`.
    pub fn check(&self, config: &UploadersConfig) -> Ported {
        match self {
            Service::Image(d) => factory::check_image(d, config),
            Service::Text(d) => factory::check_text(d, config),
            Service::File(d) => factory::check_file(d, config),
        }
    }

    /// `CreateUploader`.
    pub fn create(&self, config: &mut UploadersConfig, task: &TaskReferenceHelper) -> Option<GenericUploader> {
        match self {
            Service::Image(d) => factory::create_image(d, config, task),
            Service::Text(d) => factory::create_text(d, config, task),
            Service::File(d) => factory::create_file(d, config, task),
        }
    }

    /// `UploaderFactory.AllGenericUploaderServices`, in ShareX's order
    /// (image, text, then file services); `*.FileUploader` isn't a service.
    pub fn all() -> impl Iterator<Item = Service> {
        let image = ImageDestination::ALL.iter().filter(|d| **d != ImageDestination::FileUploader).cloned().map(Service::Image);
        let text = TextDestination::ALL.iter().filter(|d| **d != TextDestination::FileUploader).cloned().map(Service::Text);
        let file = FileDestination::ALL.iter().cloned().map(Service::File);
        image.chain(text).chain(file)
    }

    /// `ServiceIdentifier` (`EnumValue.ToString()`).
    pub fn identifier(&self) -> &str {
        match self {
            Service::Image(d) => d.name(),
            Service::Text(d) => d.name(),
            Service::File(d) => d.name(),
        }
    }
}

/// `UploaderFilter`, as stored in `TaskSettingsUpload.UploaderFilters`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UploaderFilter {
    pub uploader: String,
    pub extensions: Vec<String>,
}

impl UploaderFilter {
    pub fn from_json(v: &Value) -> Self {
        Self {
            uploader: v.get("Uploader").and_then(Value::as_str).unwrap_or_default().to_owned(),
            extensions: v
                .get("Extensions")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default(),
        }
    }

    /// `IsValidFilter`: the file's extension is one of the filter's (a
    /// leading dot ignored, any case).
    pub fn is_valid_filter(&self, file_name: &str) -> bool {
        let extension = get_file_name_extension(file_name);
        !extension.is_empty() && self.extensions.iter().any(|x| x.trim_start_matches('.').eq_ignore_ascii_case(&extension))
    }

    /// `GetUploaderService`: the service whose identifier is the filter's
    /// uploader, ignoring case.
    pub fn uploader_service(&self) -> Option<Service> {
        Service::all().find(|s| s.identifier().eq_ignore_ascii_case(&self.uploader))
    }
}

/// `CheckUploadFilters`' search: the service of the first filter that
/// matches `file_name`.
pub fn find_filter_service(filters: &[Value], file_name: &str) -> Option<Option<Service>> {
    if file_name.is_empty() {
        return None;
    }
    filters.iter().map(UploaderFilter::from_json).find(|f| f.is_valid_filter(file_name)).map(|f| f.uploader_service())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_match_extensions_and_find_services() {
        let f = UploaderFilter::from_json(&serde_json::json!({ "Uploader": "customfileuploader", "Extensions": [".PNG", "gif"] }));
        assert!(f.is_valid_filter("a.png") && f.is_valid_filter("b.GIF") && !f.is_valid_filter("c.jpg") && !f.is_valid_filter("png"));
        assert_eq!(f.uploader_service(), Some(Service::File(FileDestination::CustomFileUploader)));
        let filters = [serde_json::json!({ "Uploader": "Nope", "Extensions": ["txt"] }), serde_json::json!({ "Uploader": "Imgur", "Extensions": ["txt"] })];
        // The first matching filter wins, even when its uploader doesn't exist.
        assert_eq!(find_filter_service(&filters, "a.txt"), Some(None));
        assert_eq!(find_filter_service(&filters, "a.md"), None);
    }

    #[test]
    fn services_are_named_as_sharex_names_them() {
        assert_eq!(Service::Image(ImageDestination::Vgyme).name(), "vgy.me");
        assert_eq!(Service::all().filter(|s| s.identifier() == "FileUploader").count(), 0);
    }
}
