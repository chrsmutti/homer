use dirs;
use structopt;

use std::path::{Path, PathBuf};
use std::{env, fs, io, os::unix};
use structopt::StructOpt;

#[derive(StructOpt)]
/// "Doh!" A CLI for managing your dotfiles!
struct Opt {
    #[structopt(short = "f", long = "force")]
    /// Force symlink creation even if a regular file exists at the location (deletes the old file).
    force: bool,

    #[structopt(short = "b", long = "backup")]
    /// If a regular file is found at a location that a symlink should be created, the file will
    /// be backed up to a file with the same name, with a .bkp extension. Any old backup file will
    /// be overwritten.
    backup: bool,

    #[structopt(long = "overwrite-symlinks")]
    /// If a symlink to another path exists, overwrite it.
    overwrite_symlinks: bool,

    #[structopt(short = "v", long = "verbose")]
    /// Show verbose output about the operations.
    verbose: bool,

    /// Directory containing files to link into user's home directory. (Defaults to ./home)
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let opt = Opt::from_args();

    let input = opt
        .input
        .clone()
        .ok_or(io::Error::from(io::ErrorKind::InvalidInput))
        .or::<io::Error>({
            let dir = fs::read_dir(env::current_dir()?)?;

            let home = dir
                .filter_map(|f| f.ok())
                .find(|f| f.file_name() == "home" && f.path().is_dir())
                .expect("No 'home' dir found in current directory.");

            Ok(home.path())
        })?;

    let input = fs::canonicalize(input)?;
    verbose!(opt.verbose, "Running homer using files from: {:?}.", input);
    let entries = fs::read_dir(input)?;

    entries.filter_map(|f| f.ok()).for_each(|f| {
        match process_entry(&f, &dirs::home_dir().expect("Failed to get $HOME"), &opt) {
            Err(e) => eprintln!("Error while processing {:?}: {}", f.file_name(), e),
            _ => {}
        }
    });

    Ok(())
}

/// Process a generic entry, could be a dir or a file.
fn process_entry(entry: &fs::DirEntry, parent_path: &Path, opt: &Opt) -> io::Result<()> {
    if entry.path().is_dir() {
        process_dir(entry, parent_path, opt)
    } else {
        process_file(entry, parent_path, opt)
    }
}

/// Process a dir entry in home directory.
///
/// The files inside the entry directory will be processed.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * Regular file exists at target location.
fn process_dir(entry: &fs::DirEntry, parent_path: &Path, opt: &Opt) -> io::Result<()> {
    let mut buf = PathBuf::new();
    buf.push(parent_path);
    buf.push(Path::new(
        entry.path().file_name().expect("Failed to get file name."),
    ));

    verbose!(opt.verbose, "Checking if {:?} exists.", buf);

    match fs::metadata(&buf) {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            verbose!(opt.verbose, "Creating directory at {:?}", buf);

            fs::create_dir(&buf)?
        }
        Err(e) => return Err(e),
        Ok(stat) => {
            if !stat.is_dir() {
                eprintln!(
                    "A regular file exists at {:?}, couldn't create a directory",
                    buf
                );
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
        }
    }

    let dir = fs::read_dir(entry.path())?;
    dir.filter_map(|f| f.ok())
        .for_each(|f| match process_entry(&f, &buf, opt) {
            Err(e) => println!("Error while processing {:?}: {}", f.file_name(), e),
            _ => {}
        });

    Ok(())
}

/// Process a file entry in home directory.
///
/// The file will be linked into `$HOME/{{ parent_path }}/{{ entry }}`.
/// It checks if a regular file exists at the location, or if a symlink already
/// exists.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * Same symlink or regular file already exists and no `force` option was passed.
/// * Different symlink exists at path and no `overwrite-symlinks` option was passed.
fn process_file(entry: &fs::DirEntry, parent_path: &Path, opt: &Opt) -> io::Result<()> {
    let mut buf = PathBuf::new();
    buf.push(parent_path);
    buf.push(Path::new(
        entry.path().file_name().expect("Failed to get file name"),
    ));

    // Check for already created symlinkins to avoid doing it again, and also
    // check if the files already exist in some way.
    match fs::symlink_metadata(&buf) {
        Ok(stat) => {
            if stat.file_type().is_symlink() {
                let cpath = fs::canonicalize(&buf)?;
                if cpath == entry.path() {
                    if !opt.force {
                        eprintln!(
                            concat!(
                                "Symlink {:?} already points to {:?}.\n",
                                "Use --force to do the operation anyway."
                            ),
                            buf,
                            entry.path()
                        );
                        // If no force is specified, do nothing.
                        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
                    }

                    println!(
                        "Removing old symlink at {:?}, due to --force (-f) flag",
                        &buf
                    );
                    // Remove old symlink
                    fs::remove_file(&buf)?
                } else {
                    if !opt.overwrite_symlinks {
                        eprintln!(
                            concat!(
                                "Symlink {:?} points to another path {:?}.\n",
                                "Use --overwrite-symlinks to do the operation anyway."
                            ),
                            buf, cpath
                        );
                        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                    }

                    println!(
                        "Removing old symlink at {:?}, due to --overwrite-symlinks flag",
                        &buf
                    );
                    // Remove old symlink
                    fs::remove_file(&buf)?
                }
            } else {
                if !opt.backup {
                    eprintln!(
                        concat!(
                            "Regular file {:?} already exists and it's not a symlink.\n",
                            "Use --backup to create a backup of this file before symlinking it to {:?}, ",
                            "or --force to delete the file without backup."
                        ),
                        buf,
                        entry.path()
                    );
                    return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                }

                let bpath = buf.with_extension("bkp");
                println!(
                    "Backing up {:?} to {:?} due to --backup (-b) flag.",
                    buf, bpath
                );
                fs::rename(&buf, bpath)?;
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    verbose!(
        opt.verbose,
        "Linking {:?} to destination: {:?}",
        entry.path().as_path(),
        buf
    );
    unix::fs::symlink(entry.path(), buf)?;

    Ok(())
}

#[macro_export]
macro_rules! verbose {
    ($should:expr, $($arg:tt)*) => {{
        if $should {
            println!($($arg)*);
        }
    }};
}
