// Include the `goval` module, which is generated from goval.proto.
pub mod goval {
    include!(concat!(env!("OUT_DIR"), "/goval.rs"));
}
