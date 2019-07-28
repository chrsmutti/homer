mod errors;

use std::path::{Path, PathBuf};
use std::{fs, io};

use dirs;
use errors::HomerError;
use failure::bail;
use glob;
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

    /// File containing ignore patterns, very similar to .gitingore.
    #[structopt(
        parse(from_os_str),
        long = "ignore-file",
        default_value = ".homerignore"
    )]
    ignore_file: PathBuf,

    /// Directory containing files to link into user's home directory.
    #[structopt(parse(from_os_str), default_value = "./home")]
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
        eprintln!("[error] {}", e);
        std::process::exit(1);
    }
}

/// Returns `Err(..)` upon fatal errors. Otherwise, returns `Ok(())`.
fn run(opt: Opt) -> Result<()> {
    let input = fs::canonicalize(&opt.input)?;
    if !input.is_dir() {
        bail!(HomerError::NotFound(input));
    }

    let home = dirs::home_dir().ok_or(HomerError::NoHome)?;
    verbose!(opt.verbose, "Running homer using files from: {:?}.", input);

    let ignore = read_ignore(&opt.ignore_file)?;
    let walker = Walker::new(input, home, opt, ignore);
    walker.walk()
}

/// A Walker traverses the directory structure creating a plan for the run.
/// Ignore paths that match the ignore patterns.
struct Walker {
    path: PathBuf,
    dest: PathBuf,
    opt: Opt,
    ignore: Vec<glob::Pattern>,
}

#[derive(Debug)]
/// A file or directory abstraction.
enum Entry {
    /// Directory abstraction with children and destination.
    Dir {
        path: PathBuf,
        dest: PathBuf,
        children: Vec<Entry>,
    },

    /// A File abstraction, containing the symlink destination.
    File { path: PathBuf, dest: PathBuf },
}

impl Walker {
    /// Create a new Walker.
    fn new(path: PathBuf, dest: PathBuf, opt: Opt, ignore: Vec<glob::Pattern>) -> Walker {
        Walker {
            path,
            dest,
            opt,
            ignore,
        }
    }

    /// Walk through this Walker's `path`, creating the plan that will create
    /// directories and link files at this Walker's `dest`.
    fn walk(self) -> Result<()> {
        let home_dir = self.walk_dir(&self.path, &self.dest)?;

        // NOTE: `Option` here should never be `None`.
        home_dir.unwrap().process(&self.opt)
    }

    /// Walk through a directory, transforming all entries inside the directory
    /// into abstractions that will be used later when running the execution
    /// plan.
    fn walk_dir(&self, path: &PathBuf, dest: &PathBuf) -> Result<Option<Entry>> {
        let mut children: Vec<Entry> = Vec::new();
        let entries = fs::read_dir(&path)?;

        for entry in entries {
            if let Ok(entry) = entry {
                let mut entry_dest = dest.clone();
                let entry_path = entry.path();

                // NOTE: As we're working with `DirEntry`s a None is impossible here.
                entry_dest.push(entry_path.file_name().expect("failed to get file_name."));

                if self.check_ignore(entry_path.strip_prefix(&self.path)?) {
                    verbose!(self.opt.verbose, "Ignoring {:?}.", entry_path);
                    break;
                }

                verbose!(self.opt.verbose, "Processing {:?}", entry_path);
                if entry_path.is_dir() {
                    // NOTE: `Option` here should never be `None`.
                    children.push(self.walk_dir(&entry_path, &entry_dest)?.unwrap());
                } else {
                    children.push(Entry::File {
                        path: entry_path,
                        dest: entry_dest,
                    });
                }
            }
        }

        Ok(Some(Entry::Dir {
            path: path.into(),
            dest: dest.into(),
            children,
        }))
    }

    // TODO: This is a really inneficient way of doing this.
    /// Check if `path` matches a ignore pattern.
    fn check_ignore(&self, path: &Path) -> bool {
        self.ignore
            .iter()
            .any(|pattern| pattern.matches(path.to_str().expect("path is not utf-8")))
    }
}

