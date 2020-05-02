use std::fmt::Display;
use std::io::Write;
use std::path::PathBuf;
use std::{fs, io, os::unix, process};

use structopt::StructOpt;

use anyhow::{anyhow, Context, Result};

/// "Doh!" A CLI for managing your dotfiles!
#[derive(StructOpt)]
struct Opt {
    /// Force the program to run without prompting for confirmation.
    #[structopt(short, long)]
    force: bool,

    /// Disable backup, an action plan will be created, when other files block
    /// symlink creation they will be deleted instead of moved to a safe backup
    /// location.
    #[structopt(long = "no-backup")]
    no_backup: bool,

    /// Directory containing files to link into user's home directory.
    #[structopt(short, long, parse(from_os_str), default_value = "./home")]
    input: PathBuf,

    /// Directory the files will be linked to, defaults to $HOME.
    #[structopt(short, long, parse(from_os_str), env = "HOME")]
    output: PathBuf,
}

fn main() {
    let opt = Opt::from_args();

    if let Err(e) = run_linking(opt.input, opt.output, !opt.no_backup, opt.force) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

/// Create and execute action plan for the linking process.
///
/// `input` will be used to check for files and directories that will be linked
/// into `output`. Both `input` and `output` should be valid directories.
///
/// By passing the `backup` flag, files that would block symlink creation are moved
/// to the same directory with a `bkp` extension, otherwise they will be deleted. The
/// `force` flag disable user confirmation prompt by auto-accepting the plan.
fn run_linking(input: PathBuf, output: PathBuf, backup: bool, force: bool) -> Result<()> {
    let input = canonicalize_dir(input)?;
    let output = canonicalize_dir(output)?;

    let plan = Plan::new(&input, &output, backup)?;
    if plan.is_empty() {
        return Ok(());
    }

    // Show the plan to the user, this substitute a verbose option, as it's always shown.
    println!("The following actions will be performed: \n");
    plan.show();

    if !force {
        // User was prompted, but did not accept the plan.
        if !prompt_user()? {
            return Ok(());
        }
    }

    plan.execute()?;
    Ok(())
}

/// Canonicalize a directory path by calling `fs::canonicalize` and failing if
/// the result path is not a directory.
fn canonicalize_dir(path: PathBuf) -> Result<PathBuf> {
    let input = fs::canonicalize(&path).context(format!("{:?} not found", &path))?;

    if !input.is_dir() {
        return Err(anyhow!(format!("{:?} is not a directory", path)));
    }

    Ok(input)
}

/// Prompt user with a confirmation message and wait for the response.
/// The result will be `true` if the user accepts the prompt.
fn prompt_user() -> Result<bool> {
    print!("\nPerform this actions? (y/N) ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    return Ok(input.trim().to_lowercase() == "y");
}

/// Action plan for linking files into a destination directory.
/// There are two variants, `Plan::Noop` and `Plan::Link`.
#[derive(Debug, PartialEq)]
enum Plan {
    /// `Plan::Noop` denotes a action that does not need to be executed,
    /// it usually refers to directories that already exists on the destination
    /// path.
    Noop {
        path: PathBuf,
        dest: PathBuf,
        children: Vec<Plan>,
    },

    /// `Plan::Link` denotes a symlinking action, it can refer to a file or a
    /// directory. The `replace` flag on it is set to `true` if there is already
    /// an existing file or directory on the destination path, depending on the
    /// value of the `backup` flag, this existing file should be moved to a safe
    /// location or deleted from disk.
    Link {
        path: PathBuf,
        dest: PathBuf,
        replace: bool,
        backup: bool,
    },
}

impl Plan {
    /// Create an action plan based on a input `path` and a destination. This will
    /// recurse inside the provided directories for other directories and files to
    /// be added to the action plan.
    fn new(path: &PathBuf, dest: &PathBuf, backup: bool) -> Result<Plan> {
        if !path.exists() {
            return Err(anyhow!("{:?} does not exist", path));
        }

        // When the current path denotes a directory, we should recurse into
        // it's entries and add them to the action plan accordingly.
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

        // Check `path` and `dest` equality when both are directories.
        let dir_equality = dest.exists() && path.is_dir() && dest.is_dir();

        // When dealing with files, they will be equal if their `path`s canonicalized
        // are equal, meaning that `dest` is a link to `path`.
        let file_equality = dest.exists() & path.is_file()
            && path.canonicalize().expect("path is always valid")
                == dest.canonicalize().expect("dest is always valid");

        // If their equal, no action is needed.
        if dir_equality || file_equality {
            return Ok(Plan::Noop {
                path: path.into(),
                dest: dest.into(),
                children,
            });
        }

        Ok(Plan::Link {
            backup,
            path: path.into(),
            dest: dest.into(),
            replace: dest.exists(),
        })
    }

    /// Check if an action plan is empty, this is done by checking if the plan is `Plan::Noop`,
    /// and all it's children are also `empty`.
    fn is_empty(self: &Self) -> bool {
        match self {
            Plan::Noop { children, .. } => children.iter().all(Plan::is_empty),
            _ => false,
        }
    }

    /// Show the plan, recursing and displaying all it's children aswell.
    fn show(self: &Self) {
        match self {
            Plan::Noop { children, .. } => children.iter().for_each(Plan::show),
            _ => println!("{}", self),
        }
    }

    /// Execute the current plan.
    /// This will modify the disk. This function is unix-only.
    ///
    /// When dealing with `Plan::Link`, we need to be careful about replacing blocking
    /// files in the destination directory, they can be backed-up to a safe location or
    /// deleted from dist. This will recurse and call `Plan::execute` on plan's children.
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
