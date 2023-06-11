// Include the `goval` module, which is generated from goval.proto.
pub mod goval {
    include!(concat!(env!("OUT_DIR"), "/goval.rs"));
}

pub static HOMEVAL_JS_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/HOMEVAL_JS_SNAPSHOT.bin"));
