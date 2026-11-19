//! `ShareX/EasterEggs/ShareXClicker`: the About window's hidden clicker
//! game (`ShareXClickerGame`, `ShareXClickerState`,
//! `ShareXClickerNumberFormatter`). The window's side is in
//! `dialogs/about.rs`. Its strings are ShareX's own, in English (they
//! aren't in its `.resx`).

use std::collections::{HashMap, HashSet};

use sharex_ui::keys::app_icons as icon;

/// `StoreUnlockAt`, `EmpireGoalAt`, `BuildingMilestones`.
pub const STORE_UNLOCK_AT: f64 = 10.0;
pub const EMPIRE_GOAL_AT: f64 = 10_000_000.0;
pub const BUILDING_MILESTONES: [i32; 4] = [10, 25, 50, 100];

/// `ShareXClickerBuilding`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub base_cost: f64,
    pub production_per_second: f64,
    pub unlock_at: f64,
}

/// `ShareXClickerUpgrade`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Upgrade {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub cost: f64,
    pub unlock_at: f64,
    pub effect: &'static str,
    pub click_bonus: f64,
    pub click_multiplier: f64,
    pub production_multiplier: f64,
}

const fn b(id: &'static str, name: &'static str, icon: &'static str, base_cost: f64, production_per_second: f64, unlock_at: f64) -> Building {
    Building { id, name, icon, base_cost, production_per_second, unlock_at }
}

/// `Buildings`.
pub const BUILDINGS: [Building; 8] = [
    b("auto-clicker", "Auto Clicker", icon::PointerClick, 15.0, 1.0, 10.0),
    b("sharex-bot", "ShareX Bot", icon::Robot, 100.0, 6.0, 100.0),
    b("screenshot-station", "Screenshot Station", icon::Monitor, 600.0, 30.0, 500.0),
    b("upload-server", "Upload Server", icon::Server, 3500.0, 150.0, 2500.0),
    b("screenshot-factory", "Screenshot Factory", icon::Factory, 20000.0, 800.0, 10000.0),
    b("sharex-datacenter", "ShareX Datacenter", icon::Database, 120000.0, 4000.0, 50000.0),
    b("capture-cloud", "Capture Cloud", icon::Cloud, 700000.0, 20000.0, 250000.0),
    b("sharex-satellite", "ShareX Satellite", icon::Satellite, 4000000.0, 100000.0, 1000000.0),
];

#[allow(clippy::too_many_arguments)]
const fn u(
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    cost: f64,
    unlock_at: f64,
    effect: &'static str,
    click_bonus: f64,
    click_multiplier: f64,
    production_multiplier: f64,
) -> Upgrade {
    Upgrade { id, name, icon, cost, unlock_at, effect, click_bonus, click_multiplier, production_multiplier }
}

/// `Upgrades`.
pub const UPGRADES: [Upgrade; 6] = [
    u("better-click", "Better Click", icon::PointerClick, 50.0, 25.0, "+1 base logo per click", 1.0, 1.0, 1.0),
    u("double-click", "Double Click", icon::Mouse, 500.0, 250.0, "2x manual click value", 0.0, 2.0, 1.0),
    u("workflow-automation", "Workflow Automation", icon::Zap, 2500.0, 1000.0, "2x generator output", 0.0, 1.0, 2.0),
    u("pixel-perfect", "Pixel Perfect", icon::Target, 15000.0, 7500.0, "+4 base logos per click", 4.0, 1.0, 1.0),
    u("parallel-uploads", "Parallel Uploads", icon::CloudUpload, 75000.0, 35000.0, "3x generator output", 0.0, 1.0, 3.0),
    u("quantum-compression", "Quantum Compression", icon::Sparkles, 1000000.0, 500000.0, "2x all logo production", 0.0, 2.0, 2.0),
];

/// `ShareXClickerState` (a new one each time the window opens).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub logos: f64,
    pub lifetime_logos: f64,
    pub buildings: HashMap<&'static str, i32>,
    pub purchased_upgrades: HashSet<&'static str>,
    pub manual_clicks: i64,
    pub store_discovered: bool,
}

/// `ShareXClickerGame`.
#[derive(Clone, Debug, Default)]
pub struct Game {
    pub state: State,
}

