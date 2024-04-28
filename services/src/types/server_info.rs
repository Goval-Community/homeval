use std::{sync::LazyLock, time::Instant};

#[cfg(feature = "goval-ident")]
use serde::Serialize;

pub struct ServerInfo<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub license: &'a str,
    pub repository: &'a str,
    pub description: &'a str,

    pub start_time: &'a LazyLock<Instant>,

    // TODO: make this &[&str] (is this easily possible?)
    pub authors: &'a LazyLock<Vec<&'a str>>,
}

#[cfg(feature = "goval-ident")]
#[derive(Serialize, Debug)]
pub struct ServerInfoSerialize<'a> {
    pub server: &'a str,
    pub version: &'a str,
    pub license: &'a str,
    pub authors: &'a [&'a str],
    pub repository: &'a str,
    pub description: &'a str,
    pub uptime: u64,
    pub services: &'a [&'a str],
}

#[cfg(feature = "goval-ident")]
impl<'a> ServerInfo<'a> {
    pub fn get_serializable(&self) -> ServerInfoSerialize<'a> {
        ServerInfoSerialize {
            server: self.name,
            version: self.version,
            license: self.license,
            authors: self.authors,
            repository: self.repository,
            description: self.description,
            uptime: self.start_time.elapsed().as_secs(),
            services: crate::IMPLEMENTED_SERVICES,
        }
    }
}
