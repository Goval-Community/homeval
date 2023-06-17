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

    let output;
    let runner;
    if cfg!(target_os = "windows") {
        // Run: yarn install
        output = Command::new("cmd")
            .arg("/C")
            .arg("yarn install --pure-lockfile")
            .output()
            .expect("Getting yarn output failed");
        runner = "yarn";
    } else {
        // Run: bun install
        output = Command::new("bun")
            .arg("install")
            .arg("-y")
            .output()
            .expect("Getting bun output failed");
        runner = "bun";
    }

    assert!(output.status.success(), "Running {} install failed", runner);

    let out_file = format!("--outfile={}/api.js", std::env::var("OUT_DIR").unwrap());

    let mut esbuild_args = vec![
        "esbuild",
        "src/api.js",
        "--bundle",
        "--minify",
        "--platform=browser",
        &out_file,
    ];

    let output;
    let runner;
    if cfg!(target_os = "windows") {
        // Run: npx esbuild ...
        let mut cmd_arg = vec!["npx"];
        cmd_arg.append(&mut esbuild_args);
        output = Command::new("cmd")
            .arg("/C")
            .arg(cmd_arg.join(" "))
            .output()
            .expect("Getting `npx esbuild ...` output failed");
        runner = "npx";
    } else {
        // Run: bun x esbuild ...
        output = Command::new("bun")
            .arg("x")
            .args(esbuild_args)
            .output()
            .expect("Getting `bun x esbuild ...` output failed");
        runner = "bun";
    }

    assert!(
        output.status.success(),
        "Running esbuild via {} failed",
        runner
    );

    // TODO: snapshot api.js as well
    let homeval_extension = Extension::builder("homeval")
        .js(include_js_files!(homeval "src/runtime.js",))
        .build();

    // Build the file path to the snapshot.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let snapshot_path = out_dir.join("HOMEVAL_JS_SNAPSHOT.bin");

    // Create the snapshot.
    for file in
        deno_core::snapshot_util::create_snapshot(deno_core::snapshot_util::CreateSnapshotOptions {
            cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
            snapshot_path,
            startup_snapshot: None,
            extensions: vec![homeval_extension],
            compression_cb: None,
            snapshot_module_load_cb: None,
        })
        .files_loaded_during_snapshot
    {
        println!(
            "cargo:rerun-if-changed={}",
            file.into_os_string().into_string().unwrap()
        )
    }
}
