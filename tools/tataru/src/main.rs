use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::{ensure, eyre, WrapErr};
use tataru::*;
use tracing::info;

#[derive(Parser)]
struct Args {
    #[clap(help = "Directory to index.")]
    directory: Option<PathBuf>,
    #[clap(help = "File extension to list. Known directories have a default.")]
    extension: Option<String>,
    #[clap(long, help = "Run on all known directories.")]
    all: bool,
    #[clap(
        long,
        help = "Output file. - for stdout. Defaults to <directory>/.listing. Cannot be used with --all"
    )]
    out: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    if args.all {
        ensure!(args.out.is_none(), "--out and --all are incompatible");
        info!("Generating all listings");
        for (dir, ext) in KNOWN_DIRS.iter() {
            write_listing(dir, ext, None)?;
        }
    } else {
        let dir = args
            .directory
            .ok_or_else(|| eyre!("Must provide a directory or --all"))?;
        let ext = args
            .extension
            .or_else(|| KNOWN_DIRS.get(&dir).cloned())
            .ok_or_else(|| {
                eyre!(
                    "{} is not a known directory and no extension was provided",
                    dir.display()
                )
            })?;
        write_listing(dir, ext, None)?;
    }
    Ok(())
}

/// Generate a listing and write it to the provided file.
///
/// If out is `None`, write to the default location, if available. If out is `-`, write to stdout.
pub fn write_listing(
    dir: impl AsRef<Path>,
    ext: impl AsRef<str>,
    out: Option<PathBuf>,
) -> eyre::Result<()> {
    let dir = dir.as_ref();
    info!("Generating listing for {}", dir.display());

    let listing = generate_listing(dir, ext)
        .wrap_err_with(|| format!("Failed to generate listing of {}", dir.display()))?;

    let out = out.unwrap_or_else(|| dir.join(LISTING_FILE_NAME));
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
