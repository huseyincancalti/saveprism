//! SavePrism codecs.
//!
//! Layers (`layers`) and formats (`json`, `es3`, `gvas`, `marshal`, `lcf`, ...) implement the `Layer` and `Codec` traits of
//! `saveprism-core`. [`register_all`] adds all of them to a [`Registry`].

use saveprism_core::Registry;

pub mod adventure_creator;
pub mod ags;
pub mod alicesoft;
pub mod artemis;
pub mod bitser;
pub mod brickadia;
pub mod bson;
pub mod cbor;
pub mod csv_text;
pub mod emu;
pub mod emuera;
pub mod es3;
pub mod gamemaker;
pub mod gen3;
pub mod gff;
pub mod gff4;
pub mod godot;
pub mod gta3;
pub mod gta4;
pub mod gta_sa;
pub mod gvas;
pub mod ini;
pub mod javaser;
pub mod json;
mod kv_common;
pub mod layers;
pub mod lcf;
pub mod leveldb;
pub mod litedb;
pub mod marshal;
pub mod mca;
pub mod msgpack;
pub mod naninovel;
pub mod nbt;
pub mod nrbf;
pub mod odin;
pub mod onscripter;
pub mod palworld;
pub mod pickle;
pub mod pixelcrushers;
pub mod plist;
pub mod properties;
pub mod proto;
pub mod qsp;
pub mod raw;
pub mod reg;
pub mod renpy;
pub mod rocksdb;
pub mod rpgmaker;
pub mod satisfactory;
pub mod scs_sii;
pub mod slime_rancher;
pub mod sol;
pub mod sqlite;
pub mod terraria;
mod text_common;
mod text_encoding;
pub mod tjs2;
pub mod toml_text;
pub mod witcher3;
pub mod wolf;
pub mod xml;
pub mod yaml;
mod zip_common;

/// Turns an encoding label (a name, a WHATWG label or an EasyRPG `RPG_RT.ini` code page) into the
/// canonical name this crate stores in a step. Returns `None` for an encoding it cannot use.
pub fn resolve_text_encoding(label: &str) -> Option<&'static str> {
    text_encoding::TextEncoding::from_name(label).map(text_encoding::TextEncoding::name)
}

/// Registers every layer and codec of this crate into `reg`.
pub fn register_all(reg: &mut Registry) {
    layers::register(reg);
    ags::register(reg);
    emu::register(reg);
    alicesoft::register(reg);
    artemis::register(reg);
    emuera::register(reg);
    onscripter::register(reg);
    raw::register(reg);
    adventure_creator::register(reg);
    pixelcrushers::register(reg);
    brickadia::register(reg);
    msgpack::register(reg);
    cbor::register(reg);
    bson::register(reg);
    bitser::register(reg);
    nbt::register(reg);
    mca::register(reg);
    json::register(reg);
    leveldb::register(reg);
    litedb::register(reg);
    rocksdb::register(reg);
    rpgmaker::register(reg);
    satisfactory::register(reg);
    scs_sii::register(reg);
    slime_rancher::register(reg);
    es3::register(reg);
    gamemaker::register(reg);
    csv_text::register(reg);
    gen3::register(reg);
    gff::register(reg);
    gff4::register(reg);
    gta3::register(reg);
    gta4::register(reg);
    gta_sa::register(reg);
    gvas::register(reg);
    ini::register(reg);
    palworld::register(reg);
    marshal::register(reg);
    naninovel::register(reg);
    javaser::register(reg);
    lcf::register(reg);
    pickle::register(reg);
    plist::register(reg);
    properties::register(reg);
    proto::register(reg);
    qsp::register(reg);
    reg::register(reg);
    renpy::register(reg);
    terraria::register(reg);
    sol::register(reg);
    nrbf::register(reg);
    odin::register(reg);
    godot::register(reg);
    sqlite::register(reg);
    tjs2::register(reg);
    toml_text::register(reg);
    wolf::register(reg);
    witcher3::register(reg);
    yaml::register(reg);
    xml::register(reg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_all_works_on_an_empty_registry() {
        let mut reg = Registry::default();
        register_all(&mut reg);
        // Registering twice must be harmless (same ids replace each other).
        register_all(&mut reg);
    }
}
