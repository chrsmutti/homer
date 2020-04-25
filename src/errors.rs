use failure::{self, Fail};
use std::path::PathBuf;

#[derive(Fail, Debug)]
pub(crate) enum HomerError {
    /// If the input directory, or default `./home` directory are not found.
    #[fail(display = "No directory found at: {:?}.", _0)]
    NotFound(PathBuf),

    /// If failed to retrieve the user's home directory.
    #[fail(display = "No $HOME.")]
    NoHome,

    /// If a regular file exists at a desired `path`.
    #[fail(
        display = "Another file already exists at: {:?}. Use --force or --backup.",
        _0
    )]
    Blocked(PathBuf),
}
