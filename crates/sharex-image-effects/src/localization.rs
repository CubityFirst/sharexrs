//! `ShareX.ImageEffectsLib/Localization/ImageEffectsLocalization.cs`: the
//! names the Image effects window shows for properties, their tooltips and
//! enum values, from the library's `.resx` with ShareX's fallbacks.

fn lookup(key: &str) -> Option<&'static str> {
    sharex_strings::image_effects_lib::lookup(key)
}

/// `Helpers.GetProperName`: a space before each capital (lowered) and before
/// the first digit of a number.
pub fn proper_name(name: &str) -> String {
    let mut out = String::new();
    let mut number = false;
    for (i, c) in name.chars().enumerate() {
        if i > 0 && (c.is_uppercase() || (!number && c.is_numeric())) {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
        number = c.is_numeric();
    }
    out
}

/// `GetPropertyName`.
pub fn property_name(property: &str) -> String {
    lookup(&format!("ImageEffectProperty_{property}")).map(str::to_string).unwrap_or_else(|| proper_name(property))
}

/// `GetPropertyDescription`: the tooltip of a property that has a
/// `Description` (the ones with an `ImageEffectPropertyDescription_*` key).
pub fn property_description(effect_type: &str, property: &str) -> Option<&'static str> {
    lookup(&format!("ImageEffectPropertyDescription_{effect_type}_{property}")).filter(|s| !s.trim().is_empty())
}

/// `GetEnumValue`: `resource` is the enum's name in the keys
/// ([`crate::dotnet::PropKind::Enum`]).
pub fn enum_value(resource: &str, name: &str) -> String {
    lookup(&format!("ImageEffectEnum_{resource}_{name}")).map(str::to_string).unwrap_or_else(|| proper_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proper_names_split_words_and_numbers() {
        assert_eq!(proper_name("RandomSizeMin"), "Random size min");
        assert_eq!(proper_name("X0Y0"), "X 0 y 0");
        assert_eq!(proper_name("Rotate90FlipX"), "Rotate 90 flip x");
    }
}
