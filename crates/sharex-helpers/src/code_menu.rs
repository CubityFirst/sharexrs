//! `ShareX.HelpersLib/NameParser/CodeMenuEntry.cs` and
//! `CodeMenuEntryFilename.cs`: the name pattern variables (`%y`, `%mo`,
//! `%ra`, ...) the variable menus list, with their descriptions and
//! categories, in ShareX's order (the order of the static fields, which the
//! menus get by reflection).

use sharex_strings::helpers_lib as s;

/// `CodeMenuEntryFilename`: a variable (`Value`, without the `%`), what it
/// is, and its category (none for `%n`).
#[derive(Clone, Copy)]
pub struct CodeMenuEntryFilename {
    pub value: &'static str,
    description: fn() -> &'static str,
    category: Option<fn() -> &'static str>,
}

impl PartialEq for CodeMenuEntryFilename {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl std::fmt::Debug for CodeMenuEntryFilename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "%{}", self.value)
    }
}

impl CodeMenuEntryFilename {
    /// `Prefix`.
    pub const PREFIX: &'static str = "%";

    /// `ToPrefixString()`: `%` and the value.
    pub fn to_prefix_string(&self) -> String {
        format!("{}{}", Self::PREFIX, self.value)
    }

    pub fn description(&self) -> &'static str {
        (self.description)()
    }

    pub fn category(&self) -> Option<&'static str> {
        self.category.map(|c| c())
    }

    /// The entry for `value` (`CodeMenuEntryFilename.t`, `.i`, ...).
    pub fn get(value: &str) -> Option<Self> {
        ALL.iter().find(|e| e.value == value).copied()
    }
}

const fn e(value: &'static str, description: fn() -> &'static str, category: Option<fn() -> &'static str>) -> CodeMenuEntryFilename {
    CodeMenuEntryFilename { value, description, category }
}

const WINDOW: Option<fn() -> &'static str> = Some(s::ReplCodeMenuCategory_Window);
const DATE: Option<fn() -> &'static str> = Some(s::ReplCodeMenuCategory_Date_and_Time);
const INCREMENTAL: Option<fn() -> &'static str> = Some(s::ReplCodeMenuCategory_Incremental);
const RANDOM: Option<fn() -> &'static str> = Some(s::ReplCodeMenuCategory_Random);
const IMAGE: Option<fn() -> &'static str> = Some(s::ReplCodeMenuCategory_Image);
const COMPUTER: Option<fn() -> &'static str> = Some(s::ReplCodeMenuCategory_Computer);

/// Every entry, in the order of `CodeMenuEntryFilename`'s fields.
pub const ALL: [CodeMenuEntryFilename; 37] = [
    e("t", s::ReplCodeMenuEntry_t_Title_of_active_window, WINDOW),
    e("pn", s::ReplCodeMenuEntry_pn_Process_name_of_active_window, WINDOW),
    e("y", s::ReplCodeMenuEntry_y_Current_year, DATE),
    e("yy", s::ReplCodeMenuEntry_yy_Current_year__2_digits_, DATE),
    e("mo", s::ReplCodeMenuEntry_mo_Current_month, DATE),
    e("mon", s::ReplCodeMenuEntry_mon_Current_month_name__Local_language_, DATE),
    e("mon2", s::ReplCodeMenuEntry_mon2_Current_month_name__English_, DATE),
    e("w", s::ReplCodeMenuEntry_w_Current_week_name__Local_language_, DATE),
    e("w2", s::ReplCodeMenuEntry_w2_Current_week_name__English_, DATE),
    e("wy", s::ReplCodeMenuEntry_wy_Week_of_year, DATE),
    e("d", s::ReplCodeMenuEntry_d_Current_day, DATE),
    e("h", s::ReplCodeMenuEntry_h_Current_hour, DATE),
    e("mi", s::ReplCodeMenuEntry_mi_Current_minute, DATE),
    e("s", s::ReplCodeMenuEntry_s_Current_second, DATE),
    e("ms", s::ReplCodeMenuEntry_ms_Current_millisecond, DATE),
    e("pm", s::ReplCodeMenuEntry_pm_Gets_AM_PM, DATE),
    e("unix", s::ReplCodeMenuEntry_unix_Unix_timestamp, DATE),
    e("i", s::ReplCodeMenuEntry_i_Auto_increment_number, INCREMENTAL),
    e("ia", s::ReplCodeMenuEntry_ia_Auto_increment_alphanumeric, INCREMENTAL),
    e("iAa", s::ReplCodeMenuEntry_iAa_Auto_increment_alphanumeric_all, INCREMENTAL),
    e("ib", s::ReplCodeMenuEntry_ib_Auto_increment_base_alphanumeric, INCREMENTAL),
    e("ix", s::ReplCodeMenuEntry_ix_Auto_increment_hexadecimal, INCREMENTAL),
    e("rn", s::ReplCodeMenuEntry_rn_Random_number_0_to_9, RANDOM),
    e("ra", s::ReplCodeMenuEntry_ra_Random_alphanumeric_char, RANDOM),
    e("rna", s::RandomNonAmbiguousAlphanumericCharRepeatUsingN, RANDOM),
    e("rx", s::ReplCodeMenuEntry_rx_Random_hexadecimal, RANDOM),
    e("guid", s::ReplCodeMenuEntry_guid_Random_guid, RANDOM),
    e("radjective", s::CodeMenuEntryFilename_RandomAdjective, RANDOM),
    e("ranimal", s::CodeMenuEntryFilename_RandomAnimal, RANDOM),
    e("remoji", s::RandomEmojiRepeatUsingN, RANDOM),
    e("rf", s::ReplCodeMenuEntry_rf_Random_line_from_file, RANDOM),
    e("width", s::ReplCodeMenuEntry_width_Gets_image_width, IMAGE),
    e("height", s::ReplCodeMenuEntry_height_Gets_image_height, IMAGE),
    e("un", s::ReplCodeMenuEntry_un_User_name, COMPUTER),
    e("uln", s::ReplCodeMenuEntry_uln_User_login_name, COMPUTER),
    e("cn", s::ReplCodeMenuEntry_cn_Computer_name, COMPUTER),
    e("n", s::ReplCodeMenuEntry_n_New_line, None),
];

/// `entries.GroupBy(entry => entry.Category)`: the groups in the order
/// their first entry comes, each with its entries in order; `None` is the
/// group without a category.
pub fn group_by_category(entries: impl IntoIterator<Item = CodeMenuEntryFilename>) -> Vec<(Option<&'static str>, Vec<CodeMenuEntryFilename>)> {
    let mut groups: Vec<(Option<&'static str>, Vec<CodeMenuEntryFilename>)> = Vec::new();
    for entry in entries {
        let key = entry.category();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(entry),
            None => groups.push((key, vec![entry])),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_keep_sharex_order() {
        let groups = group_by_category(ALL);
        let names: Vec<_> = groups.iter().map(|(k, _)| k.unwrap_or("-")).collect();
        assert_eq!(names, ["Window", "Date and time", "Incremental", "Random", "Image", "Computer", "-"]);
        assert_eq!(groups[1].1.len(), 15);
        assert_eq!(ALL.iter().filter(|e| e.value == "i").count(), 1);
        assert_eq!(CodeMenuEntryFilename::get("rna").unwrap().to_prefix_string(), "%rna");
    }
}