impl Entry {
    /// Process the execution plan for the current entry.
    fn process(self, opt: &Opt) -> Result<()> {
        match self {
            Entry::Dir { dest, children, .. } => process_dir(&dest, children, opt),
            Entry::File { path, dest } => process_file(&path, &dest, opt),
        }
    }
}

/// Process the execution plan for a `Dir` Entry.
///
/// The `children` entries will be processed.
/// Creates the directory at `dest` if it does not exist. If it does exist and
/// is not a directory, will return a error unless `force` or `backup` options
/// are present.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * A symlink or a regular file already exists at `dest` and no `force` option was passed.
/// * A regular file already exists at `dest` and no `backup` option was passed.
fn process_dir(dest: &PathBuf, children: Vec<Entry>, opt: &Opt) -> Result<()> {
    match fs::metadata(dest) {
        Ok(ref stat) if !stat.is_dir() => {
            if opt.backup && !stat.file_type().is_symlink() {
                backup(dest, opt)?;
            } else if opt.force {
                force_remove(dest, opt)?;
            } else {
                bail!(HomerError::Blocked(dest.into()));
            }

            verbose!(opt.verbose, "Creating directory at {:?}", dest);
            if !opt.dry_run {
                fs::create_dir(&dest)?;
            }
        }
        Err(e) => {
            if !(e.kind() == io::ErrorKind::NotFound) {
                bail!(e)
            }
        }
        _ => verbose!(opt.verbose, "Directory at {:?} already exists.", dest),
    }

    for child in children {
        if let Err(e) = child.process(opt) {
            eprintln!("[error] {}", e);
        }
    }

    Ok(())
}

/// Process the execution plan for a `File` Entry.
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
/// * A symlink or a regular file already exists at `dest` and no `force` option was passed.
/// * A regular file already exists at `path` and no `backup` option was passed.
fn process_file(path: &PathBuf, dest: &PathBuf, opt: &Opt) -> Result<()> {
    match fs::symlink_metadata(dest) {
        Ok(ref stat) if stat.file_type().is_symlink() => {
            if opt.force {
                force_remove(dest, opt)?;
            } else {
                bail!(HomerError::Blocked(dest.into()));
            }
        }
        Ok(stat) => {
            if opt.backup && !stat.file_type().is_symlink() {
                backup(dest, opt)?;
            } else if opt.force {
                force_remove(dest, opt)?;
            } else {
                bail!(HomerError::Blocked(dest.into()));
            }
        }
        Err(e) => {
            if !(e.kind() == io::ErrorKind::NotFound) {
                bail!(e);
            }
        }
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

/// Backup the file at `path`.
///
/// Does nothing if `opt.dry_run` is active.
/// This renames the file at `path` to a file at the same location, with the
/// same name and a added prefix `.bkp`. If a `.bkp` file already exists this
/// will override it.
fn backup(path: &PathBuf, opt: &Opt) -> Result<()> {
    let bpath = path.with_extension("bkp");
    println!(
        "Backing up {:?} to {:?} due to --backup (-b) flag.",
        path, bpath
    );
    if !opt.dry_run {
        fs::rename(path, bpath)?;
    }

    Ok(())
}

/// Remove the file at `path`.
///
/// Does nothing if `opt.dry_run` is active.
fn force_remove(path: &Path, opt: &Opt) -> Result<()> {
    println!(
        "Removing regular file at {:?}, due to --force (-f) flag.",
        path
    );
    if !opt.dry_run {
        fs::remove_file(path)?
    }

    Ok(())
}

/// Read a `.homerignore` file at `path`.
///
/// Creates patterns from the lines of the file.
/// Ignores lines starting with "#".
fn read_ignore(path: &PathBuf) -> Result<Vec<glob::Pattern>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let mut patterns: Vec<glob::Pattern> = Vec::new();
    let content = fs::read_to_string(path);
    for line in content?.lines() {
        if line.starts_with("#") {
            continue;
        }

        patterns.push(glob::Pattern::new(line)?);
    }

    Ok(patterns)
}
