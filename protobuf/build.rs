use prost_build::Config;
extern crate prost_build;

fn main() {
    println!("cargo:rerun-if-changed=src/goval.proto");

    // Compile protobufs
    let mut config = Config::new();
    config
        .compile_protos(&["src/goval.proto"], &["src/"])
        .unwrap();
}
