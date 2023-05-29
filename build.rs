use std::process::Command;
extern crate prost_build;

fn main() {
    // Only rerun if a protobuf changed, or api.js/package.json is changed
    println!("cargo:rerun-if-changed=src/protobufs");
    println!("cargo:rerun-if-changed=src/api.js");
    println!("cargo:rerun-if-changed=package.json");

    // Compile protobufs
    prost_build::compile_protos(&["src/protobufs/goval.proto"], &["src/"]).unwrap();

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
