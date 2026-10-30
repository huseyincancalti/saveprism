Four new languages, more games, and fixes for Linux and macOS.

### What's new
- **More saves:** Grand Theft Auto III, IV and San Andreas (money, health, armour, wanted and chaos level, in-game
  time), Minecraft Java region files (`.mca`, `.mcr`: chunk values such as chest contents) and more GameMaker `ds_map` /
  `ds_list` dumps. Thanks to @YunusTAS13 for all three (#11)!
- **New languages:** Spanish, Russian, Japanese and Simplified Chinese. Pick one in the language menu in the header
  or in Settings. Want SavePrism in your language? A language is a single file:
  [Add a language](https://github.com/huseyincancalti/saveprism/blob/main/CONTRIBUTING.md#add-a-language).
- **Linux:** SavePrism turns off the WebKitGTK renderer that left the window white with some graphics drivers and in
  virtual machines. You no longer need to set `WEBKIT_DISABLE_DMABUF_RENDERER=1` yourself (a value you set is kept).
- **App color:** picking an app color or a color preset now recolors the app; before, the installed app ignored the
  choice.
- **"Where is my save?"** lists the usual save folders on macOS and Linux too, including Steam Proton folders (Steam
  Deck as well). Thanks to @EfeHasNoLuck for the first outside contribution (#10)!

### Download
| System | File |
|---|---|
| Windows 10 and 11 | [SavePrism_0.9.1_x64-setup.exe](https://github.com/huseyincancalti/saveprism/releases/download/v0.9.1/SavePrism_0.9.1_x64-setup.exe) |
| macOS (Intel and Apple silicon) | [SavePrism_0.9.1_universal.dmg](https://github.com/huseyincancalti/saveprism/releases/download/v0.9.1/SavePrism_0.9.1_universal.dmg) |
| Linux | [SavePrism_0.9.1_amd64.AppImage](https://github.com/huseyincancalti/saveprism/releases/download/v0.9.1/SavePrism_0.9.1_amd64.AppImage) or [SavePrism_0.9.1_amd64.deb](https://github.com/huseyincancalti/saveprism/releases/download/v0.9.1/SavePrism_0.9.1_amd64.deb) |

### Good to know
- The Linux and macOS builds are still new. Found a problem? Please open an issue.
- GTA III and GTA IV support is brand new: it was built from the public format description, and we have not had a
  real save from those games yet. If you try one, we would love to hear how it went, and if a save does not open,
  please open an issue.
- The installers are not code-signed. Windows: **More info → Run anyway**. macOS: **System Settings → Privacy &
  Security → Open Anyway**.
