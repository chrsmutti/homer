use dirs;
use std::path::{Path, PathBuf};
use std::{env, fs, io, os::unix};

fn main() -> io::Result<()> {
    println!("Welcome to homer.");
    let dir = fs::read_dir(env::current_dir()?)?;

    let home = dir
        .filter_map(|f| f.ok())
        .find(|f| f.file_name() == "home" && f.path().is_dir());
    if home.is_none() {
        eprintln!("No 'home' dir found in current directory.");
        return Ok(());
    }

    let dir = home.map(|e| fs::read_dir(e.path())).unwrap()?;
    dir.filter_map(|f| f.ok()).for_each(|f| {
        match process_entry(&f, &dirs::home_dir().expect("Failed to get $HOME")) {
            Err(e) => eprintln!("Error while processing {:?}: {}", f.file_name(), e),
            _ => {}
        }
    });

    Ok(())
}

/// Process a generic entry, could be a dir or a file.
fn process_entry(entry: &fs::DirEntry, parent_path: &Path) -> io::Result<()> {
    if entry.path().is_dir() {
        process_dir(entry, parent_path)
    } else {
        process_file(entry, parent_path)
    }
}

/// Process a dir entry in home directory.
///
/// The files inside the entry directory will be processed.
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * Regular file exists at target location.
fn process_dir(entry: &fs::DirEntry, parent_path: &Path) -> io::Result<()> {
    let mut buf = PathBuf::new();
    buf.push(parent_path);
    buf.push(Path::new(
        entry.path().file_name().expect("Failed to get file name."),
    ));

    println!("Checking if {:?} exists.", buf);

    match fs::metadata(&buf) {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            println!("Creating {:?}", buf);

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
        .for_each(|f| match process_entry(&f, &buf) {
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
fn process_file(entry: &fs::DirEntry, parent_path: &Path) -> io::Result<()> {
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
                    eprintln!("Symlink {:?} already points to {:?}", buf, entry.path());
                    // TODO: Add a -f (--force) option.
                    return Ok(());
                } else {
                    eprintln!("Symlink {:?} points to another path {:?}", buf, cpath);
                    // TODO: Add an --ovewrite-symlinks option.
                    return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                }
            } else {
                eprintln!(
                    concat!(
                        "Regular file {:?} already exists and it's not a symlink.\n",
                        "Use --backup to create a backup of this file before symlinking it to {:?}, ",
                        "or --force to delete the file without backup."
                    ),
                    buf,
                    entry.path()
                );

                // TODO: Add a --backup option and a --force option.
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    println!(
        "Linking {:?} to destination: {:?}",
        entry.path().as_path(),
        buf
    );
    unix::fs::symlink(entry.path(), buf)?;

    Ok(())
}