impl Game {
    pub fn new(mut state: State) -> Self {
        // `NormalizeState`.
        let valid = |v: f64| v.is_finite();
        state.logos = if valid(state.logos) { state.logos.max(0.0) } else { 0.0 };
        state.lifetime_logos = if valid(state.lifetime_logos) { state.lifetime_logos.max(0.0) } else { 0.0 };
        state.lifetime_logos = state.lifetime_logos.max(state.logos);
        state.manual_clicks = state.manual_clicks.max(0);
        state.store_discovered |= state.lifetime_logos >= STORE_UNLOCK_AT;
        Self { state }
    }

    fn purchased(&self) -> impl Iterator<Item = &'static Upgrade> + '_ {
        UPGRADES.iter().filter(|u| self.is_purchased(u))
    }

    /// `ClickValue`.
    pub fn click_value(&self) -> f64 {
        (1.0 + self.purchased().map(|u| u.click_bonus).sum::<f64>()) * self.purchased().fold(1.0, |v, u| v * u.click_multiplier)
    }

    /// `ProductionMultiplier`.
    pub fn production_multiplier(&self) -> f64 {
        self.purchased().fold(1.0, |v, u| v * u.production_multiplier)
    }

    /// `TotalBuildings`.
    pub fn total_buildings(&self) -> i64 {
        BUILDINGS.iter().map(|b| self.owned(b) as i64).sum()
    }

    /// `LogosPerSecond`.
    pub fn logos_per_second(&self) -> f64 {
        BUILDINGS.iter().map(|b| self.building_total_production(b)).sum()
    }

    /// `Click`: the logos it earned.
    pub fn click(&mut self) -> f64 {
        let gain = self.click_value();
        self.add_logos(gain);
        self.state.manual_clicks = self.state.manual_clicks.saturating_add(1);
        self.state.store_discovered |= self.state.lifetime_logos >= STORE_UNLOCK_AT;
        gain
    }

    /// `Tick(elapsed)`: the generators' logos over `seconds`.
    pub fn tick(&mut self, seconds: f64) -> f64 {
        let rate = self.logos_per_second();
        if seconds <= 0.0 || rate <= 0.0 {
            return 0.0;
        }
        let earned = rate * seconds;
        self.add_logos(earned);
        self.state.store_discovered |= self.state.lifetime_logos >= STORE_UNLOCK_AT;
        earned
    }

    pub fn owned(&self, b: &Building) -> i32 {
        self.state.buildings.get(b.id).copied().unwrap_or(0).max(0)
    }

    pub fn building_unlocked(&self, b: &Building) -> bool {
        self.state.lifetime_logos >= b.unlock_at
    }

    /// `GetBuildingCost(building, quantity)`: 15% dearer each.
    pub fn building_cost(&self, b: &Building, quantity: i32) -> f64 {
        if quantity <= 0 {
            return 0.0;
        }
        let owned = self.owned(b);
        if quantity > i32::MAX - owned {
            return f64::MAX;
        }
        let mut total = 0.0;
        for i in 0..quantity {
            let item = (b.base_cost * 1.15f64.powi(owned + i)).ceil();
            if item.is_infinite() || total > f64::MAX - item {
                return f64::MAX;
            }
            total += item;
        }
        total
    }

    /// `GetMaxAffordableQuantity`.
    pub fn max_affordable(&self, b: &Building) -> i32 {
        if !self.building_unlocked(b) {
            return 0;
        }
        let owned = self.owned(b);
        let (mut quantity, mut total) = (0, 0.0);
        while owned + quantity < i32::MAX {
            let item = (b.base_cost * 1.15f64.powi(owned + quantity)).ceil();
            if item.is_infinite() || total > self.state.logos - item {
                break;
            }
            total += item;
            quantity += 1;
        }
        quantity
    }

    pub fn can_buy_building(&self, b: &Building, quantity: i32) -> bool {
        let owned = self.owned(b);
        quantity > 0 && quantity <= i32::MAX - owned && self.building_unlocked(b) && self.state.logos >= self.building_cost(b, quantity)
    }

    pub fn buy_building(&mut self, b: &Building, quantity: i32) -> bool {
        if !self.can_buy_building(b, quantity) {
            return false;
        }
        let cost = self.building_cost(b, quantity);
        self.state.logos -= cost;
        let owned = self.owned(b);
        self.state.buildings.insert(b.id, owned + quantity);
        true
    }

    /// `GetBuildingMilestoneMultiplier`: doubled at each milestone reached.
    pub fn milestone_multiplier(&self, b: &Building) -> i32 {
        let owned = self.owned(b);
        1 << BUILDING_MILESTONES.iter().filter(|m| owned >= **m).count()
    }

