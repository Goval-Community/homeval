// use deno_core::{include_js_files, Extension};
// use std::env;
// use std::path::PathBuf;
extern crate prost_build;

fn main() {
    println!("cargo:rerun-if-changed=src/protobufs");
    prost_build::compile_protos(
        &["src/protobufs/goval.proto", "src/protobufs/client.proto"],
        &["src/"],
    )
    .unwrap();

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
