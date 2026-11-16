# Ghostline Item Builder

Create Cyberpunk 2077 weapon variants and replacements. Choose an appearance,
edit performance and colours, review the generated changes and export a mod.

## Start the editor

Use Rust and a published sibling Items catalogue. Keep the sibling native crates
declared in [Cargo.toml](Cargo.toml) available. Install the parent
`ghostline77` JavaScript workspace with `npm ci` before a fresh build.
Run from this repository:

```powershell
cargo build --release --locked
cargo run --release --locked -- serve --catalogue ../items/dist/catalogue
```

Open the URL printed at startup. The OS chooses a free port; `--port 8795`
sets a fixed port. Browsing, gameplay editing and gameplay-only exports do not
require the game.

Supply `--game "path/to/Cyberpunk 2077"` for interactive 3D previews, custom
colours and their native exports. Reflection metadata loads automatically.

## Author a weapon

1. Search for a concrete weapon record and select its appearance.
2. Edit **Performance** or **Appearance**.
3. In **Export**, choose a new variant or replacement and name it.
4. Inspect **Review changes**, save the project and export the ZIP.

Gameplay exports require RED4ext and TweakXL. Custom-colour exports also require
ArchiveXL and include native appearance resources. Variants are not automatically
added to shops or loot. Follow the ZIP's installation instructions.

- [Editing, previews, installation and CLI export](docs/authoring.md)
- [Native stat and reload recipes](docs/native-recipes.md)
- [Weapon capabilities and custom-mesh contract](docs/weapon-capabilities.md)

The backend owns project validation and packaging. Items supplies published native
selections, Ghostline Red handles native resources, and Renderer produces native
preview images and inventory artwork.
