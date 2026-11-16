use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub catalogue: String,
    pub item: String,
    pub mode: Mode,
    pub record: String,
    pub damage_multiplier: f64,
    pub magazine_bonus: f64,
    pub reload_time_bonus: f64,
    pub rarity: Option<String>,
    #[serde(default)]
    pub iconic: Option<bool>,
    pub reload: Reload,
    pub appearance: Option<String>,
    #[serde(default)]
    pub recolours: Vec<Recolour>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Recolour {
    pub region: String,
    pub colour: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Replace,
    Variant,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Reload {
    Original,
    Reloadable,
    Disposable,
}

pub fn identifier(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 80
        && text.as_bytes()[0].is_ascii_alphabetic()
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl Project {
    pub fn new(id: String, item: String, catalogue: String) -> Self {
        Self {
            format: "ghostline-item-project".into(),
            version: 1,
            title: "My weapon mod".into(),
            record: format!("Items.Ghostline_{id}"),
            id,
            item,
            catalogue,
            mode: Mode::Variant,
            damage_multiplier: 1.,
            magazine_bonus: 0.,
            reload_time_bonus: 0.,
            rarity: None,
            iconic: None,
            reload: Reload::Original,
            appearance: None,
            recolours: Vec::new(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.recolours.len() <= 64, "too many recoloured regions");
        let mut regions = std::collections::BTreeSet::new();
        for edit in &self.recolours {
            ensure!(
                edit.colour.len() == 7
                    && edit.colour.starts_with('#')
                    && edit.colour[1..].bytes().all(|c| c.is_ascii_hexdigit()),
                "colour must be #RRGGBB"
            );
            ensure!(regions.insert(&edit.region), "duplicate recolour region");
        }
        ensure!(
            self.recolours.is_empty() || self.appearance.is_some(),
            "choose a source appearance before recolouring"
        );
        ensure!(
            self.format == "ghostline-item-project" && self.version == 1,
            "unsupported project format"
        );
        ensure!(
            identifier(&self.id),
            "project ID must start with a letter and contain only letters, digits and underscores"
        );
        ensure!(
            !self.title.trim().is_empty() && self.title.len() <= 120,
            "title must be 1–120 characters"
        );
        ensure!(
            self.record.strip_prefix("Items.").is_some_and(identifier),
            "variant record must be Items.<identifier>"
        );
        ensure!(
            !self.record.starts_with("Items.GhostlineIB_"),
            "Items.GhostlineIB_ is reserved for generated modifier records"
        );
        ensure!(
            self.damage_multiplier.is_finite() && (0.01..=100.).contains(&self.damage_multiplier),
            "damage multiplier must be 0.01–100"
        );
        ensure!(
            self.magazine_bonus.is_finite() && (-0.95..=10.).contains(&self.magazine_bonus),
            "magazine bonus must be -95%–1000%"
        );
        ensure!(
            self.reload_time_bonus.is_finite() && (-0.4..=0.4).contains(&self.reload_time_bonus),
            "reload time change must be -40%–40% (native stat limits)"
        );
        ensure!(
            self.rarity.as_deref().is_none_or(|s| [
                "Common",
                "Uncommon",
                "Rare",
                "Epic",
                "Legendary"
            ]
            .contains(&s)),
            "invalid rarity"
        );
        Ok(())
    }
}
