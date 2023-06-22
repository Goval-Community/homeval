mod chat;
mod traits;
mod types;

pub use types::*;

pub struct Channel {
    pub clients: Vec<i32>,
    _inner: Box<dyn traits::Service>,
}

impl Channel {
    pub fn new(
        _id: i32,
        service: String,
        name: Option<String>,
        read: deadqueue::unlimited::Queue<JsMessage>,
    ) -> Channel {
        Channel {
            clients: vec![],
            _inner: Box::new(chat::Chat {}),
        }
    }
}

pub static IMPLEMENTED_SERVICES: &[&str] = &["chat"];
