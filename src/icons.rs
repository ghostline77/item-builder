//! Preserve a source weapon's native inventory image after changing its identity.
use crate::catalogue::value;
use serde_json::Value;

pub fn generated_icon(id: &str) -> String {
    format!("GhostlineIB_{id}_Inventory")
}

/// Returns a concrete, native inventory icon name without the `UIIcon.` prefix.
/// Explicit paths take precedence. Generated names must exist in the native index.
pub fn source_icon(item: &Value) -> Option<String> {
    if let Some(path) = value(item, "iconPath").as_str().filter(|s| !s.is_empty()) {
        return Some(path.to_owned());
    }
    let resource = value(item, "appearanceResourceName")
        .as_str()
        .filter(|s| !s.is_empty() && *s != "None")?;
    let appearance = value(item, "appearanceName").as_str().unwrap_or("None");
    let tags = value(item, "visualTags")
        .as_array()
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("_")
        })
        .unwrap_or_default();
    let name = format!("weapon_{resource}_{appearance}__{tags}");
    include_str!("../data/native-weapon-icons.txt")
        .lines()
        .any(|line| line == name)
        .then_some(name)
}
