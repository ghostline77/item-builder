use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

pub struct Catalogue {
    pub root: PathBuf,
    pub identity: String,
    pub sources: Value,
    pub items: BTreeMap<String, Value>,
    pub native_names: BTreeSet<String>,
    selection_shards: Vec<String>,
    selection_cache: std::sync::Mutex<BTreeMap<usize, std::sync::Arc<Vec<Value>>>>,
}

pub fn relative(root: &Path, path: &str) -> Result<PathBuf> {
    ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "invalid catalogue path"
    );
    let resolved = root.join(path).canonicalize()?;
    ensure!(resolved.starts_with(root), "catalogue path escapes package");
    Ok(resolved)
}

pub fn value<'a>(item: &'a Value, key: &str) -> &'a Value {
    &item["properties"][key]["value"]
}

impl Catalogue {
    pub fn load(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        let descriptor: Value =
            serde_json::from_slice(&fs::read(root.join("data/catalogue.json"))?)?;
        ensure!(
            descriptor["format"] == "ghostline-items" && descriptor["version"] == 1,
            "unsupported items catalogue"
        );
        let mut items = BTreeMap::new();
        let mut native_names = BTreeSet::new();
        for shard in descriptor["collections"]["items"]
            .as_array()
            .context("items collection missing")?
        {
            let rows: Vec<Value> = serde_json::from_slice(&fs::read(relative(
                &root,
                shard.as_str().context("invalid shard")?,
            )?)?)?;
            for item in rows {
                if let Some(name) = item["native_name"].as_str() {
                    native_names.insert(name.to_owned());
                }
                if item["native_name"].is_string()
                    && item["item_type"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("Wea_"))
                {
                    let id = item["id"].as_str().context("item ID missing")?.to_owned();
                    ensure!(items.insert(id, item).is_none(), "duplicate item identity");
                }
            }
        }
        let selection_shards: Vec<String> =
            serde_json::from_value(descriptor["collections"]["selections"].clone())?;
        let mut hash = Sha256::new();
        hash.update(serde_json::to_vec(&descriptor["sources"])?);
        for item in items.values() {
            hash.update(serde_json::to_vec(item)?);
        }
        Ok(Self {
            root,
            identity: format!("{:x}", hash.finalize()),
            sources: descriptor["sources"].clone(),
            items,
            native_names,
            selection_shards,
            selection_cache: Default::default(),
        })
    }

    pub fn item(&self, id: &str) -> Result<&Value> {
        self.items
            .get(id)
            .context("weapon is absent from the loaded catalogue")
    }

    pub fn selections(&self, item: &Value) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let ids: BTreeSet<_> = item["selections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for id in ids {
            let (mut low, mut high) = (0, self.selection_shards.len());
            while low < high {
                let middle = low + (high - low) / 2;
                let cached = self
                    .selection_cache
                    .lock()
                    .map_err(|_| anyhow::anyhow!("selection cache poisoned"))?
                    .get(&middle)
                    .cloned();
                let rows = if let Some(rows) = cached {
                    rows
                } else {
                    let rows: Vec<Value> = serde_json::from_slice(&fs::read(relative(
                        &self.root,
                        &self.selection_shards[middle],
                    )?)?)?;
                    ensure!(
                        !rows.is_empty()
                            && rows
                                .windows(2)
                                .all(|v| v[0]["id"].as_str() < v[1]["id"].as_str()),
                        "selection shards must have sorted native identities"
                    );
                    let rows = std::sync::Arc::new(rows);
                    let mut cache = self
                        .selection_cache
                        .lock()
                        .map_err(|_| anyhow::anyhow!("selection cache poisoned"))?;
                    if cache.len() >= 64 {
                        cache.pop_first();
                    }
                    cache.insert(middle, rows.clone());
                    rows
                };
                if id < rows[0]["id"].as_str().context("selection ID missing")? {
                    high = middle;
                } else if id
                    > rows.last().context("empty selection shard")?["id"]
                        .as_str()
                        .context("selection ID missing")?
                {
                    low = middle + 1;
                } else {
                    if let Some(row) = rows.iter().find(|r| r["id"] == id) {
                        out.push(row.clone());
                    }
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn search(&self, query: &str, kind: &str, offset: usize) -> Value {
        let words: Vec<_> = query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let found: Vec<_> = self
            .items
            .values()
            .filter(|item| {
                let text = format!(
                    "{} {} {}",
                    item["label"], item["native_name"], item["native_id"]
                )
                .to_lowercase();
                (kind.is_empty() || item["item_type"] == kind)
                    && words.iter().all(|w| text.contains(w))
            })
            .collect();
        let mut sorted = found;
        sorted.sort_by_key(|item| {
            (
                item["selections"].as_array().is_none_or(Vec::is_empty),
                item["label"].as_str().unwrap_or(""),
            )
        });
        json!({"total":sorted.len(),"offset":offset,"items":sorted.into_iter().skip(offset).take(60).map(|i|json!({"id":i["id"],"record":i["native_name"],"label":i["label"],"kind":i["item_type"],"looks":i["selections"].as_array().map_or(0,Vec::len)})).collect::<Vec<_>>()})
    }
}
