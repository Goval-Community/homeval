use std::{path::PathBuf, process::Command};

use deno_core::{include_js_files, Extension};
use prost_build::Config;
extern crate prost_build;

fn main() {
    // Only rerun if a protobuf changed, or api.js/package.json is changed
    println!("cargo:rerun-if-changed=src/protobufs");
    println!("cargo:rerun-if-changed=src/api.js");
    println!("cargo:rerun-if-changed=src/runtime.js");

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

    // TODO: snapshot api.js as well
    let outdir = std::env::var("OUT_DIR").unwrap();
    let runjs_extension = Extension::builder("homeval")
        .esm(vec![
            deno_core::ExtensionFileSource {
                specifier: "src/runtime.js".to_string(),
                code: ::deno_core::ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
                    std::path::PathBuf::from("/home/potentialstyx/goval-community/homeval-expand")
                        .join("src/runtime.js"),
                ),
            },
            // deno_core::ExtensionFileSource {
            //     specifier: format!("{}/api.js", outdir),
            //     code: ::deno_core::ExtensionFileSourceCode::LoadedFromFsDuringSnapshot(
            //         std::path::PathBuf::from(outdir).join("api.js"),
            //     ),
            // },
        ])
        .build();

    // Build the file path to the snapshot.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("HOMEVAL_JS_SNAPSHOT.bin");

    // Create the snapshot.
    deno_core::snapshot_util::create_snapshot(deno_core::snapshot_util::CreateSnapshotOptions {
        cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
        snapshot_path,
        startup_snapshot: None,
        extensions: vec![runjs_extension],
        compression_cb: None,
        snapshot_module_load_cb: None,
    })
}
