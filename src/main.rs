use std::fs::{read_dir, DirEntry};
use std::io;
use std::path::{Path, PathBuf};

fn main() -> Result<(), io::Error> {
    println!("Welcome to homer.");
    let dir = read_dir(Path::new("."))?;

    let home = dir
        .filter_map(|f| f.ok())
        .find(|f| f.file_name() == "home" && f.path().is_dir());
    if home.is_none() {
        eprintln!("No 'home' dir found in current directory.");
        return Ok(());
    }

    let dir = home.map(|e| read_dir(e.path())).unwrap()?;
    dir.filter_map(|f| f.ok())
        .for_each(|f| match process_entry(&f, &Path::new("~")) {
            Err(e) => println!(
                "Error while processing {}: {}",
                f.file_name().to_str().unwrap(),
                e
            ),
            _ => {}
        });

    Ok(())
}

fn process_entry(entry: &DirEntry, parent_path: &Path) -> Result<(), io::Error> {
    if entry.path().is_dir() {
        process_dir(entry, parent_path)
    } else {
        process_file(entry, parent_path)
    }
}

fn process_dir(entry: &DirEntry, parent_path: &Path) -> Result<(), io::Error> {
    println!(
        "{} is a dir, should recursively process entries.",
        entry.path().as_os_str().to_str().unwrap()
    );

    let mut buf = PathBuf::new();
    buf.push(parent_path);
    buf.push(Path::new(&entry.path().file_name().unwrap()));

    let dir = read_dir(entry.path())?;
    dir.filter_map(|f| f.ok())
        .for_each(|f| match process_entry(&f, Path::new(&buf)) {
            Err(e) => println!(
                "Error while processing {}: {}",
                f.file_name().to_str().unwrap(),
                e
            ),
            _ => {}
        });

    Ok(())
}

fn process_file(entry: &DirEntry, parent_path: &Path) -> Result<(), io::Error> {
    let mut buf = PathBuf::new();
    buf.push(parent_path);
    buf.push(Path::new(&entry.path().file_name().unwrap()));

    println!(
        "{} is a file, link or copy it to destination {}",
        entry.path().as_os_str().to_str().unwrap(),
        buf.to_str().unwrap()
    );

    Ok(())
}
