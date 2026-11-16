# Weapon editing capabilities

Weapon records, attack definitions and appearance assets serve different jobs.
A weapon's inventory description is not a complete description of its combat
behaviour. Editing a shared base record can change many unrelated weapons.
Exports must create owned records and redirect only the selected weapon or its
new variant.

## Existing controls

The editor currently exports damage/DPS scaling, magazine and reload bonuses,
rarity, the iconic flag, disposable-handgun reload conversion, appearance
selection and recolouring. See [native recipes](native-recipes.md). The iconic
flag does not grant another weapon's iconic ability. Custom mesh import and the
additional controls below are not implemented yet.

## Additional controls and their native contracts

| Editing area | Native data | Required handling |
| --- | --- | --- |
| Critical hits | `CritChance`, `CritDamage`; hit effectors | Ordinary crit chance uses percentage points. Attack-crit effectors use fractions. Keep their controls separate. |
| Headshots and penetration | `HeadshotDamageMultiplier`, attack/armour penetration stats | Restrict controls to applicable attacks; distinguish damage multipliers from penetration flags. |
| Rate of fire and bursts | `CycleTime`, `CycleTime_Burst`, `NumShotsInBurst`, trigger-mode records | Edit timing and trigger behaviour together. A DPS multiplier does not set fire rate. |
| Handling | Recoil kick/recovery, hip/ADS spread, aim and equip timings | Offer grouped controls with linked minimum/maximum validation; keep detailed axes under Advanced. |
| Range | `EffectiveRange`, `MaximumRange`, falloff curves | Preserve the source weapon's falloff and projectile behaviour unless explicitly replaced. |
| Ammunition and reload | Ammo record, capacity, reload timings, weapon state machine | Resolve compatible ammo, animation and state-machine contracts. The existing handgun recipe is not a universal conversion. |
| Power weapons | Ricochet chance/count/angles and their prerequisites | Gate by weapon evolution and required player capabilities. |
| Tech weapons | Charge timings, thresholds, multipliers, charged bursts and piercing | Validate threshold ordering and the charged/uncharged attack pair. Changing only `evolution` cannot convert an arbitrary mesh and animation set into a tech weapon. |
| Smart weapons | Lock times/range, target limits/angles, targeting queues, smart-link requirement | Preserve targeting packages and projectile setup; separate hip and ADS controls. |
| Melee | Attack speed, stamina, blocking, combos, dash/deflect flags, attack list | Respect animation and attack definitions, including separate normal/strong/equip attacks. |
| Thrown weapons | Throw attack/projectile, gravity and return/recovery behaviour | Treat as a weapon-family recipe, including recovery abilities; do not expose one generic return-time flat. |
| Damage type | Damage groups, min/max calculations, attack effects, visual tags and material/effect setup | Clone and reconnect the complete conversion. Thermal-looking materials alone do not cause thermal damage. |
| Iconic effects | Mod slots/parts, abilities, packages, effectors, prerequisites and status effects | Copy the selected ability chain into owned records. Preserve unrelated native packages and attachment behaviour. |
| Attachments | Compatible slot and part lists, scope/muzzle/mod records and entity slots | Validate both the gameplay slot and the physical attachment. An attachment record does not create a missing mesh socket. |
| Names, icons and distribution | Localization, inventory atlas, item record and loot/vendor recipes | Keep item creation separate from world placement. Quest Builder owns placed loot containers. |

A useful UI presents **Performance**, **Handling**, **Weapon mechanics**,
**Abilities** and **Appearance**. Controls should appear only for supported
weapon families. Advanced can expose the detailed recipe fields while retaining
the same Rust validation and export path. Vehicle guns, fists, cyberware and
internal weapon records need their own recipes rather than all controls merely
because their type starts with `Wea_`.

## Abilities are graphs of records

An event-driven ability typically follows:

```text
weapon or installed mod part
  → gameplay package
  → effector + prerequisite
  → target status effect
  → status-effect package
  → stat-group effector
  → stat modifiers
```

There are variations: a hit effector can modify the hit directly, and an equipped
package can apply a stat group without a status effect. Generate the chain needed
by the selected recipe rather than attaching every ability at the same point.

