use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, read_dir},
    io,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use eyre::{eyre, WrapErr};
use map_macro::hash_map;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

pub const DIRNAME_FILE_NAME: &str = ".dirname";
pub const LISTING_FILE_NAME: &str = ".listing";
pub static KNOWN_DIRS: LazyLock<HashMap<PathBuf, String>> = LazyLock::new(|| {
    hash_map! {
        Path::new("assets/arenas").into() => ".arena.ron".into(),
    }
});

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Listing {
    pub name: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub subdirs: BTreeMap<String, Listing>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contents: Vec<String>,
}

impl Listing {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    pub fn write(&self, mut w: impl io::Write) -> eyre::Result<()> {
        Ok(w.write_all(
            serde_json::to_string_pretty(self)
                .wrap_err("serializing listing to json")?
                .as_bytes(),
        )?)
    }
}

pub fn generate_listing(
    dir: impl AsRef<Path>,
    extension: impl AsRef<str>,
) -> eyre::Result<Listing> {
    let extension = extension.as_ref();
    let dir = dir.as_ref();
    let mut out = Listing::new(dir.to_string_lossy().into_owned());
    debug!("Generating listing for {}", dir.display());

    for entry in
        read_dir(dir).wrap_err_with(|| format!("Failed to list contents of {}", dir.display()))?
    {
        let entry = entry
            .wrap_err_with(|| format!("Failed to access directory entry in {}", dir.display()))?;
        let entry_name = entry.file_name().into_string().map_err(|s| {
            eyre!(
                "Unable to convert file name \"{}\" to string as it is not UTF-8",
                s.to_string_lossy(),
            )
        })?;
        let meta = entry
            .metadata()
            .wrap_err_with(|| format!("Failed to access metadata of {entry_name}",))?;

        if meta.is_symlink() {
            warn!("Ignoring unsupported symlink: {}", entry.path().display());
        } else if meta.is_file() {
            if entry_name.ends_with(extension) {
                out.contents.push(entry_name.clone());
            }
            if entry_name == DIRNAME_FILE_NAME {
                out.name = match fs::read_to_string(entry.path()) {
                    Ok(dirname) => dirname,
                    Err(e) => {
                        if e.kind() == io::ErrorKind::NotFound {
                            entry_name
                        } else {
                            return Err(e)?;
                        }
                    }
                };
            }
        } else if meta.is_dir() {
            let subdir = generate_listing(entry.path(), extension)
                .wrap_err_with(|| format!("Failed to generate listing of {}", dir.display()))?;
            out.subdirs.insert(entry_name, subdir);
        }
    }
    out.contents.sort();
    Ok(out)
}