    pub fn next_milestone(&self, b: &Building) -> Option<i32> {
        let owned = self.owned(b);
        BUILDING_MILESTONES.iter().copied().find(|m| owned < *m)
    }

    /// `GetBuildingProductionPerSecond`: one of them.
    pub fn building_production(&self, b: &Building) -> f64 {
        b.production_per_second * self.milestone_multiplier(b) as f64 * self.production_multiplier()
    }

    pub fn building_total_production(&self, b: &Building) -> f64 {
        self.owned(b) as f64 * self.building_production(b)
    }

    pub fn upgrade_unlocked(&self, u: &Upgrade) -> bool {
        self.state.lifetime_logos >= u.unlock_at
    }

    pub fn is_purchased(&self, u: &Upgrade) -> bool {
        self.state.purchased_upgrades.contains(u.id)
    }

    pub fn can_buy_upgrade(&self, u: &Upgrade) -> bool {
        self.upgrade_unlocked(u) && !self.is_purchased(u) && self.state.logos >= u.cost
    }

    pub fn buy_upgrade(&mut self, u: &Upgrade) -> bool {
        if !self.can_buy_upgrade(u) {
            return false;
        }
        self.state.logos -= u.cost;
        self.state.purchased_upgrades.insert(u.id);
        true
    }

    fn add_logos(&mut self, amount: f64) {
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        self.state.logos += amount;
        self.state.lifetime_logos += amount;
    }

    /// `BuildingsToDisplay`: the unlocked ones and the next two.
    pub fn buildings_to_display(&self) -> Vec<&'static Building> {
        let mut v: Vec<&'static Building> = BUILDINGS.iter().filter(|b| self.building_unlocked(b)).collect();
        v.extend(BUILDINGS.iter().filter(|b| !self.building_unlocked(b)).take(2));
        v
    }

    /// `UpgradesToDisplay`: the unlocked ones and the next one.
    pub fn upgrades_to_display(&self) -> Vec<&'static Upgrade> {
        let mut v: Vec<&'static Upgrade> = UPGRADES.iter().filter(|u| self.upgrade_unlocked(u)).collect();
        v.extend(UPGRADES.iter().filter(|u| !self.upgrade_unlocked(u)).take(1));
        v
    }

    /// `UpdateGoal`: the goal text and the progress towards it (0 to 1).
    pub fn goal(&self) -> (String, f64) {
        let lifetime = self.state.lifetime_logos;
        if lifetime < STORE_UNLOCK_AT {
            return self.goal_progress("Store", STORE_UNLOCK_AT, 0.0);
        }
        let mut next: Vec<(&str, f64)> = BUILDINGS.iter().filter(|b| !self.building_unlocked(b)).map(|b| (b.name, b.unlock_at)).collect();
        next.extend(UPGRADES.iter().filter(|u| !self.upgrade_unlocked(u)).map(|u| (u.name, u.unlock_at)));
        // `OrderBy` is stable: the first of equal targets.
        next.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(&(name, target)) = next.first().filter(|n| n.1 > 0.0) {
            let previous = BUILDINGS
                .iter()
                .filter(|b| self.building_unlocked(b))
                .map(|b| b.unlock_at)
                .chain(UPGRADES.iter().filter(|u| self.upgrade_unlocked(u)).map(|u| u.unlock_at))
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
                .unwrap_or(STORE_UNLOCK_AT);
            return self.goal_progress(name, target, previous);
        }
        if lifetime < EMPIRE_GOAL_AT {
            return self.goal_progress("Complete the empire", EMPIRE_GOAL_AT, 1_000_000.0);
        }
        ("Screenshot empire complete!".into(), 1.0)
    }

    fn goal_progress(&self, name: &str, target: f64, previous: f64) -> (String, f64) {
        let remaining = (target - self.state.lifetime_logos).max(0.0);
        let progress = ((self.state.lifetime_logos - previous) / (target - previous).max(1.0)).clamp(0.0, 1.0);
        (format!("Next: {name} • {} to go", format_number(remaining)), progress)
    }
}

