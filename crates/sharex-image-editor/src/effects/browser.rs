//! The effect browser's model (`Presentation/Controls/EffectBrowserPanel`
//! and `Presentation/Effects/EffectSearch`, ShareX 22.4), without its
//! drawing: the Recent and Favorites categories and the four effect
//! categories with their items (each catalogue effect and editor operation
//! under its browser label, sorted as `EffectCategory.Insert` sorts), the
//! search (`EffectSearch.Score`, matches ordered by score, then the
//! label's length, then the label; categories by their best match while
//! searching), the recent effects (ten, newest first) and the favourites
//! (toggled with a right click), kept in `ImageEditorOptions.RecentEffects`
//! and `FavoriteEffects`.

use std::cmp::Ordering;

use super::{EffectCategory, EffectDefinition, catalog};

/// `EffectBrowserPanel.MaxRecentEffects`.
pub const MAX_RECENT_EFFECTS: usize = 10;

/// `EffectBrowserPanel.EffectAliases`: older persisted ids.
const ALIASES: [(&str, &str); 8] = [
    ("resize", "resize_image"),
    ("canvas", "resize_canvas"),
    ("crop", "crop_image"),
    ("auto_crop", "auto_crop_image"),
    ("editor_auto_crop", "auto_crop_image"),
    ("rotate", "rotate_custom_angle"),
    ("rotate_90", "rotate_90_clockwise"),
    ("rotate_90_cc", "rotate_90_counter_clockwise"),
];

/// `EffectSearch.Keywords`: plain-language words that aren't in the name.
const KEYWORDS: [(&str, &str); 25] = [
    ("black_and_white", "bw b&w monochrome mono"),
    ("grayscale", "grey gray greyscale desaturate monochrome"),
    ("blur", "blurry soften smooth"),
    ("gaussian_blur", "blurry soften"),
    ("rotate_90_clockwise", "turn right"),
    ("rotate_90_counter_clockwise", "turn left"),
    ("rotate_180", "turn upside down"),
    ("rotate_custom_angle", "turn angle tilt straighten"),
    ("resize_image", "smaller bigger shrink enlarge scale size dimensions"),
    ("resize_canvas", "padding expand canvas size"),
    ("brightness", "brighter darker lighten darken light"),
    ("sharpen", "sharper crisp clarity detail"),
    ("pixelate", "censor hide mosaic blocks"),
    ("shadow", "drop shadow"),
    ("invert", "negative inverse colors"),
    ("flip_horizontal", "mirror"),
    ("flip_vertical", "mirror upside down"),
    ("rounded_corners", "round corners radius"),
    ("remove_background", "transparent background remove bg cutout"),
    ("text_watermark", "watermark copyright"),
    ("border", "frame outline edge"),
    ("add_noise", "grain"),
    ("sepia", "old vintage brown"),
    ("crop_image", "trim cut"),
    ("vignette", "dark edges"),
];

/// `EffectSearch.FillerWords`.
const FILLER_WORDS: [&str; 13] = ["make", "the", "a", "an", "my", "image", "picture", "photo", "screenshot", "it", "to", "more", "add"];

/// `EffectItem.NormalizeEffectId`: letters and digits lower-cased, runs of
/// anything else one `_`, none at either end.
pub fn normalize_effect_id(value: &str) -> String {
    let mut s = String::with_capacity(value.len());
    let mut last_underscore = false;
    for c in value.chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            s.push('_');
            last_underscore = true;
        }
    }
    s.trim_matches('_').to_string()
}

/// `StringComparison.OrdinalIgnoreCase` (`ToUpperInvariant` per UTF-16 unit).
pub fn compare_ignore_case(a: &str, b: &str) -> Ordering {
    let up = |s: &str| -> Vec<u16> {
        s.chars()
            .map(|c| {
                let mut u = c.to_uppercase();
                match (u.next(), u.next()) {
                    (Some(x), None) => x,
                    _ => c,
                }
            })
            .collect::<String>()
            .encode_utf16()
            .collect()
    };
    up(a).cmp(&up(b))
}

/// `EffectSearch.Normalize`: lower case, `&` as " and ", anything but
/// letters and digits a space, words joined by single spaces; filler words
/// dropped when asked (unless that leaves none).
fn normalize(text: &str, remove_filler_words: bool) -> String {
    let mut s = String::with_capacity(text.len() + 8);
    for c in text.to_lowercase().chars() {
        if c == '&' {
            s.push_str(" and ");
        } else if c.is_alphanumeric() {
            s.push(c);
        } else {
            s.push(' ');
        }
    }
    let mut words: Vec<&str> = s.split(' ').filter(|w| !w.is_empty()).collect();
    if remove_filler_words {
        let kept: Vec<&str> = words.iter().copied().filter(|w| !FILLER_WORDS.iter().any(|f| f.eq_ignore_ascii_case(w))).collect();
        if !kept.is_empty() {
            words = kept;
        }
    }
    words.join(" ")
}

