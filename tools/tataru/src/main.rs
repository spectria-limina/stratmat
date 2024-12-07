use std::{
    fs::{create_dir_all, File},
    io,
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::{eyre, WrapErr};
use tataru::*;
use tracing::info;

const DEFAULT_DIR: &str = "assets";

#[derive(Parser)]
struct Args {
    #[clap(
        help = "Directory to index. If --all is provided, this is the root assets/ directory.",
        default_value = "assets"
    )]
    directory: PathBuf,
    #[clap(help = "File extension to list. Known directories have a default.")]
    extension: Option<String>,
    #[clap(long, help = "Run on all known directories.")]
    all: bool,
    #[clap(
        long,
        help = "Output file/dir.",
        long_help = r#"Use '-' for stdout. Defaults to <directory>/.listing.

For --all, outputs will be at <out>/<directory>/.listing."#
    )]
    out: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    if args.all {
        info!("Generating all listings");
        for (dir, ext) in KNOWN_DIRS.iter() {
            let parent = args
                .out
                .as_ref()
                .map_or(Path::new(DEFAULT_DIR), |p| p.as_ref())
                .join(dir);
            create_dir_all(&parent)?;
            write_listing(
                args.directory.join(dir),
                ext,
                parent.join(LISTING_FILE_NAME),
            )?;
        }
    } else {
        let dir = args.directory;
        let ext = args
            .extension
            .or_else(|| -> Option<String> {
                if dir.starts_with(Path::new(DEFAULT_DIR)) {
                    KNOWN_DIRS.get(&dir).cloned()
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                eyre!(
                    "{} is not a known directory and no extension was provided",
                    dir.display()
                )
            })?;
        write_listing(
            &dir,
            ext,
            args.out.unwrap_or_else(|| dir.join(LISTING_FILE_NAME)),
        )?;
    }
    Ok(())
}

/// Generate a listing and write it to the provided file.
///
/// If out is `None`, write to the default location, if available. If out is `-`, write to stdout.
pub fn write_listing(
    dir: impl AsRef<Path>,
    ext: impl AsRef<str>,
    out: PathBuf,
) -> eyre::Result<()> {
    let dir = dir.as_ref();
    info!(
        "Generating listing for {} to {}",
        dir.display(),
        out.display()
    );

    let listing = generate_listing(dir, ext)
        .wrap_err_with(|| format!("Failed to generate listing of {}", dir.display()))?;

    if out == Path::new("-") {
        listing
            .write(io::stdout())
            .wrap_err("Failed to write listing to stdout")?;
    } else {
        listing
            .write(
                File::create(&out).wrap_err_with(|| format!("Failed to open {}", out.display()))?,
            )
            .wrap_err_with(|| {
                format!(
                    "Failed to write listing for {} to {}",
                    dir.display(),
                    out.display()
                )
            })?;
    }

    info!("Finished generating listing for {}", dir.display());
    Ok(())
}
