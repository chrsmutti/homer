use failure::Fail;
use std::path::PathBuf;

#[derive(Fail, Debug)]
pub(crate) enum HomerError {
    #[fail(display = "No directory found at: {:?}.", path)]
    /// If the input directory, or default `./home` directory are not found.
    NotFound { path: PathBuf },

    #[fail(display = "No $HOME.")]
    /// If failed to retrieve the user's home directory.
    NoHome,

    #[fail(
        display = "Symlink at {:?} already exists, points to {:?}. Use --force to do the operation anyway.",
        spath, sdest
    )]
    /// If a symlink already exists at a desired `spath`, it points to `sdest`.
    AlreadyExists { spath: PathBuf, sdest: PathBuf },

    #[fail(
        display = "Regular file already exists at: {:?}. Use --backup to create a backup of this file,  or --force do delete file without backup.",
        dest
    )]
    /// If a regular file exists at a desired `path`.
    RegularFileAtDest { dest: PathBuf },
}
