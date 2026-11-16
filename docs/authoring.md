# Author and export weapon mods

## Run

```powershell
cargo build --release --locked
cargo run --release --locked -- serve --catalogue path/to/items-catalogue
```

Open the local URL printed at startup. By default the OS assigns a free port;
pass `--port 8795` to choose a fixed port. The game is not required to browse,
author projects or export a mod. No game files are modified by the application.
The server binds to loopback and rejects foreign browser origins and hosts.

For **interactive 3D previews** and **new colours**, supply the installed game
directory. Reflection metadata is automatic; `--schema` is an optional advanced
override. Without the game, browsing retains catalogue images:

```powershell
cargo run --release --locked -- serve --catalogue path/to/items-catalogue --game 'path/to/Cyberpunk 2077'
```

In Appearance, choose **Edit custom colours**, select a masked material layer,
and enter a new RGB hex colour or use the colour picker. The interactive preview
loads native geometry and reusable material coefficients into ui-core's browser viewport.
Colour edits update GPU shader uniforms locally, retaining native layer masks,
texture detail, opacity, microblend and coverage by later layers. Geometry and
material inputs are loaded once per appearance; edits do not rebuild the mod.
Drag to orbit, right-drag to pan and scroll to zoom. Hero, Left, Front and Fit
change the camera locally, without requesting new renders. Editing rarity or
gameplay settings retains the loaded scene. Native GPU rendering still produces
the inventory icon and export artwork. Use **Render inventory icon** to inspect
that artwork before export. Browser shading uses standard PBR and
can differ from native image previews.
Visual results are cached independently of gameplay edits and reused by export.
Save projects to retain colour edits. Changing the starting appearance clears
edits to its material layers.

**Highlight affected area** projects the selected native layer mask onto the
weapon in magenta. This shows authored coverage, including areas covered by later
material layers. Turn it off to return to the colour preview.

Colour export packages project-owned `.mlsetup`, `.mltemplate`, mesh and appearance
resources into an `.archive`, with a compiled factory and ArchiveXL registration.
It also generates a transparent inventory icon from the recoloured side view,
cooks it into a native `.xbm`/`.inkatlas`, and binds a project-owned UIIcon record.
The ZIP includes `inventory-icon.png` so the artwork can be inspected directly.
Shared source resources remain unchanged. Native masks, textures, opacity and
surface parameters are retained. **ArchiveXL** is required for these colour mods.
The game installation is required when exporting colour projects from
the CLI too; pass the same `--game` option to `export`.

The first colour editor covers explicit multilayer-template colour overrides.
Texture painting, gradient/decal recolouring and external material-source mesh
bindings require additional editing recipes; they are not silently approximated.

## Author a weapon mod

1. Search for a weapon, preferably its concrete `Items.Preset_…` record.
2. Use **Appearance** to choose a native finish from the render thumbnails.
   Switch camera views, zoom in, or use **Fit** to restore the full image.
3. Use **Performance** for damage, magazine capacity, reload settings, rarity and
   the separate **Iconic status**. Iconic marks the native stat flag; it does not
   invent unique abilities.
4. Use **Export** to name the project and choose a separate variant or replacement.
5. Expand **Review changes** for exact before/after values and generated YAML.
6. Save the project or choose **Export mod** for the installable ZIP.

The inspector supports keyboard tab navigation with Left/Right, Home and End.
On narrow screens it follows the preview; the library remains separately scrollable.

ZIPs contain `r6/tweaks/ghostline-item-builder/<project>.yaml`, a portable project,
installation instructions, a generation report and a CET acquisition command.
Install **RED4ext and TweakXL**, then extract the ZIP into the game directory.
Remove the generated YAML to uninstall. Variants are not automatically distributed
to shops or loot. Existing inventory instances may need replacement after edits.

Projects have undo/redo, local draft recovery, JSON import/export and revision-checked
atomic saves. Catalogue pins cover native weapon data and source identity, excluding
render progress. Rebase explicitly after changing the source catalogue and review
the resulting edits. Export always validates the submitted project again.

CLI export uses the same compiler:

```powershell
cargo run --release --locked -- export --catalogue path/to/items-catalogue --project my-weapon.item.json --out my-weapon.zip
```

## Supported behaviour

Catalogue game-version labels are provenance, not a compatibility gate. Existing
2.31 catalogues remain usable after game updates. Reload conversion validates
the disposable-handgun and native Unity donor properties in the catalogue.

- Damage edits multiply native **DPS**, not a fabricated final damage number. The
  game calculates per-hit damage from level, weapon properties and other bonuses.
- Magazine edits add to `MagazineCapacityBonus` and preserve native base capacity.
- Reload time edits add to `ReloadTimeBonus`, with its native −40% to +40% limits.
- Rarity pins `Quality` and removes random-quality modifiers on the selected record.
- Reloadable conversion supports the 2.31 disposable-handgun contract used by the
  Slaught-O-Matic. It removes `DiscardOnEmpty`, adopts the native Unity weapon state
  machine and reserve handgun ammo, and adds its four reload timing stats. It
  preserves unrelated tags, stats, meshes and animation resources.
- Disposable mode uses the native heavy-weapon state machine and discard tag for
  handguns. Neither operation manufactures missing reload animation clips.
- Appearance edits select an exact existing appearance from that weapon's published
  selections and preserve its registered appearance resource.

Gameplay, reload presentation and assembled appearance still need in-game
playtesting. Previews use catalogue images or prepared interactive geometry. They do not simulate combat.
See [weapon capabilities](weapon-capabilities.md) for the custom-mesh import contract and additional recipes. Exports do not distribute variants to vendors or loot.

## Ownership

`items` owns catalogue discovery and publication. `item-builder` owns projects,
editing recipes, review and packaging. Native resource decoding and serialization
remain in `ghostline-red`; edited-material GPU previews use `renderer`.
The app loads weapon records once and retrieves selection details by bounded,
cached binary searches over the catalogue's sorted shards, avoiding a full scan
of furniture and character preview metadata.

See [native recipe references](native-recipes.md) for the supported stat and
reload contracts.

See [weapon capabilities](weapon-capabilities.md) for additional editing
recipes and the custom-mesh import contract.
