//! Native masked-layer colour editing and self-contained appearance packages.
use crate::{catalogue::Catalogue, project::Project};
use anyhow::{Context, Result, ensure};
use ghostline_red::{assets::ResourceKey, material, writer};
use ghostline_renderer::{
    AssemblyOptions, Camera, GameSource, PreparedScene, RenderOptions, Renderer, SceneAssembly,
    View,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize)]
pub struct Region {
    pub id: String,
    pub label: String,
    pub colour: String,
    pub layer: usize,
    pub setup: String,
    pub template: String,
    pub choice: String,
}
struct Mesh {
    document: Value,
    setups: BTreeMap<String, String>,
}
struct Plan {
    app: Value,
    appearance: String,
    meshes: BTreeMap<u64, Mesh>,
    setups: BTreeMap<String, Value>,
    templates: BTreeMap<String, Value>,
    regions: Vec<Region>,
}
pub struct Native {
    source: GameSource,
    game: PathBuf,
    schema: Option<PathBuf>,
    plans: Mutex<BTreeMap<String, Plan>>,
    gpu: Mutex<Option<Renderer>>,
    artifacts: Mutex<BTreeMap<String, Arc<Assets>>>,
    highlight: Mutex<Option<HighlightScene>>,
    browser: Mutex<BTreeMap<String, Arc<BrowserAsset>>>,
}
struct BrowserAsset {
    scene: Arc<Vec<u8>>,
    layers: Value,
}
struct HighlightScene {
    key: String,
    assembly: SceneAssembly,
    prepared: PreparedScene,
    work: tempfile::TempDir,
}
pub struct Assets {
    pub archive: Vec<u8>,
    pub xl: String,
    pub previews: BTreeMap<String, Vec<u8>>,
    pub inventory_icon: Vec<u8>,
}
fn text(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| value["$value"].as_str())
}
fn depot(value: &Value) -> Option<&str> {
    text(&value["DepotPath"])
}
fn resource_path(path: &str) -> Value {
    json!({"$type":"ResourcePath","$storage":"string","$value":path})
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..20].to_owned()
}
pub fn factory(p: &Project) -> String {
    format!("ghostline_ib_{}", p.id)
}
fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(c: f64) -> u8 {
    let c = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1. / 2.4) - 0.055
    };
    (c.clamp(0., 1.) * 255.).round() as u8
}
fn colour_entry<'a>(template: &'a Value, choice: &str) -> Option<&'a Value> {
    template
        .pointer("/Data/RootChunk/overrides/colorScale")?
        .as_array()?
        .iter()
        .find(|entry| text(&entry["n"]) == Some(choice))
}
impl Native {
    pub fn open(game: PathBuf, schema: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            source: GameSource::open(&game, schema.as_deref())?,
            game,
            schema,
            plans: Mutex::new(BTreeMap::new()),
            gpu: Mutex::new(None),
            artifacts: Mutex::new(BTreeMap::new()),
            highlight: Mutex::new(None),
            browser: Mutex::new(BTreeMap::new()),
        })
    }
    fn look(c: &Catalogue, p: &Project) -> Result<Value> {
        p.validate()?;
        ensure!(p.catalogue == c.identity, "project catalogue has changed");
        let id = p
            .appearance
            .as_ref()
            .context("choose a native appearance before editing its colours")?;
        c.selections(c.item(&p.item)?)?
            .into_iter()
            .find(|look| look["id"] == *id)
            .context("appearance does not belong to this weapon")
    }
    fn plan(&self, look: &Value) -> Result<Plan> {
        let resource: ResourceKey = serde_json::from_value(look["resource"].clone())?;
        let mut reader = self.source.resources();
        let app = (*reader.document(&resource)?).clone();
        ensure!(
            app["Data"]["RootChunk"]["$type"] == "appearanceAppearanceResource",
            "recolouring requires an appearance resource"
        );
        let mut plan = Plan {
            app,
            appearance: look["appearance"]
                .as_str()
                .context("appearance name absent")?
                .into(),
            meshes: BTreeMap::new(),
            setups: BTreeMap::new(),
            templates: BTreeMap::new(),
            regions: Vec::new(),
        };
        for binding in look
            .pointer("/context/material_bindings")
            .and_then(Value::as_array)
            .context("resolved material bindings absent")?
        {
            let binding = &binding["binding"];
            if binding["shadow_only"] == true {
                continue;
            }
            let key: ResourceKey = serde_json::from_value(binding["material_source"].clone())?;
            let geometry: ResourceKey = serde_json::from_value(binding["geometry"].clone())?;
            ensure!(
                geometry.native_hash() == key.native_hash(),
                "this appearance uses a separate material-source mesh; its recolour binding recipe is not implemented yet"
            );
            let mesh = reader.document(&key)?;
            let appearance = binding["appearance"]
                .as_str()
                .context("mesh appearance absent")?;
            let materials =
                material::active_local_material_documents(&mesh, appearance, self.source.schema())?;
            let slots = binding["chunk_materials"]
                .as_array()
                .context("chunk materials absent")?;
            // Active documents are returned in material-entry index order.
            let names: BTreeSet<_> = slots.iter().filter_map(Value::as_str).collect();
            let mut entries: Vec<_> = mesh["Data"]["RootChunk"]["materialEntries"]
                .as_array()
                .context("material entries absent")?
                .iter()
                .filter(|e| {
                    e["isLocalInstance"] == true
                        && text(&e["name"]).is_some_and(|n| names.contains(n))
                })
                .collect();
            entries.sort_by_key(|e| e["index"].as_u64().unwrap_or(0));
            let out = plan
                .meshes
                .entry(key.native_hash())
                .or_insert_with(|| Mesh {
                    document: (*mesh).clone(),
                    setups: BTreeMap::new(),
                });
            for (entry, document) in entries.into_iter().zip(materials) {
                let Some(values) = document["Data"]["RootChunk"]["values"].as_array() else {
                    continue;
                };
                let Some(setup) = values.iter().find_map(|v| depot(&v["MultilayerSetup"])) else {
                    continue;
                };
                let slot = text(&entry["name"]).context("material slot name absent")?;
                out.setups.insert(slot.into(), setup.into());
                if plan.setups.contains_key(setup) {
                    continue;
                }
                let document = (*reader.document(&ResourceKey::Path(setup.into()))?).clone();
                for (index, layer) in document["Data"]["RootChunk"]["layers"]
                    .as_array()
                    .context("multilayer layers absent")?
                    .iter()
                    .enumerate()
                {
                    if layer["opacity"].as_f64() == Some(0.) {
                        continue;
                    }
                    let Some(path) = depot(&layer["material"]) else {
                        continue;
                    };
                    let template = if let Some(t) = plan.templates.get(path) {
                        t.clone()
                    } else {
                        let t = (*reader.document(&ResourceKey::Path(path.into()))?).clone();
                        plan.templates.insert(path.into(), t.clone());
                        t
                    };
                    let choice = text(&layer["colorScale"]).unwrap_or("null_null");
                    let Some(channels) = colour_entry(&template, choice)
                        .and_then(|entry| entry.pointer("/v/Elements"))
                        .and_then(Value::as_array)
                    else {
                        continue;
                    };
                    if channels.len() != 3 {
                        continue;
                    }
                    let colour = format!(
                        "#{:02X}{:02X}{:02X}",
                        srgb(channels[0].as_f64().unwrap_or(0.)),
                        srgb(channels[1].as_f64().unwrap_or(0.)),
                        srgb(channels[2].as_f64().unwrap_or(0.))
                    );
                    let label = path
                        .rsplit('\\')
                        .next()
                        .unwrap_or(path)
                        .trim_end_matches(".mltemplate")
                        .split('_')
                        .filter(|word| *word != "hq" && !word.chars().all(|c| c.is_ascii_digit()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let set = setup
                        .rsplit('\\')
                        .next()
                        .unwrap_or(setup)
                        .trim_end_matches(".mlsetup")
                        .split("__")
                        .last()
                        .unwrap_or("material");
                    let suffix = format!("_{}", plan.appearance);
                    let part = set
                        .strip_suffix(&suffix)
                        .unwrap_or(set)
                        .trim_end_matches("_default")
                        .replace('_', " ");
                    let role = if index == 0 {
                        "Base coat".into()
                    } else {
                        format!("Layer {index}")
                    };
                    plan.regions.push(Region {
                        id: digest(&format!("{setup}#{index}")),
                        label: format!("{part} · {role} ({label})"),
                        colour,
                        layer: index,
                        setup: setup.into(),
                        template: path.into(),
                        choice: choice.into(),
                    });
                }
                plan.setups.insert(setup.into(), document);
            }
        }
        Ok(plan)
    }
    pub fn browser_scene(&self, c: &Catalogue, p: &Project) -> Result<Arc<Vec<u8>>> {
        Ok(Arc::clone(&self.browser_asset(c, p)?.scene))
    }
    pub fn browser_layers(&self, c: &Catalogue, p: &Project) -> Result<Value> {
        Ok(self.browser_asset(c, p)?.layers.clone())
    }
    fn browser_asset(&self, c: &Catalogue, p: &Project) -> Result<Arc<BrowserAsset>> {
        p.validate()?;
        ensure!(p.catalogue == c.identity, "project catalogue has changed");
        let looks = c.selections(c.item(&p.item)?)?;
        let look = looks
            .iter()
            .find(|v| p.appearance.as_ref().is_some_and(|id| v["id"] == *id))
            .or_else(|| p.appearance.is_none().then(|| looks.first()).flatten())
            .context("native appearance absent")?;
        let key = look["id"].as_str().context("selection ID absent")?;
        let mut cache = self
            .browser
            .lock()
            .map_err(|_| anyhow::anyhow!("browser scene cache unavailable"))?;
        if let Some(scene) = cache.get(key) {
            return Ok(Arc::clone(scene));
        }
        let resource: ResourceKey = serde_json::from_value(look["resource"].clone())?;
        let options = AssemblyOptions {
            bind_pose: true,
            ..Default::default()
        };
        let directory = tempfile::tempdir()?;
        let assembly = self.source.load_assembly(
            &resource,
            look["appearance"].as_str(),
            &options,
            &directory.path().join("scene"),
        )?;
        let mut layers = Vec::new();
        for node in assembly.components.values() {
            let Some(asset) = assembly.scene.asset_for_node(*node) else {
                continue;
            };
            let Some(manifest) = asset.material_provenance()? else {
                continue;
            };
            for material in asset.materials() {
                let Some(data) = ghostline_red::pbr::colour_layers(material, &manifest)? else {
                    continue;
                };
                let mut png = Cursor::new(Vec::new());
                data.atlas.write_to(&mut png, image::ImageFormat::Png)?;
                let ids: Vec<_> = (0..data.colours.len())
                    .map(|index| digest(&format!("{}#{index}", data.setup)))
                    .collect();
                layers.push(json!({"node":assembly.scene.node(*node).context("scene node absent")?.name,"material":material.name,"colours":data.colours,"regions":ids,"columns":data.columns,"png":png.into_inner()}));
            }
        }
        let scene = Arc::new(BrowserAsset {
            scene: Arc::new(assembly.scene.browser_glb()?),
            layers: json!(layers),
        });
        if cache.len() >= 4 {
            cache.pop_first();
        }
        cache.insert(key.into(), Arc::clone(&scene));
        Ok(scene)
    }

    pub fn regions(&self, c: &Catalogue, p: &Project) -> Result<Vec<Region>> {
        let look = Self::look(c, p)?;
        let key = look["id"].as_str().context("selection ID absent")?;
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| anyhow::anyhow!("material cache unavailable"))?;
        if !plans.contains_key(key) {
            if plans.len() >= 8 {
                plans.clear();
            }
            plans.insert(key.into(), self.plan(&look)?);
        }
        Ok(plans[key].regions.clone())
    }
    pub fn validate(&self, c: &Catalogue, p: &Project) -> Result<()> {
        if p.recolours.is_empty() {
            return Ok(());
        }
        let regions = self.regions(c, p)?;
        for edit in &p.recolours {
            ensure!(
                regions.iter().any(|r| r.id == edit.region),
                "recolour region does not belong to the selected appearance"
            );
        }
        Ok(())
    }
    pub fn build(&self, c: &Catalogue, p: &Project) -> Result<Arc<Assets>> {
        self.validate(c, p)?;
        ensure!(!p.recolours.is_empty(), "choose a new layer colour first");
        let visual_key = serde_json::to_string(
            &json!({"catalogue":p.catalogue,"item":p.item,"appearance":p.appearance,"colours":p.recolours,"id":p.id,"record":p.record,"mode":p.mode}),
        )?;
        let mut artifacts = self
            .artifacts
            .lock()
            .map_err(|_| anyhow::anyhow!("preview cache unavailable"))?;
        if let Some(assets) = artifacts.get(&visual_key) {
            return Ok(Arc::clone(assets));
        }
        let look = Self::look(c, p)?;
        let key = look["id"].as_str().context("selection ID absent")?;
        let plans = self
            .plans
            .lock()
            .map_err(|_| anyhow::anyhow!("material cache unavailable"))?;
        let plan = plans.get(key).context("material plan absent")?;
        let staging = tempfile::tempdir()?;
        let depot_root = staging.path().join("depot");
        fs::create_dir(&depot_root)?;
        let prefix = format!("mod\\ghostline_item_builder\\{}", p.id);
        let mut setups = plan.setups.clone();
        let mut setup_paths = BTreeMap::new();
        for edit in &p.recolours {
            let region = plan
                .regions
                .iter()
                .find(|r| r.id == edit.region)
                .context("region absent")?;
            let mut template = plan.templates[&region.template].clone();
            let values = template
                .pointer_mut("/Data/RootChunk/overrides/colorScale")
                .and_then(Value::as_array_mut)
                .context("template colour overrides absent")?;
            let entry = values
                .iter_mut()
                .find(|e| text(&e["n"]) == Some(&region.choice))
                .context("template colour selection absent")?;
            let mut channels = Vec::new();
            for start in [1, 3, 5] {
                channels.push(linear(u8::from_str_radix(
                    &edit.colour[start..start + 2],
                    16,
                )?));
            }
            entry["v"]["Elements"] = json!(channels);
            let path = format!("{prefix}\\layer_{}.mltemplate", region.id);
            self.write(&depot_root, &path, &template)?;
            setups.get_mut(&region.setup).context("setup absent")?["Data"]["RootChunk"]["layers"]
                [region.layer]["material"]["DepotPath"] = resource_path(&path);
            setup_paths.insert(
                region.setup.clone(),
                format!("{prefix}\\setup_{}.mlsetup", digest(&region.setup)),
            );
        }
        for (old, new) in &setup_paths {
            self.write(&depot_root, new, &setups[old])?;
        }
        let mut replacements = BTreeMap::new();
        for (hash, mesh) in &plan.meshes {
            let mut document = mesh.document.clone();
            let mut changed = false;
            for (slot, setup) in &mesh.setups {
                if let Some(new) = setup_paths.get(setup) {
                    material::override_material_parameters(
                        &mut document,
                        slot,
                        &[
                            json!({"$type":"rRef:Multilayer_Setup","MultilayerSetup":{"DepotPath":resource_path(new),"Flags":"Default"}}),
                        ],
                        self.source.schema(),
                    )?;
                    changed = true;
                }
            }
            if changed {
                let path = format!("{prefix}\\mesh_{hash}.mesh");
                self.write(&depot_root, &path, &document)?;
                replacements.insert(*hash, path);
            }
        }
        let mut app = plan.app.clone();
        if p.mode == crate::project::Mode::Variant {
            app["Data"]["RootChunk"]["preset"]["$value"] = json!(p.record);
        }
        replace_paths(&mut app, &replacements);
        let app_path = format!("{prefix}\\appearance.app");
        self.write(&depot_root, &app_path, &app)?;
        let factory_path = format!("{prefix}\\factory.csv");
        self.write(&depot_root,&factory_path,&json!({"Data":{"RootChunk":{"$type":"C2dArray","cookingPlatform":"PLATFORM_PC","headers":["name","path","preload"],"data":[[factory(p),app_path,"true"]]}}}))?;
        let archive_path = staging.path().join("recolour.archive");
        ghostline_red::archive::pack(&depot_root, &archive_path)?;
        let previews = {
            let source = GameSource::open_with_overlays(
                &self.game,
                self.schema.as_deref(),
                std::slice::from_ref(&archive_path),
            )?;
            let options = AssemblyOptions {
                bind_pose: true,
                ..AssemblyOptions::default()
            };
            let assembly = source.load_assembly(
                &ResourceKey::Path(app_path.clone()),
                Some(&plan.appearance),
                &options,
                &staging.path().join("preview"),
            )?;
            let mut gpu = self
                .gpu
                .lock()
                .map_err(|_| anyhow::anyhow!("GPU unavailable"))?;
            if gpu.is_none() {
                *gpu = Some(pollster::block_on(Renderer::new())?);
            }
            let gpu = gpu.as_ref().context("GPU initialization failed")?;
            let mut prepared = gpu.prepare_scene(&assembly.scene)?;
            let mut previews = BTreeMap::new();
            for (name, view) in [
                ("hero", View::Hero),
                ("left", View::Left),
                ("front", View::Front),
            ] {
                let rendered = gpu.render(
                    &mut prepared,
                    RenderOptions {
                        size: 768,
                        camera: Camera::Frame(view),
                    },
                )?;
                let mut png = Cursor::new(Vec::new());
                rendered.image.write_to(&mut png, image::ImageFormat::Png)?;
                previews.insert(name.into(), png.into_inner());
            }
            previews
        };
        let icon = inventory_image(&previews["left"])?;
        let texture_path = format!("{prefix}\\inventory.xbm");
        self.write(
            &depot_root,
            &texture_path,
            &ghostline_red::texture::rgba_ui_texture(&icon),
        )?;
        self.write(
            &depot_root,
            &format!("{prefix}\\inventory.inkatlas"),
            &ghostline_red::texture::single_image_atlas(&texture_path, "inventory"),
        )?;
        let mut png = Cursor::new(Vec::new());
        icon.write_to(&mut png, image::ImageFormat::Png)?;
        let final_archive = staging.path().join("complete.archive");
        ghostline_red::archive::pack(&depot_root, &final_archive)?;
        let assets = Arc::new(Assets {
            archive: fs::read(final_archive)?,
            xl: format!("factories:\n  - {factory_path}\n"),
            previews,
            inventory_icon: png.into_inner(),
        });
        if artifacts.len() >= 4 {
            artifacts.pop_first();
        }
        artifacts.insert(visual_key, Arc::clone(&assets));
        Ok(assets)
    }
    pub fn highlight(
        &self,
        c: &Catalogue,
        p: &Project,
        region_id: &str,
        view: View,
    ) -> Result<Vec<u8>> {
        let region = self
            .regions(c, p)?
            .into_iter()
            .find(|r| r.id == region_id)
            .context("unknown material layer")?;
        let look = Self::look(c, p)?;
        let key = look["id"].as_str().context("selection ID absent")?;
        let mut state = self
            .highlight
            .lock()
            .map_err(|_| anyhow::anyhow!("layer preview unavailable"))?;
        let mut gpu = self
            .gpu
            .lock()
            .map_err(|_| anyhow::anyhow!("GPU unavailable"))?;
        if gpu.is_none() {
            *gpu = Some(pollster::block_on(Renderer::new())?);
        }
        let gpu = gpu.as_ref().context("GPU initialization failed")?;
        if state.as_ref().is_none_or(|scene| scene.key != key) {
            let work = tempfile::tempdir()?;
            let assembly = self.source.load_assembly(
                &serde_json::from_value(look["resource"].clone())?,
                look["appearance"].as_str(),
                &AssemblyOptions {
                    bind_pose: true,
                    ..AssemblyOptions::default()
                },
                &work.path().join("asset"),
            )?;
            let prepared = gpu.prepare_scene(&assembly.scene)?;
            *state = Some(HighlightScene {
                key: key.into(),
                assembly,
                prepared,
                work,
            });
        }
        let state = state.as_mut().context("layer scene absent")?;
        for (component, node) in &state.assembly.components {
            let Some(asset) = state.assembly.scene.asset_for_node(*node) else {
                continue;
            };
            let manifest = asset.material_provenance()?;
            for (index, material) in asset.materials().iter().enumerate() {
                // Retain native decal alpha so flat decal cards cannot occlude the mask.
                if material.decal.is_some() {
                    continue;
                }
                let selected = manifest
                    .as_ref()
                    .and_then(|m| m["Materials"].as_array())
                    .is_some_and(|records| {
                        records.iter().any(|r| {
                            r["Name"] == material.name
                                && r["Data"]["MultilayerSetup"] == region.setup
                        })
                    });
                let edited = if selected {
                    let path = state.work.path().join(format!(
                        "{}_{}_{}.png",
                        digest(component),
                        index,
                        region.id
                    ));
                    ghostline_red::pbr::highlight_layer(
                        material,
                        manifest.as_ref().context("material manifest absent")?,
                        region.layer,
                        &path,
                    )?
                } else {
                    let mut edited = material.clone();
                    edited.base_color_texture = None;
                    edited.base_color_factor = [0.16, 0.18, 0.18, 1.];
                    edited.normal_texture = None;
                    edited.metallic_roughness_texture = None;
                    edited.metallic_factor = 0.;
                    edited.roughness_factor = 1.;
                    edited.emissive_texture = None;
                    edited.emissive_factor = [0.; 3];
                    edited
                };
                gpu.set_material(&mut state.prepared, *node, index, edited)?;
            }
        }
        let image = gpu
            .render(
                &mut state.prepared,
                RenderOptions {
                    size: 768,
                    camera: Camera::Frame(view),
                },
            )?
            .image;
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png)?;
        Ok(png.into_inner())
    }
    fn write(&self, root: &Path, path: &str, document: &Value) -> Result<()> {
        let output = root.join(path.replace('\\', "/"));
        fs::create_dir_all(output.parent().context("resource parent absent")?)?;
        let json = output.with_extension("source.json");
        fs::write(&json, serde_json::to_vec(document)?)?;
        writer::write(&json, &output, self.source.schema())
            .with_context(|| format!("serialize {path}"))?;
        let readback = ghostline_red::codec::decode_wkit_from_bytes(
            &fs::read(&output)?,
            self.source.schema(),
        )?;
        ensure!(
            readback["Data"]["RootChunk"]["$type"] == document["Data"]["RootChunk"]["$type"],
            "resource readback failed"
        );
        if document["Data"]["RootChunk"]["$type"] == "C2dArray" {
            for field in ["headers", "data"] {
                ensure!(
                    readback["Data"]["RootChunk"][field] == document["Data"]["RootChunk"][field],
                    "factory {field} changed during serialization"
                );
            }
            ensure!(
                readback["Data"]["RootChunk"]["$rawTail"].is_string(),
                "compiled factory appendix absent"
            );
        }
        fs::remove_file(json)?;
        Ok(())
    }
}

