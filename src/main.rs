use std::fmt::Display;
use std::io::Write;
use std::path::PathBuf;
use std::{fs, io, os::unix};

use structopt::StructOpt;

use anyhow::{anyhow, Context, Result};

/// "Doh!" A CLI for managing your dotfiles!
#[derive(StructOpt)]
struct Opt {
    #[structopt(short, long)]
    force: bool,

    #[structopt(long = "no-backup")]
    no_backup: bool,

    /// Directory containing files to link into user's home directory.
    #[structopt(short, long, parse(from_os_str), default_value = "./home")]
    input: PathBuf,

    #[structopt(short, long, parse(from_os_str), env = "HOME")]
    output: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    let input = fs::canonicalize(&opt.input);
    if input.is_err() {
        eprintln!("Directory not found {:?}", &opt.input);
        return;
    }

    let input = input.unwrap();
    if !input.is_dir() {
        eprintln!("{:?} is not a directory", &opt.input);
        return;
    }

    let output = fs::canonicalize(&opt.output);
    if output.is_err() {
        eprintln!("Directory not found {:?}", &opt.output);
        return;
    }

    let output = output.unwrap();
    if !output.is_dir() {
        eprintln!("{:?} is not a directory", &opt.output);
        return;
    }

    let plan = Plan::new(&input, &output, !opt.no_backup);
    if plan.is_err() {
        eprintln!("{}", plan.unwrap_err());
        return;
    }

    let plan = &plan.unwrap();
    if plan.is_empty() {
        return;
    }

    println!("The following actions will be performed: \n");
    if let Err(e) = plan.show() {
        eprintln!("{}", e);
        return;
    }

    if !opt.force {
        print!("\nPerform this actions? (y/N) ");
        if io::stdout().flush().is_err() {
            eprintln!("Failed to flush STDOUT");
            return;
        }

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("Failed to read from STDIN");
            return;
        }

        if input.trim().to_lowercase() != "y" {
            return;
        }
    }

    if let Err(e) = plan.execute() {
        eprintln!("{}", e);
        return;
    }
}

#[derive(Debug, PartialEq)]
enum Plan {
    Noop {
        path: PathBuf,
        dest: PathBuf,
        children: Vec<Plan>,
    },

    Link {
        path: PathBuf,
        dest: PathBuf,
        replace: bool,
        backup: bool,
    },
}

impl Plan {
    fn new(path: &PathBuf, dest: &PathBuf, backup: bool) -> Result<Plan> {
        if !path.exists() {
            return Err(anyhow!("{:?} does not exist", path));
        }

        let mut children = Vec::new();
        if path.is_dir() {
            let entries: Vec<_> = fs::read_dir(path)
                .context(format!("could not read {:?}", path))?
                .collect();

            for entry in entries {
                let entry = entry?;
                let dest = dest.join(
                    entry
                        .path()
                        .strip_prefix(path)
                        .expect("path to be root of file"),
                );

                children.push(Plan::new(&entry.path(), &dest, backup)?);
            }
        }

        let dir_equality = dest.exists() && path.is_dir() && dest.is_dir();
        let file_equality = dest.exists() & path.is_file()
            && path.canonicalize().expect("path is always valid")
                == dest.canonicalize().expect("dest is always valid");

        if dir_equality || file_equality {
            Ok(Plan::Noop {
                path: path.into(),
                dest: dest.into(),
                children,
            })
        } else {
            Ok(Plan::Link {
                backup,
                path: path.into(),
                dest: dest.into(),
                replace: dest.exists(),
            })
        }
    }

    fn is_empty(self: &Self) -> bool {
        match self {
            Plan::Noop { children, .. } => children.iter().all(Plan::is_empty),
            _ => false,
        }
    }

    fn show(self: &Self) -> Result<()> {
        match self {
            Plan::Noop { children, .. } => children.iter().try_for_each(Plan::show),
            _ => {
                println!("{}", self);
                Ok(())
            }
        }
    }

    fn execute(self: &Self) -> Result<()> {
        match self {
            Plan::Link {
                path,
                dest,
                replace,
                backup,
            } => {
                if *replace && *backup {
                    fs::rename(dest, dest.with_extension("bkp"))?;
                } else if *replace {
                    fs::remove_file(dest)?;
                }

                // NOTE: This makes the binary unix-only ¯\_(ツ)_/¯.
                unix::fs::symlink(path, dest)?;
                Ok(())
            }
            Plan::Noop { children, .. } => children.iter().try_for_each(Plan::execute),
        }
    }
}

impl Display for Plan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Plan::Link {
                path,
                dest,
                replace,
                backup,
            } => {
                if *replace && *backup {
                    writeln!(f, "\t Move: {:?} -> {:?}", dest, dest.with_extension("bkp"))?
                } else if *replace {
                    writeln!(f, "\t Delete: {:?}", dest)?
                }

                write!(f, "\t Symlink: {:?} -> {:?}", dest, path)
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_input() {
        let plan = Plan::new(
            &"./__test__/life".into(),
            &"./__test__/output".into(),
            false,
        );
        assert!(plan.is_err(), "input path should not exist");
    }
    #[test]
    fn missing_output() {
        let path: PathBuf = "./__test__/simple".into();
        let dest: PathBuf = "./__test__/life".into();
        let plan = Plan::new(&path, &dest, false);

        match plan {
            Ok(Plan::Link {
                path: p,
                dest: d,
                backup,
                replace,
            }) => {
                assert_eq!(p, path);
                assert_eq!(d, dest);
                assert_eq!(backup, false);
                assert_eq!(replace, false);
            }
            _ => panic!("plan should be to link folder"),
        }
    }

    #[test]
    fn simple() {
        let path: PathBuf = "./__test__/simple".into();
        let dest: PathBuf = "./__test__/output".into();

        let expected = Plan::Noop {
            path: path.clone(),
            dest: dest.clone(),
            children: vec![Plan::Link {
                path: path.join("file"),
                dest: dest.join("file"),
                backup: false,
                replace: false,
            }],
        };

        let plan = Plan::new(&path, &dest, false);
        assert!(plan.is_ok(), "everything should be fine");
        assert_eq!(plan.unwrap(), expected);
    }

    #[test]
    fn idempotent() {
        let path: PathBuf = "./__test__/idempotent".into();
        let dest: PathBuf = "./__test__/output".into();

        let expected = Plan::Noop {
            path: path.clone(),
            dest: dest.clone(),
            children: vec![Plan::Noop {
                path: path.join("linked"),
                dest: dest.join("linked"),
                children: vec![],
            }],
        };

        let plan = Plan::new(&path, &dest, false);
        assert!(plan.is_ok(), "everything should be fine");
        assert_eq!(plan.unwrap(), expected);
    }

    #[test]
    fn replace() {
        let path: PathBuf = "./__test__/replace".into();
        let dest: PathBuf = "./__test__/output".into();

        let expected = Plan::Noop {
            path: path.clone(),
            dest: dest.clone(),
            children: vec![Plan::Link {
                path: path.join("replaceable"),
                dest: dest.join("replaceable"),
                backup: false,
                replace: true,
            }],
        };

        let plan = Plan::new(&path, &dest, false);
        assert!(plan.is_ok(), "everything should be fine");
        assert_eq!(plan.unwrap(), expected);
    }
}