/// `EffectSearch.IsOneEditApart`.
fn is_one_edit_apart(a: &[char], b: &[char]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (a, b) = if a.len() > b.len() { (b, a) } else { (a, b) };
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] != b[j] {
            edits += 1;
            if edits > 1 {
                return false;
            }
            if a.len() == b.len() {
                i += 1;
            }
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }
    edits + (b.len() - j) <= 1
}

/// `EffectSearch.MatchesAnyWord`: a word starts with the token, or (tokens
/// of five or more) its start is one edit away.
fn matches_any_word(token: &str, words: &[&str]) -> bool {
    let t: Vec<char> = token.chars().collect();
    for word in words {
        if word.starts_with(token) {
            return true;
        }
        if t.len() >= 5 {
            let w: Vec<char> = word.chars().collect();
            let w = if w.len() > t.len() { &w[..t.len() + 1] } else { &w[..] };
            if is_one_edit_apart(&t, w) {
                return true;
            }
        }
    }
    false
}

/// `EffectSearch.Score`: 0 when the effect doesn't match, otherwise higher
/// for a better match (the name, then the keywords, then the description).
pub fn score(query: &str, name: &str, description: &str, effect_id: Option<&str>) -> i32 {
    let q = normalize(query, true);
    if q.is_empty() {
        return 0;
    }
    let label = normalize(name, false);
    let keywords = effect_id
        .and_then(|id| KEYWORDS.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)))
        .map(|(_, w)| normalize(w, false))
        .unwrap_or_default();
    let tokens: Vec<&str> = q.split(' ').collect();
    let label_words: Vec<&str> = label.split(' ').collect();
    let keyword_words: Vec<&str> = if keywords.is_empty() { Vec::new() } else { keywords.split(' ').collect() };

    if q == label {
        return 1000;
    }
    if label.starts_with(&q) {
        return 900;
    }
    if tokens.iter().all(|t| matches_any_word(t, &label_words)) {
        return 800;
    }
    let compact = q.replace(' ', "");
    if label.replace(' ', "").contains(&compact) || label.replace(" and ", " ").replace(' ', "").contains(&compact) {
        return 700;
    }
    if keywords.contains(&q) || tokens.iter().all(|t| matches_any_word(t, &keyword_words)) {
        return 600;
    }
    if tokens.iter().all(|t| matches_any_word(t, &label_words) || matches_any_word(t, &keyword_words)) {
        return 500;
    }
    let desc = normalize(description, false);
    let desc_words: Vec<&str> = desc.split(' ').collect();
    if desc.contains(&q) || tokens.iter().all(|t| matches_any_word(t, &desc_words)) {
        return 300;
    }
    0
}

/// `EffectItem`: an effect's button in the browser.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: &'static str,
    /// `Name`: the browser label (`Name...` when it opens a dialog),
    /// localised by the host.
    pub name: String,
    pub icon: &'static str,
    pub description: &'static str,
}

/// Which section of the browser an item is shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Recent,
    Favorites,
    Category(EffectCategory),
}

/// A section as the browser shows it (`EffectCategory` with its
/// `VisibleEffects`).
#[derive(Clone, Debug)]
pub struct VisibleSection {
    pub section: Section,
    /// The section's whole size (`AllEffects.Count`, the header's count).
    pub count: usize,
    pub items: Vec<Item>,
}

/// The browser's state: every item by category, the recent and favourite
/// ids, and the search.
#[derive(Clone, Debug)]
pub struct Browser {
    categories: Vec<(EffectCategory, Vec<Item>)>,
    recent: Vec<&'static str>,
    favorites: Vec<&'static str>,
    pub search: String,
}

impl Browser {
    /// `InitializeEffects`: each category's effects (catalogue effects and
    /// editor operations, all under their browser label) in label order;
    /// `label` localises a definition's browser label.
    pub fn new(label: impl Fn(&EffectDefinition) -> String) -> Self {
        let mut categories = Vec::new();
        for cat in EffectCategory::ALL {
            let mut items: Vec<Item> = Vec::new();
            // Catalogue effects first, then the editor operations, each
            // inserted after the items it doesn't sort before.
            let defs = catalog().iter().filter(|d| d.category == cat);
            let (ops, others): (Vec<_>, Vec<_>) = defs.partition(|d| d.editor_operation().is_some());
            for d in others.into_iter().chain(ops) {
                let item = Item { id: d.id, name: label(d), icon: d.icon, description: d.description };
                let at = items.iter().position(|i| compare_ignore_case(&i.name, &item.name) == Ordering::Greater).unwrap_or(items.len());
                items.insert(at, item);
            }
            categories.push((cat, items));
        }
        Browser { categories, recent: Vec::new(), favorites: Vec::new(), search: String::new() }
    }