For a draw-attack buff, the trigger must be a hit prerequisite with the equip
attack condition, not a condition record used where a prerequisite is expected.
Duration is a stat-modifier group containing `MaxDuration`. Weapon-targeted
stat groups need the weapon application target and removal with their effector.
Cooldown markers must live on the intended actor and be checked before applying
the buff. Time dilation also needs a unique reason, cleanup, and interaction
testing with other time-dilation sources.

The ordinary `CritChance` stat is divided by 100 in the damage calculation.
`ModifyAttackCritChanceEffector` passes its calculated value directly to the
hit's additional crit chance: **100 and 1 mean the same 100-percentage-point
bonus in those two different contexts**. The compiler must convert user-facing
percentages according to the destination field.

Vanilla Errata (`Items.Preset_Katana_E3`) is a useful layered example: thermal
damage groups and `Thermal_Katana` visuals are separate from
`Items.E3KatanaWeaponModAbility`, whose hit prerequisite checks a burning target
and grants attack crit chance. Byakko's
`Items.WakakoKatanaWeaponModAbility` applies
`BaseStatusEffect.Wakako_Tiger_Buff` after a qualifying kill. Neither is reproduced
by setting `IsItemIconic`.

## Custom meshes

Import should start with an existing weapon's entity, rig and appearance contract
and replace a selected mesh component. Keep its native component name or update
all bindings explicitly. Imported geometry must retain its own vertices, UVs and
materials; a reference weapon supplies compatibility information, not replacement
geometry.

Before preview or export, inspect:

| Area | Checks |
| --- | --- |
| Geometry | Index ranges, stream counts, finite coordinates, axes, scale, pivot and placement against the weapon rig. |
| Skinning | Vertex influence streams, palette indices, normalized weights, bone names and inverse binds. A bone list alone does not skin vertices. |
| Bounds | Mesh bounds and per-bone extents covering the imported geometry. |
| Visibility | LOD distances, chunk LOD masks, available LOD geometry and first/third-person visibility. |
| Bindings | Parent and shadow bindings, animation components and visual-controller component names and mesh dependencies. |
| Materials | Chunk-to-material mapping, appearance entries, local/external materials, texture references and material inheritance. |
| Packaging | Owned resource paths, dependency closure and platform restrictions for entity/appearance resources. |

Repairs must be explicit and unambiguous. For example, a rigid, single-bone sword
can be assigned full weight to its declared base bone when the author selects
rigid attachment. That is unsafe for a multi-bone gun. A missing distant LOD
should be reported; using the same geometry as a fallback must be an explicit
choice, not presented as an optimized LOD. Missing materials or ambiguous
component bindings require an author choice rather than guessed substitutions.

`ghostline-red` currently decodes geometry and skinning and replaces geometry or
weights within existing layouts. Supporting a mesh that has no skin stream
requires a native stream-layout rebuild; changing bone metadata alone is
insufficient. That reusable implementation belongs in `ghostline-red`.
`item-builder` should own import mapping, repair choices and the review report;
`renderer` should provide the assembled preview.

A static asset check cannot prove correct drawing, sheathing, dashing or reload
animation in game. Validate those behaviours on the exported weapon.

## Native references

These contracts can be checked against the game's REDmod reference files:

- `items/weapons/melee/bases`, `items/weapons/ranged`: family stats and attacks.
- `items/weapons/parts/mods_abilities.tweak`, `iconic_mods.tweak`: iconic chains.
- `stats/weapon_stats/weapon_stats.tweak`: stat definitions and bounds.
- `status_effect/weapon_mods.tweak`: buffs and duration/stat-group chains.
- `scripts/cyberpunk/damage/damageSystem.script`: ordinary crit calculation.
- `scripts/core/gameplay/effectors/damage/modifyAttackCritChanceEffector.script`:
  attack-specific crit calculation.

The `items/...` paths above are relative to REDmod's
`tweaks/base/gameplay/static_data/database`. Confirm final values and record
types against the installed game's compiled TweakDB; reference sources can lag
behind patches.
