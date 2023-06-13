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

    let output;
    let runner;
    if cfg!(target_os = "windows") {
        // Run: yarn install
        output = Command::new("yarn")
            .arg("install")
            .arg("--pure-lockfile")
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

    let esbuild_args = vec![
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
        output = Command::new("npx")
            .args(esbuild_args)
            .output()
            .expect("Getting `yarn dlx esbuild ...` output failed");
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
}
