/** Pure helpers that turn host facts into display text. */

const EXTENSION_LABELS: Record<string, string> = {
  rmmzsave: "RPG Maker MZ",
  rpgsave: "RPG Maker MV",
  rvdata2: "RPG Maker VX Ace",
  rvdata: "RPG Maker VX",
  rxdata: "RPG Maker XP",
  lsd: "RPG Maker 2000/2003",
  es3: "Easy Save 3",
  rsv: "RAGS",
  sav: "SAV",
  mcr: "PS1 memory card",
  mcd: "Memory card",
  mem: "PS1 memory card",
  gme: "PS1 memory card (.gme)",
  ps2: "PS2 memory card",
  psu: "PS2 .psu",
  sps: "SharkPort GBA",
  xps: "X-Port GBA",
  sol: "Flash",
  reg: "Registry (.reg)",
  sqlite: "SQLite",
  sqlite3: "SQLite",
  msgpack: "MessagePack",
  msgp: "MessagePack",
  cbor: "CBOR",
  bson: "BSON",
  nbt: "Minecraft NBT",
  mca: "Minecraft region",
  xml: "XML",
  properties: "Properties",
  toml: "TOML",
  yaml: "YAML",
  yml: "YAML",
  csv: "CSV",
  plist: "Property list",
  plr: "Terraria",
  wld: "Terraria",
  das: "Dragon Age",
  erf: "BioWare ERF",
  mod: "Neverwinter Nights",
  hak: "Neverwinter Nights",
  nwm: "Neverwinter Nights",
  sl2: "Elden Ring / Dark Souls",
  bnd: "BND4",
  brs: "Brickadia",
  brz: "Brickadia",
  mpq: "MPQ",
  sc2save: "StarCraft II",
  w3z: "Warcraft III",
  pb: "Protocol Buffers",
  pbf: "Protocol Buffers",
  protobuf: "Protocol Buffers",
  protobin: "Protocol Buffers",
  sii: "SCS SII",
  state: "RetroArch",
  state1: "RetroArch",
  state2: "RetroArch",
  state3: "RetroArch",
  frz: "Snes9x",
  asd: "AliceSoft",
  as2: "AliceSoft",
  asdbak: "AliceSoft",
  c2save: "Construct",
  c3save: "Construct",
};

const CODEC_LABELS: Record<string, string> = {
  "format.json": "JSON",
  "format.rpgmaker-json": "RPG Maker",
  "format.es3": "Easy Save 3",
  "format.es3-json": "Easy Save 3",
  "format.adventure-creator": "Adventure Creator",
  "format.pixel-crushers": "Pixel Crushers",
  "format.odin": "Odin Serializer",
  "format.gvas": "Unreal",
  "format.ruby-marshal": "Ruby Marshal",
  "format.java-serialization": "Java",
  "format.python-pickle": "Python pickle",
  "format.msgpack": "MessagePack",
  "format.cbor": "CBOR",
  "format.bson": "BSON",
  "format.nbt": "Minecraft NBT",
  "format.minecraft-region": "Minecraft region",
  "format.flash-sol": "Flash LSO",
  "format.pokemon-gen3": "Pokémon Gen 3",
  "format.lcf": "RPG Maker 2000/2003",
  "format.nrbf": ".NET BinaryFormatter",
  "format.reg": "Registry (.reg)",
  "format.godot-config": "Godot",
  "format.godot-variant": "Godot",
  "format.godot-resource": "Godot",
  "format.godot-resource-text": "Godot",
  "format.sqlite": "SQLite",
  "format.leveldb-sst": "LevelDB",
  "format.leveldb-log": "LevelDB",
  "format.rocksdb": "RocksDB",
  "format.litedb": "LiteDB",
  "format.wolf-rpg": "Wolf RPG",
  "format.qsp": "QSP",
  "format.ags": "Adventure Game Studio",
  "format.onscripter": "NScripter",
  "format.alicesoft": "AliceSoft",
  "format.artemis": "Artemis",
  "format.emuera": "Emuera",
  "format.tjs2": "KiriKiri",
  "format.gamemaker-ds-map": "GameMaker",
  "format.gamemaker-ds-list": "GameMaker",
  "format.gta-iii": "Grand Theft Auto III",
  "format.gta-iv": "Grand Theft Auto IV",
  "format.gta-sa": "Grand Theft Auto: San Andreas",
  "format.naninovel": "Naninovel",
  "format.witcher3": "The Witcher 3",
  "format.satisfactory": "Satisfactory",
  "format.slime-rancher-2": "Slime Rancher 2",
  "format.terraria-player": "Terraria",
  "format.terraria-world": "Terraria",
  "format.xml": "XML",
  "format.ini": "INI",
  "format.properties": "Properties",
  "format.toml": "TOML",
  "format.yaml": "YAML",
  "format.csv": "CSV",
  "format.plist": "Property list",
  "format.gff": "BioWare GFF",
  "format.gff4": "Dragon Age",
  "container.erf": "BioWare ERF",
  "container.psx-card": "PS1 memory card",
  "container.psx-gme": "PS1 memory card (.gme)",
  "container.ps2-card": "PS2 memory card",
  "container.psu": "PS2 .psu",
  "container.sharkport": "SharkPort GBA",
  "container.bnd4": "BND4",
  "container.mpq": "MPQ",
  "container.brz": "Brickadia",
  "format.brickadia-brs": "Brickadia",
  "format.protobuf": "Protocol Buffers",
  "format.bitser": "Bitser",
  "format.scs-sii": "SCS SII",
  "format.scs-sii-binary": "SCS SII (binary)",
  "format.scs-sii-encrypted": "SCS SII (encrypted)",
  "format.snes9x-snapshot": "Snes9x snapshot",
  "format.retroarch-state": "RetroArch state",
  "format.emu-sram": "Emulator SRAM",
};

const ENGINE_LABELS: Record<string, string> = {
  sugarcube: "SugarCube",
  renpy: "Ren'Py",
  naninovel: "Naninovel",
  tyranoscript: "TyranoScript",
  "rpgmaker-mv": "RPG Maker MV",
  "rpgmaker-mz": "RPG Maker MZ",
};

/**
 * Short engine or format name for the badge; `null` when nothing is known. `engine` is the engine
 * the host found by the document's shape; it wins over the file name and the codec.
 */
export function engineLabel(fileName: string, codec: string, engine?: string | null): string | null {
  const byShape = engine ? ENGINE_LABELS[engine] : undefined;
  if (byShape) return byShape;
  const dot = fileName.lastIndexOf(".");
  const extension = dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : "";
  const byExtension = EXTENSION_LABELS[extension];
  if (byExtension && extension !== "sav") return byExtension;
  return CODEC_LABELS[codec] ?? byExtension ?? null;
}

/** Last part of a path, for either kind of separator. */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

/**
 * Default file name of "Export .json": `file1.rmmzsave` becomes `file1-export.json`, so it never
 * equals the name of a `.json` save.
 */
export function exportFileName(fileName: string): string {
  const dot = fileName.lastIndexOf(".");
  const stem = dot > 0 ? fileName.slice(0, dot) : fileName;
  return `${stem}-export.json`;
}

/** Folder part of a path (empty when there is none). */
export function dirName(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index >= 0 ? path.slice(0, index) : "";
}