/// `ShareXClickerNumberFormatter.Format`: whole numbers under 1000, then
/// K, M, B, T, Qa... with one decimal under 100.
pub fn format_number(value: f64) -> String {
    const SUFFIXES: [&str; 12] = ["", "K", "M", "B", "T", "Qa", "Qi", "Sx", "Sp", "Oc", "No", "Dc"];
    if !value.is_finite() {
        return "0".into();
    }
    let mut suffix = 0;
    let mut scaled = value.max(0.0);
    while scaled >= 1000.0 && suffix < SUFFIXES.len() - 1 {
        scaled /= 1000.0;
        suffix += 1;
    }
    if suffix == 0 {
        return format!("{}", scaled.floor());
    }
    // .NET's "0" / "0.0": away from zero at the half.
    let text = if scaled >= 100.0 { format!("{}", round_half_away(scaled, 0)) } else { format!("{:.1}", round_half_away(scaled, 1)) };
    format!("{text}{}", SUFFIXES[suffix])
}

fn round_half_away(v: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (v * m).round() / m
}

/// `{0:N0}` with the invariant group separator, for the stats tooltip.
pub fn group(v: i64) -> String {
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_as_sharex_does() {
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(999.9), "999");
        assert_eq!(format_number(1500.0), "1.5K");
        assert_eq!(format_number(123_456.0), "123K");
        assert_eq!(format_number(10_000_000.0), "10.0M");
        assert_eq!(format_number(f64::NAN), "0");
        assert_eq!(group(1_234_567), "1,234,567");
    }

    #[test]
    fn clicks_open_the_store_at_ten() {
        let mut g = Game::new(State::default());
        for _ in 0..9 {
            assert_eq!(g.click(), 1.0);
        }
        assert!(!g.state.store_discovered);
        g.click();
        assert!(g.state.store_discovered);
        assert_eq!(g.state.manual_clicks, 10);
        assert_eq!(g.goal().0, "Next: Better Click • 15 to go");
    }

    #[test]
    fn buildings_cost_more_each_and_produce() {
        let mut g = Game::new(State { logos: 1000.0, lifetime_logos: 1000.0, ..Default::default() });
        let auto = &BUILDINGS[0];
        // 15, then ceil(15 * 1.15) = 18.
        assert_eq!(g.building_cost(auto, 1), 15.0);
        assert_eq!(g.building_cost(auto, 2), 33.0);
        assert!(g.buy_building(auto, 2));
        assert_eq!(g.state.logos, 967.0);
        assert_eq!(g.logos_per_second(), 2.0);
        assert_eq!(g.tick(0.5), 1.0);
        // Ten of them double their output.
        g.state.buildings.insert(auto.id, 10);
        assert_eq!(g.milestone_multiplier(auto), 2);
        assert_eq!(g.next_milestone(auto), Some(25));
        assert_eq!(g.logos_per_second(), 20.0);
    }

    #[test]
    fn upgrades_change_clicks_and_production() {
        let mut g = Game::new(State { logos: 10_000.0, lifetime_logos: 10_000.0, ..Default::default() });
        assert!(g.buy_upgrade(&UPGRADES[0]));
        assert!(g.buy_upgrade(&UPGRADES[1]));
        // (1 + 1) * 2.
        assert_eq!(g.click_value(), 4.0);
        assert!(g.buy_upgrade(&UPGRADES[2]));
        assert_eq!(g.production_multiplier(), 2.0);
        assert!(!g.buy_upgrade(&UPGRADES[2]));
        assert!(!g.can_buy_upgrade(&UPGRADES[5]));
    }

    #[test]
    fn the_store_shows_whats_unlocked_and_whats_next() {
        let g = Game::new(State { lifetime_logos: 150.0, logos: 150.0, ..Default::default() });
        let ids: Vec<&str> = g.buildings_to_display().iter().map(|b| b.id).collect();
        assert_eq!(ids, ["auto-clicker", "sharex-bot", "screenshot-station", "upload-server"]);
        let ups: Vec<&str> = g.upgrades_to_display().iter().map(|u| u.id).collect();
        assert_eq!(ups, ["better-click", "double-click"]);
        assert_eq!(g.max_affordable(&BUILDINGS[0]), 6);
    }

    #[test]
    fn the_empire_completes_at_ten_million() {
        let g = Game::new(State { lifetime_logos: 20_000_000.0, ..Default::default() });
        assert_eq!(g.goal(), ("Screenshot empire complete!".into(), 1.0));
        let g = Game::new(State { lifetime_logos: 2_000_000.0, ..Default::default() });
        assert_eq!(g.goal().0, "Next: Complete the empire • 8.0M to go");
    }
}
