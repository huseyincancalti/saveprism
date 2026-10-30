# Changelog

Player-facing notes for each release. The app shows the section of the running version.

## Unreleased

### More saves
- Grand Theft Auto III, Grand Theft Auto IV and Grand Theft Auto: San Andreas saves: money, health, armour, wanted and chaos level, in-game time and the last mission name where the game stores one.
- Minecraft Java region files (`.mca`, `.mcr`): edit the NBT tree of each chunk; LZ4, unknown and external `.mcc` chunks stay read-only.
- GameMaker `ds_list` dumps and the newer `ds_map` format (magic 403), plus arrays and the remaining scalar types in both maps and lists.

## 0.9.1

Four new languages, and fixes for Linux and macOS.

### Languages
- SavePrism now speaks Spanish, Russian, Japanese and Simplified Chinese: pick one in the language menu in the header or in Settings.
- Want SavePrism in your language? A language is a single file: see "Add a language" on GitHub.

### Fixes
- Linux: SavePrism turns off the WebKitGTK renderer that left the window white with some graphics drivers and in virtual machines.
- Picking an app color or a color preset now recolors the app; before, the installed app ignored the choice.

### Where is my save?
- The usual save folders are listed on macOS and Linux too, including Steam Proton folders (Steam Deck as well). Thanks to EfeHasNoLuck for the first outside contribution.

## 0.9.0

SavePrism 0.9.0 is the first public release: Windows, plus the first Linux and macOS builds.

### Open and edit
- Open saves from RPG Maker, Unity, Unreal Engine, Ren'Py, Twine, Godot and many more engines and formats.
- Edit gold, stats, items, variables and switches in a clear list, or switch to JSON for the whole save.
- Every edit is checked: a save that would not come back byte for byte opens read-only.

### Your files stay yours
- Before a file is replaced, a backup is made first, and every written file is read back and checked.
- No account, no upload: everything runs on your PC.

### Get around faster
- Undo and redo, keyboard shortcuts, and search in every list.
- Paste a save code or copy one back out.
- Compare two saves, restore a backup, and find "Where is my save?" for your engine.

### Make it yours
- Text size, color presets and a custom color for each, patterns or your own background photo.
- Choose how many backups to keep and where they go.

### AI help
- Ask AI copies a list for any chatbot; AI apps can also open, compare and edit saves and pasted codes through the MCP server, with the backup folder you chose.

### This release
- Settings with a search box, an in-app third-party notices page, and this What's new dialog.
