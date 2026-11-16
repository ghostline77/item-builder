# Native inventory icon names

`native-weapon-icons.txt` contains the 407 weapon icon identifiers in Cyberpunk
2077 2.31's REDmod `ui/icons/generated/items_icons.tweak`. It contains identifiers,
not image assets. It validates generated source-icon names for offline authoring.

The exporter retains explicit source `iconPath` values. Otherwise it resolves the
source appearance resource, appearance name and visual tags against this index,
then writes a concrete `iconPath` before changing appearance identities. When no
source-specific icon is available, it reports the native weapon-category fallback.

Non-recoloured exports retain the original weapon artwork. Recoloured exports
instead cook their side-view render into a project-owned texture and atlas, with
an explicit UIIcon binding. They include the matching PNG in the ZIP.
