use std::{io::Error, path::PathBuf};

use include_directory::{include_directory, Dir};
static SERVICES_DIR: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/services");

#[inline(always)]
pub fn get_module_core(service: String) -> Result<Option<deno_core::FastString>, Error> {
    match SERVICES_DIR.get_file(format!("{}.js", service)) {
        Some(file) => {
            return match file.contents_utf8() {
                Some(contents) => Ok(Some(deno_core::FastString::Static(contents))),
                None => Err(Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Module: {} has None for .contents_utf8()", service),
                )),
            }
        }
        None => Err(Error::new(
            std::io::ErrorKind::NotFound,
            format!("Module: {} has None for .contents_utf8()", service),
        )),
    }
}

// TODO: compute this at compile time
#[inline(always)]
pub fn get_all() -> Result<Vec<PathBuf>, Error> {
    let mut res = vec![];
    for file in SERVICES_DIR.files() {
        res.push(file.path().to_path_buf())
    }
    Ok(res)
}