fn inventory_image(png: &[u8]) -> Result<image::RgbaImage> {
    let image = image::load_from_memory(png)?.into_rgba8();
    let mut bounds = (image.width(), image.height(), 0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] > 8 {
            bounds.0 = bounds.0.min(x);
            bounds.1 = bounds.1.min(y);
            bounds.2 = bounds.2.max(x + 1);
            bounds.3 = bounds.3.max(y + 1);
        }
    }
    ensure!(
        bounds.2 > bounds.0 && bounds.3 > bounds.1,
        "inventory render is empty"
    );
    let cropped = image::imageops::crop_imm(
        &image,
        bounds.0,
        bounds.1,
        bounds.2 - bounds.0,
        bounds.3 - bounds.1,
    )
    .to_image();
    let resized = image::DynamicImage::ImageRgba8(cropped)
        .resize(304, 144, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let mut canvas = image::RgbaImage::new(320, 160);
    image::imageops::overlay(
        &mut canvas,
        &resized,
        i64::from((320 - resized.width()) / 2),
        i64::from((160 - resized.height()) / 2),
    );
    Ok(canvas)
}
fn replace_paths(value: &mut Value, replacements: &BTreeMap<u64, String>) {
    if value["$type"] == "ResourcePath" {
        let hash = value["$value"].as_u64().or_else(|| {
            text(value).and_then(|s| {
                s.parse()
                    .ok()
                    .or_else(|| Some(ghostline_red::archive::depot_path_hash(s)))
            })
        });
        if let Some(new) = hash.and_then(|h| replacements.get(&h)) {
            *value = resource_path(new);
            return;
        }
    }
    match value {
        Value::Object(map) => {
            for v in map.values_mut() {
                replace_paths(v, replacements);
            }
        }
        Value::Array(a) => {
            for v in a {
                replace_paths(v, replacements);
            }
        }
        _ => {}
    }
}
