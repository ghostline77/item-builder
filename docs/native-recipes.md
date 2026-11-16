# Native editing contracts

These operations target Cyberpunk 2077 2.31. The published catalogue supplies the
selected record's inherited properties. A separate variant uses TweakXL `$base`;
a replacement writes only the selected record. Shared native modifier records
and groups are never changed. Generated modifier IDs belong to the project.

## Stat edits

Inventory icons are pinned with an explicit `iconPath` when creating a variant or
changing appearances. Empty native paths use automatic appearance-based lookup;
copying that empty value does not preserve an icon after changing the appearance
resource/name. Resolve the original appearance against the native icon-name index
and retain that icon. The review reports category fallbacks when no exact icon is
available. Recoloured exports generate a side-view icon and native texture/atlas
instead of retaining the source artwork. All three atlas slots refer to the same
transparent UI image; export includes an inspectable PNG.

Iconic status is independent of quality. Explicit Iconic adds a project-owned
`BaseStats.IsItemIconic` additive modifier of 1. Explicit Not iconic multiplies
that flag by 0. Keeping original adds no modifier. These controls do not add or
remove unique gameplay abilities.

`statModifiers` uses `!append-once` to add project-owned
`gamedataConstantStatModifier_Record` records. Existing native modifiers remain.
Explicit rarity removes the named quality and plus-tier randomizers through
TweakXL array operations and sets `qualityRestrictedByParent: false`.

| Control | Native stat | Operation |
|---|---|---|
| Damage multiplier | `BaseStats.DPS` | Multiplier |
| Magazine bonus | `BaseStats.MagazineCapacityBonus` | Additive fractional bonus |
| Reload time change | `BaseStats.ReloadTimeBonus` | Additive fraction, −0.4 to +0.4 |

Native references under an installed game's `tools/redmod`:

- `tweaks/base/gameplay/static_data/database/stats/weapon_stats/weapon_stats.tweak`
- `tweaks/base/gameplay/static_data/database/items/weapons/ranged/handguns/slaughtomatic/base_slaughtomatic.tweak`

Local modding reference:
`modding_docs/for-mod-creators-theory/references-lists-and-overviews/cheat-sheet-tweak-ids/weapons/cheat-sheet-weapon-basestats.md`.
The weapon authoring guide demonstrates fractional `ReloadTimeBonus` and
`MagazineCapacityBonus` additions independently of base DPS calculations.

## Disposable handgun to reloadable

The source must be a handgun with `DiscardOnEmpty` and zero base reload timings.
The compiler verifies the native Unity donor's ammo ID using
`ghostline_red::tweakdb::TweakId`, then copies its state-machine and animation
timing properties from the catalogue. Its documented timing modifiers are:

| Stat | Additive value |
|---|---:|
| `ReloadTimeBase` | 0.75 |
| `EmptyReloadTime` | 0.75 |
| `ReloadEndTimeBase` | 0.35 |
| `EmptyReloadEndTime` | 1.25 |

The operation removes the discard tag, sets `ammo: Ammo.HandgunAmmo` and retains
unrelated native stats/tags, including the weapon's identity tag. It does not
remove crafting restrictions associated with that identity.

Native references:

- `tweaks/base/gameplay/static_data/database/items/weapons/ranged/handguns/unity/base_unity.tweak`
- `scripts/cyberpunk/items/weapon.script`: `CanReload` requires reserve ammo and
  a magazine that is not full.
- `scripts/cyberpunk/player/psm/weaponTransitions.script`: discard-on-empty uses
  the native discard tag or heavy equip area; reload timings and logical end
  timings are distinct stats.

The Slaught-O-Matic has the `HeavyWeapon` state-machine name while Unity uses
`Weapon`. Changing the state machine and timings does not supply absent native
animation clips. This is an authored configuration requiring a runtime playtest,
not a certification of animation compatibility.

## Native appearance selection

`appearanceName` is taken from an exact catalogue selection belonging to the
selected weapon. The compiler rejects foreign or unresolved selections. Native
`appearanceResourceName`, factory registration and part records are retained.

Reference: `modding_docs/modding-guides/items-equipment/adding-new-items/weapons/standalone-weapon-appearance.md`.
