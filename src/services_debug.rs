use std::io::Error;
use std::path::PathBuf;

#[inline(always)]
pub fn get_module_core(_: String) -> Result<Option<String>, Error> {
    Ok(None)
}

#[inline(always)]
pub fn get_all() -> Result<Vec<PathBuf>, Error> {
    let mut res = vec![];
    for file in std::fs::read_dir("services/").expect("Error reading services") {
        res.push(file.unwrap().path())
    }
    Ok(res)
}
