use std::process::Command;
extern crate prost_build;

fn main() {
    // Only rerun if a protobuf changed, or if api.js changed
    println!("cargo:rerun-if-changed=src/protobufs,api.js");

    // Compile protobufs
    prost_build::compile_protos(
        &["src/protobufs/goval.proto", "src/protobufs/client.proto"],
        &["src/"],
    )
    .unwrap();

    // Run: bun install
    let output = Command::new("bun")
        .arg("install")
        .output()
        .expect("Getting bun output failed");

    assert!(output.status.success(), "Running bun install failed");

    // Run: bun x esbuild ./api.js --bundle --minify --platform=browser --outfile=src/api.js
    let output = Command::new("bun")
        .arg("x")
        .arg("esbuild")
        .arg("./api.js")
        .arg("--bundle")
        .arg("--minify")
        .arg("--platform=browser")
        .arg("--outfile=src/api.js")
        .output()
        .expect("Getting esbuild output failed");

    assert!(output.status.success(), "Running esbuild failed");
}
