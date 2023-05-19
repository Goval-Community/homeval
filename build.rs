// use deno_core::{include_js_files, Extension};
// use std::env;
// use std::path::PathBuf;
use std::process::Command;
extern crate prost_build;

fn main() {
    println!("cargo:rerun-if-changed=src/protobufs,api.js");

    prost_build::compile_protos(
        &["src/protobufs/goval.proto", "src/protobufs/client.proto"],
        &["src/"],
    )
    .unwrap();

    let output = Command::new("bun")
        .arg("install")
        .output()
        .expect("Failed to install js packages");

    assert!(
        output.status.code().expect("exit code needed") == 0,
        "Bun install failed"
    );

    // bun x esbuild ./api.js --bundle --minify --platform=browser --outfile=src/api.js
    let output = Command::new("bun")
        .arg("x")
        .arg("esbuild")
        .arg("./api.js")
        .arg("--bundle")
        .arg("--minify")
        .arg("--platform=browser")
        .arg("--outfile=src/api.js")
        .output()
        .expect("Esbuild failed");

    assert!(
        output.status.code().expect("exit code needed") == 0,
        "Esbuild failed"
    );

    // let runjs_extension = Extension::builder("runjs")
    //     .esm(include_js_files!("src/runtime.js",))
    //     .build();

    // // Build the file path to the snapshot.
    // let o = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // let snapshot_path = o.join("RUNTIME_SNAPSHOT.bin");

    // // Create the snapshot.
    // deno_core::snapshot_util::create_snapshot(deno_core::snapshot_util::CreateSnapshotOptions {
    //     cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
    //     snapshot_path: snapshot_path,
    //     startup_snapshot: None,
    //     extensions: vec![runjs_extension],
    //     compression_cb: None,
    //     snapshot_module_load_cb: None,
    // })
}
