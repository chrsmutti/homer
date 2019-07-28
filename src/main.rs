mod errors;

use std::fs;
use std::path::{Path, PathBuf};

use dirs;
use errors::HomerError;
use failure::bail;
use structopt;
use structopt::StructOpt;

#[derive(StructOpt)]
/// "Doh!" A CLI for managing your dotfiles!
struct Opt {
    #[structopt(short, long)]
    /// Show verbose output about the operations.
    verbose: bool,

    #[structopt(short, long)]
    /// Force symlink creation even if a regular file exists at the location
    /// (deletes the old file).
    force: bool,

    #[structopt(short, long)]
    /// If a regular file is found at a location that a symlink or directory
    /// should be created, the file will be backed up to a file with the same
    /// name, with a .bkp extension. Any old backup file will be overwritten.
    backup: bool,

    #[structopt(long = "dry-run")]
    /// Do not actually change anything. Use with --verbose to se all steps.
    dry_run: bool,

    /// Directory containing files to link into user's home directory.
    /// (defaults to ./home)
    #[structopt(parse(from_os_str), default_value = "home")]
    input: PathBuf,
}

/// Standard result type.
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

fn main() {
    if let Err(e) = run(Opt::from_args()) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

/// Returns `Err(..)` upon fatal errors. Otherwise, returns `Ok(())`.
fn run(opt: Opt) -> Result<()> {
    let input = fs::canonicalize(&opt.input)?;
    if !input.is_dir() {
        bail!(HomerError::NotFound { path: input });
    }

    let home = dirs::home_dir().ok_or(HomerError::NoHome)?;
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

                if !opt.dry_run {
                    fs::create_dir(&dest)?;
                }
            }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
            verbose!(opt.verbose, "Creating directory at {:?}", dest);
            if !opt.dry_run {
                fs::create_dir(&dest)?;
            }
        }
        Err(e) => bail!(e),
    }

    fs::read_dir(path)?.filter_map(|f| f.ok()).for_each(|f| {
        if let Err(e) = process_entry(&f, &dest, &opt) {
            eprintln!("{}", e)
        }
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
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Ok(stat) => {
            if stat.file_type().is_symlink() {
                handle_symlink_at_dest(dest, opt)?;
            } else {
                handle_regular_file_at_dest(dest, opt)?;
            }
        }
        Err(e) => bail!(e),
    }

    verbose!(
        opt.verbose,
        "Creating link at: {:?}, points to: {:?}",
        dest,
        path
    );
    if !opt.dry_run {
        std::os::unix::fs::symlink(path, dest)?; // This makes the binary unix-only ¯\_(ツ)_/¯.
    }

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
    if !opt.dry_run {
        fs::remove_file(dest)?;
    }

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
        if !opt.dry_run {
            fs::rename(dest, bpath)?;
        }
    } else if opt.force {
        println!(
            "Removing regular file at {:?}, due to --force (-f) flag.",
            dest
        );
        if !opt.dry_run {
            fs::remove_file(dest)?
        }
    }

    Ok(())
}
