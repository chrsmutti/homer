use dirs;
use failure;
use structopt;

use failure::{bail, Fail};
use std::path::{Path, PathBuf};
use std::{env, fs, io, os::unix};
use structopt::StructOpt;

#[derive(StructOpt)]
/// "Doh!" A CLI for managing your dotfiles!
struct Opt {
    #[structopt(short = "v", long = "verbose")]
    /// Show verbose output about the operations.
    verbose: bool,

    #[structopt(short = "f", long = "force")]
    /// Force symlink creation even if a regular file exists at the location
    /// (deletes the old file).
    force: bool,

    #[structopt(short = "b", long = "backup")]
    /// If a regular file is found at a location that a symlink or directory
    /// should be created, the file will be backed up to a file with the same
    /// name, with a .bkp extension. Any old backup file will be overwritten.
    backup: bool,

    /// Directory containing files to link into user's home directory.
    /// (defaults to ./home)
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

#[derive(Fail, Debug)]
enum HomerError {
    #[fail(display = "No directory found at: {:?}", path)]
    /// If the input directory, or default `./home` directory are not found.
    NotFound { path: PathBuf },

    #[fail(display = "No $HOME")]
    /// If failed to retrieve the user's home directory.
    NoHome,

    #[fail(
        display = "Symlink at {:?} already exists, points to {:?}.
                   Use --force to do the operation anyway.",
        spath, sdest
    )]
    /// If a symlink already exists at a desired `spath`, it points to `sdest`.
    AlreadyExists { spath: PathBuf, sdest: PathBuf },

    #[fail(
        display = "Regular file already exists at: {:?}.
                   Use --backup to create a backup of this file, 
                   or --force do delete file without backup.",
        dest
    )]
    /// If a regular file exists at a desired `path`.
    RegularFileAtDest { dest: PathBuf },
}

type Result<T> = std::result::Result<T, failure::Error>;

#[macro_use]
mod homer {
    /// A simple verbose macro, uses `println!` if first argument evaluates to `true`.
    macro_rules! verbose {
        ($should:expr, $($arg:tt)*) => {{
            if $should {
                println!($($arg)*);
            }
        }};
    }
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let input = opt.input.clone().unwrap_or({
        let mut buf = env::current_dir()?;
        buf.push("home");

        buf
    });

    if !input.is_dir() {
        bail!(HomerError::NotFound { path: input });
    }

    let home = dirs::home_dir().ok_or(HomerError::NoHome)?;
    let input = fs::canonicalize(input)?;
    verbose!(opt.verbose, "Running homer using files from: {:?}.", input);

    process_dir(&input, &home, &opt)?;

    Ok(())
}

/// Process a generic entry, could be a dir or a file.
fn process_entry(entry: &fs::DirEntry, parent_path: &Path, opt: &Opt) -> Result<()> {
    let mut dest = parent_path.to_path_buf();
    // NOTE: as we're working with `DirEntry`s a None is impossible here.
    dest.push(entry.path().file_name().expect("failed to get file_name."));

    if entry.path().is_dir() {
        process_dir(&entry.path(), &dest, opt)
    } else {
        process_file(&entry.path(), &dest, opt)
    }
}

/// Process a dir entry in home directory.
///
/// The files inside `path` will be processed.
/// Creates the directory at `dest` if it does not exist. If it does exist and
/// is not a directory, will return a error unless `force` or `backup` options
/// are present.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * A symlink or a regular file already exists at `path` and no `force` option was passed.
/// * A regular file already exists at `path` and no `backup` option was passed.
fn process_dir(path: &PathBuf, dest: &PathBuf, opt: &Opt) -> Result<()> {
    verbose!(opt.verbose, "Checking if {:?} exists.", dest);
    match fs::metadata(&dest) {
        Ok(stat) => {
            if !stat.is_dir() {
                if stat.file_type().is_symlink() {
                    handle_symlink_at_dest(dest, opt)?;
                } else {
                    handle_regular_file_at_dest(dest, opt)?;
                }

                verbose!(opt.verbose, "Creating directory at {:?}", dest);
                fs::create_dir(&dest)?;
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            verbose!(opt.verbose, "Creating directory at {:?}", dest);
            fs::create_dir(&dest)?;
        }
        Err(e) => bail!(e),
    }

    fs::read_dir(path)?
        .filter_map(|f| f.ok())
        .for_each(|f| match process_entry(&f, &dest, &opt) {
            Err(e) => eprintln!("{}", e),
            _ => {}
        });

    Ok(())
}

/// Process a file entry in home directory.
///
/// The file will be linked into `dest`.
/// It checks if a regular file exists at the location, or if a symlink already
/// exists.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * A symlink or a regular file already exists at `path` and no `force` option was passed.
/// * A regular file already exists at `path` and no `backup` option was passed.
fn process_file(path: &PathBuf, dest: &PathBuf, opt: &Opt) -> Result<()> {
    match fs::symlink_metadata(dest) {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        Ok(stat) => {
            if stat.file_type().is_symlink() {
                handle_symlink_at_dest(dest, opt)?;
            } else {
                handle_regular_file_at_dest(dest, opt)?;
            }
        }
        Err(e) => bail!(e),
    }

    verbose!(opt.verbose, "Linking {:?} to destination: {:?}", path, dest);
    unix::fs::symlink(path, dest)?; // This makes the binary unix-only ¯\_(ツ)_/¯.

    Ok(())
}

/// Handle existing symlink at desired `dest`.
///
/// It removes the symlink at `dest` if the `force` option is present.
fn handle_symlink_at_dest(dest: &PathBuf, opt: &Opt) -> Result<()> {
    let canonical_dest = fs::canonicalize(dest)?;
    if !opt.force {
        bail!(HomerError::AlreadyExists {
            spath: dest.into(),
            sdest: canonical_dest,
        });
    }

    println!("Removing symlink at {:?}, due to --force (-f) flag.", dest);
    fs::remove_file(dest)?;

    Ok(())
}

/// Handle existing file at desired `dest`.
///
/// It removes the file at `dest` if `force` option is present, or creates a
/// `backup` file with the same name, but a `.bkp` extension added.
fn handle_regular_file_at_dest(dest: &PathBuf, opt: &Opt) -> Result<()> {
    if !opt.backup || !opt.force {
        bail!(HomerError::RegularFileAtDest { dest: dest.into() });
    }

    if opt.backup {
        let bpath = dest.with_extension("bkp");
        println!(
            "Backing up {:?} to {:?} due to --backup (-b) flag.",
            dest, bpath
        );
        fs::rename(dest, bpath)?;
    } else if opt.force {
        println!(
            "Removing regular file at {:?}, due to --force (-f) flag.",
            dest
        );
        fs::remove_file(dest)?
    }

    Ok(())
}
