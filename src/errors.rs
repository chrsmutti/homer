use failure::{self, Fail};
use std::path::PathBuf;

#[derive(Fail, Debug)]
pub(crate) enum HomerError {
    #[fail(display = "No directory found at: {:?}.", _0)]
    /// If the input directory, or default `./home` directory are not found.
    NotFound(PathBuf),

    #[fail(display = "No $HOME.")]
    /// If failed to retrieve the user's home directory.
    NoHome,

    #[fail(
        display = "Another file already exists at: {:?}. Use --force or --backup.",
        _0
    )]
    /// If a regular file exists at a desired `path`.
    Blocked(PathBuf),
}
