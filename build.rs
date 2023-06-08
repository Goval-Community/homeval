use std::process::Command;

use prost_build::Config;
extern crate prost_build;

fn main() {
    // Only rerun if a protobuf changed, or api.js/package.json is changed
    println!("cargo:rerun-if-changed=src/protobufs");
    println!("cargo:rerun-if-changed=src/api.js");
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

    // Run: bun install
    let output = Command::new("bun")
        .arg("install")
        .output()
        .expect("Getting bun output failed");

    assert!(output.status.success(), "Running bun install failed");

    // Run: bun x esbuild src/api.js --bundle --minify --platform=browser --outfile=$OUT_DIR/api.js
    let output = Command::new("bun")
        .arg("x")
        .arg("esbuild")
        .arg("src/api.js")
        .arg("--bundle")
        .arg("--minify")
        .arg("--platform=browser")
        .arg(format!(
            "--outfile={}/api.js",
            std::env::var("OUT_DIR").unwrap()
        ))
        .output()
        .expect("Getting esbuild output failed");

    assert!(output.status.success(), "Running esbuild failed");
}
