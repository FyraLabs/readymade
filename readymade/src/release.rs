use std::path::Path;

// os-release module to read system installer information
use color_eyre::eyre::Result;
use os_release::OsRelease;

pub fn release_root<P: AsRef<Path>>(root: P) -> Result<OsRelease> {
    let mut path = root.as_ref().to_path_buf();
    path.push("etc");
    path.push("os-release");
    Ok(OsRelease::new_from(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_release_root() {
        let release = release_root("/").unwrap();
        println!("{:#?}", release);
    }
}