    fn item(&self, id: &str) -> Option<&Item> {
        self.categories.iter().flat_map(|(_, items)| items.iter()).find(|i| i.id.eq_ignore_ascii_case(id))
    }

    /// `TryResolveEffect`: a stored id (normalised, aliases followed).
    pub fn resolve(&self, value: &str) -> Option<&'static str> {
        let mut id = normalize_effect_id(value);
        if id.is_empty() {
            return None;
        }
        if let Some((_, a)) = ALIASES.iter().find(|(k, _)| k.eq_ignore_ascii_case(&id)) {
            id = (*a).to_string();
        }
        self.item(&id).map(|i| i.id)
    }

    /// `SetOptions`: the recent and favourite effects from the options;
    /// returns them as they're stored again (resolved, without unknown
    /// ids or repeats).
    pub fn load(&mut self, recent: &[String], favorites: &[String]) -> (Vec<String>, Vec<String>) {
        self.recent.clear();
        for r in recent {
            let Some(id) = self.resolve(r) else { continue };
            if self.recent.iter().any(|x| x.eq_ignore_ascii_case(id)) {
                continue;
            }
            self.recent.push(id);
            if self.recent.len() >= MAX_RECENT_EFFECTS {
                break;
            }
        }
        self.favorites.clear();
        for f in favorites {
            if let Some(id) = self.resolve(f)
                && !self.favorites.contains(&id)
            {
                self.favorites.push(id);
            }
        }
        (self.recent_ids(), self.favorite_ids())
    }

    pub fn recent_ids(&self) -> Vec<String> {
        self.recent.iter().map(|s| s.to_string()).collect()
    }

    pub fn favorite_ids(&self) -> Vec<String> {
        self.favorites.iter().map(|s| s.to_string()).collect()
    }

    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|f| f.eq_ignore_ascii_case(id))
    }

    /// `RegisterRecentEffect`: an effect run from the browser (or the
    /// favourites menu) goes first in Recent.
    pub fn register_recent(&mut self, id: &str) {
        let Some(id) = self.resolve(id) else { return };
        self.recent.retain(|r| !r.eq_ignore_ascii_case(id));
        self.recent.insert(0, id);
        self.recent.truncate(MAX_RECENT_EFFECTS);
    }

    /// A right click on an item (`OnEffectItemPointerPressed`): one in
    /// Recent leaves Recent, any other toggles its favourite.
    pub fn right_click(&mut self, section: Section, id: &str) {
        if section == Section::Recent {
            self.recent.retain(|r| !r.eq_ignore_ascii_case(id));
        } else if let Some(id) = self.resolve(id) {
            if let Some(i) = self.favorites.iter().position(|f| *f == id) {
                self.favorites.remove(i);
            } else {
                self.favorites.push(id);
            }
        }
    }

    /// `UpdateSearchWatermark`'s count: every effect outside the pinned
    /// sections.
    pub fn total(&self) -> usize {
        self.categories.iter().map(|(_, i)| i.len()).sum()
    }

    /// The favourites menu's items (`CreateFavoriteMenuItems`): by name.
    pub fn favorite_menu(&self) -> Vec<Item> {
        let mut items: Vec<Item> = self.favorites.iter().filter_map(|id| self.item(id).cloned()).collect();
        items.sort_by(|a, b| compare_ignore_case(&a.name, &b.name));
        items
    }

    fn pinned(&self, ids: &[&'static str]) -> Vec<Item> {
        ids.iter().filter_map(|id| self.item(id).cloned()).collect()
    }

    /// The sections as shown for the search (`ApplyCurrentFilter` and
    /// `OrderCategoriesByBestMatch`): empty ones hidden; while searching,
    /// each section's matches by score and the categories by their best
    /// match, Recent and Favorites first.
    pub fn visible(&self) -> Vec<VisibleSection> {
        let searching = !self.search.trim().is_empty();
        let mut sections: Vec<(Section, Vec<Item>)> = vec![(Section::Recent, self.pinned(&self.recent)), (Section::Favorites, self.pinned(&self.favorites))];
        sections.extend(self.categories.iter().map(|(c, items)| (Section::Category(*c), items.clone())));
        let mut out: Vec<(VisibleSection, i32)> = Vec::new();
        for (section, all) in sections {
            let count = all.len();
            let (items, best) = if searching {
                let mut m: Vec<(Item, i32)> = all
                    .into_iter()
                    .map(|i| {
                        let s = score(&self.search, &i.name, i.description, Some(i.id));
                        (i, s)
                    })
                    .filter(|(_, s)| *s > 0)
                    .collect();
                m.sort_by(|(a, sa), (b, sb)| {
                    sb.cmp(sa)
                        .then_with(|| a.name.encode_utf16().count().cmp(&b.name.encode_utf16().count()))
                        .then_with(|| compare_ignore_case(&a.name, &b.name))
                });
                let best = m.first().map_or(0, |(_, s)| *s);
                (m.into_iter().map(|(i, _)| i).collect::<Vec<_>>(), best)
            } else {
                (all, 0)
            };
            if !items.is_empty() {
                out.push((VisibleSection { section, count, items }, best));
            }
        }
        if searching {
            // Stable: equal best scores keep the default order.
            let (pinned, mut rest): (Vec<_>, Vec<_>) = out.into_iter().partition(|(s, _)| !matches!(s.section, Section::Category(_)));
            rest.sort_by_key(|s| std::cmp::Reverse(s.1));
            out = pinned.into_iter().chain(rest).collect();
        }
        out.into_iter().map(|(s, _)| s).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser() -> Browser {
        Browser::new(|d| d.browser_label.clone())
    }

    #[test]
    fn normalizes_ids() {
        assert_eq!(normalize_effect_id("  Resize Image... "), "resize_image");
        assert_eq!(normalize_effect_id("Rotate 90° CC"), "rotate_90_cc");
        assert_eq!(normalize_effect_id("___"), "");
    }

    #[test]
    fn categories_sort_by_label() {
        let b = browser();
        assert_eq!(b.total(), 230);
        let v = b.visible();
        let manip = v.iter().find(|s| s.section == Section::Category(EffectCategory::Manipulations)).unwrap();
        assert_eq!(manip.count, 33);
        let names: Vec<&str> = manip.items.iter().map(|i| i.name.as_str()).take(8).collect();
        // As ShareX's browser shows them: "Flip..." after the two flips.
        assert_eq!(names, ["Auto crop image...", "Crop image...", "Cylinder wrap...", "Displacement map...", "Fisheye lens...", "Flip horizontal", "Flip vertical", "Flip..."]);
    }

    #[test]
    fn search_orders_matches() {
        let mut b = browser();
        b.search = "blur".into();
        let v = b.visible();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section, Section::Category(EffectCategory::Filters));
        assert_eq!(v[0].count, 149);
        let names: Vec<&str> = v[0].items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["Blur...", "Spin blur...", "Zoom blur...", "Motion blur...", "Surface blur...", "Gaussian blur...", "Lens blur (bokeh)...", "Miniature...", "Dream glow...", "Double exposure..."]);
        b.search = "straighten".into();
        assert_eq!(b.visible()[0].items[0].id, "rotate_custom_angle");
        b.search = "background ima".into();
        assert_eq!(b.visible()[0].items[0].id, "draw_background_image");
        b.search = "border".into();
        assert_eq!(b.visible()[0].items[0].id, "border");
        b.search = "zzzz".into();
        assert!(b.visible().is_empty());
        // One typo in a longer word.
        b.search = "gausian".into();
        assert_eq!(b.visible()[0].items[0].id, "gaussian_blur");
    }

    #[test]
    fn recent_and_favorites() {
        let mut b = browser();
        let favs: Vec<String> = crate::options::DEFAULT_FAVORITE_EFFECTS.iter().map(|s| s.to_string()).collect();
        let (recent, f) = b.load(&["Resize".into(), "nope".into(), "resize_image".into(), "invert".into()], &favs);
        assert_eq!(recent, ["resize_image", "invert"]);
        assert_eq!(f, favs);
        let v = b.visible();
        assert_eq!(v[0].section, Section::Recent);
        assert_eq!(v[1].section, Section::Favorites);
        assert_eq!(v[1].count, 9);
        b.register_recent("gaussian_blur");
        assert_eq!(b.recent_ids()[0], "gaussian_blur");
        for i in 0..12 {
            b.register_recent(catalog()[i].id);
        }
        assert_eq!(b.recent_ids().len(), MAX_RECENT_EFFECTS);
        // A right click in Recent removes it there; elsewhere it toggles
        // the favourite.
        let first = b.recent_ids()[0].clone();
        b.right_click(Section::Recent, &first);
        assert!(!b.recent_ids().contains(&first));
        b.right_click(Section::Category(EffectCategory::Filters), "gaussian_blur");
        assert!(b.is_favorite("gaussian_blur"));
        b.right_click(Section::Favorites, "gaussian_blur");
        assert!(!b.is_favorite("gaussian_blur"));
        let menu: Vec<String> = b.favorite_menu().iter().map(|i| i.name.clone()).collect();
        assert_eq!(menu[0], "Auto crop image...");
    }
}
