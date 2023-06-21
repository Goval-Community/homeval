use prost_build::Config;
extern crate prost_build;

fn main() {
    println!("cargo:rerun-if-changed=src/protobufs");

    println!("cargo:rerun-if-changed=package.json");

    // Compile protobufs
    let mut config = Config::new();
    // config.type_attribute(".", "#[serde(rename_all = \"camelCase\")]");
    config.type_attribute(".", "#[derive(serde::Serialize,serde::Deserialize)]");
    // config.extern_path(".google.protobuf", "::prost_types");
    config.extern_path(".google.protobuf", "::prost-wkt-types");
    config.compile_well_known_types();
    config
        .compile_protos(&["src/protobufs/goval.proto"], &["src/"])
        .unwrap();
}
