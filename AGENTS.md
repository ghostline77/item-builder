# Item Builder

- Rust owns catalogue access, project validation and mod generation. The browser
  is a client of those operations and must not invent exported game values.
- Consume the published sibling `items` catalogue. Native formats belong in
  `ghostline-red`; shared GPU rendering belongs in `renderer`.
- Preserve source record identities and scope edits to the selected record or a
  new derived record. Never mutate shared native stat groups as a shortcut.
- Check native game sources and `modding_docs` before introducing game semantics.
- Keep generated mods, captures and local projects in ignored `work` or `dist`.
- Verify the real Slaught-O-Matic workflow, exported YAML and UI. Do not claim
  in-game animation or combat validation from an asset preview.
