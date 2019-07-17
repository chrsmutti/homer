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
            Err(e) => println!("Error while processing {:?}: {}", f.file_name(), e),
            _ => {}
        }
    });

    Ok(())
}

/// Process a generic entry, could be a dir or a file.
fn process_entry(entry: &fs::DirEntry, parent_path: &Path) -> Result<(), io::Error> {
    if entry.path().is_dir() {
        process_dir(entry, parent_path)
    } else {
        process_file(entry, parent_path)
    }
}

/// Process a dir entry in home directory.
///
/// The files inside the entry directory will be processed.
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
        _ => {}
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
/// The file will be linked into `$HOME/{{ parent_path }}/{{ entry }}`
fn process_file(entry: &fs::DirEntry, parent_path: &Path) -> io::Result<()> {
    let mut buf = PathBuf::new();
    buf.push(parent_path);
    buf.push(Path::new(
        entry.path().file_name().expect("Failed to get file name"),
    ));

    println!(
        "Linking {:?} to destination: {:?}",
        entry.path().as_path(),
        buf
    );
    unix::fs::symlink(entry.path(), buf)?;

    Ok(())
}
