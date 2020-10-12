use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::{fs, io, os::unix};

use anyhow::{anyhow, Context, Result};
use crossterm::execute;
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetForegroundColor};
use structopt::StructOpt;

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

    /// Directory containing scripts that will be run after the plan is completed.
    /// If force flag is passed, no confirmation prompt will be shown.
    #[structopt(long, parse(from_os_str))]
    scripts: Option<PathBuf>,

    /// Directory containing files to link into user's home directory.
    #[structopt(short, long, parse(from_os_str), default_value = "./home")]
    input: PathBuf,

    /// Directory the files will be linked to, defaults to $HOME.
    #[structopt(short, long, parse(from_os_str), env = "HOME")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    run_linking(opt.input, opt.output, !opt.no_backup, opt.force)?;

    if let Some(scripts) = opt.scripts {
        run_scripts(scripts, opt.force)?;
    }

    Ok(())
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
    plan.show()?;

    if !force {
        // User was prompted, but did not accept the plan.
        if !prompt_user()? {
            return Ok(());
        }
    }

    plan.execute()?;
    Ok(())
}

/// Create and execute and action plan for scripts inside a directory.
///
/// All scripts that are inside the directory will be run, but it will not recurse
/// inside directories folder looking for scripts. The `force` flag disable user
/// confirmation prompt by auto-accepting the plan.
fn run_scripts(path: PathBuf, force: bool) -> Result<()> {
    let path = canonicalize_dir(path)?;
    let entries = fs::read_dir(&path).context(format!("Could not read {:?}", &path))?;

    // Get all files inside the scripts directory, but do not recurse further
    // into it's directories.
    let mut scripts = Vec::new();
    for entry in entries {
        let script = entry?.path();

        if script.is_file() {
            scripts.push(script);
        }
    }

    if scripts.is_empty() {
        return Ok(());
    }

    // Skip one line after the linking output.
    println!();
    for script in &scripts {
        // Show scripts that will execute, formatted.
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Green),
            SetAttribute(Attribute::Bold),
            Print("- run: "),
            SetAttribute(Attribute::Reset),
            SetForegroundColor(Color::Green),
            Print(format!("{}", script.display())),
            Print("\n"),
            SetForegroundColor(Color::Reset),
        )?;
    }

    if !force {
        // User was prompted, but did not accept the plan.
        if !prompt_user()? {
            return Ok(());
        }
    }

    // Spawn `process::Command` for the scripts inside the directory.
    for script in &scripts {
        Command::new(script)
            .spawn()
            .context(format!("Failed to execute {:?}", script))?
            .wait()?;
    }

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
    execute!(
        io::stdout(),
        Print("\n"),
        Print("Perform these actions? "),
        SetAttribute(Attribute::Dim),
        Print("(y/N) "),
        SetAttribute(Attribute::Reset),
    )?;

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
                .context(format!("Could not read {:?}", path))?
                .collect();

            for entry in entries {
                let entry = entry?;
                let dest = dest.join(entry.path().strip_prefix(path)?);

                children.push(Plan::new(&entry.path(), &dest, backup)?);
            }
        }

        // Check `path` and `dest` equality when both are directories.
        let dir_equality = dest.exists() && path.is_dir() && dest.is_dir();

        // When dealing with files, they will be equal if their `path`s canonicalized
        // are equal, meaning that `dest` is a link to `path`.
        let file_equality =
            dest.exists() & path.is_file() && path.canonicalize()? == dest.canonicalize()?;

        // If they're equal, no action is needed.
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
    fn is_empty(&self) -> bool {
        match self {
            Plan::Noop { children, .. } => children.iter().all(Plan::is_empty),
            _ => false,
        }
    }

    /// Execute the current plan.
    /// This will modify the disk. This function is unix-only.
    ///
    /// When dealing with `Plan::Link`, we need to be careful about replacing blocking
    /// files in the destination directory, they can be backed-up to a safe location or
    /// deleted from dist. This will recurse and call `Plan::execute` on plan's children.
    fn execute(&self) -> Result<()> {
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
                    if dest.is_dir() {
                        fs::remove_dir_all(dest)?;
                    } else {
                        fs::remove_file(dest)?;
                    }
                }

                // NOTE: This makes the binary unix-only ¯\_(ツ)_/¯.
                unix::fs::symlink(path, dest)?;
                Ok(())
            }
            Plan::Noop { children, .. } => children.iter().try_for_each(Plan::execute),
        }
    }

    /// Show the plan, recursing and displaying all it's children aswell.
    fn show(self: &Self) -> Result<()> {
        match self {
            Plan::Link {
                path,
                dest,
                replace,
                backup,
            } => {
                if *replace && *backup {
                    // Show backup formatted text
                    execute!(
                        io::stdout(),
                        SetForegroundColor(Color::Magenta),
                        SetAttribute(Attribute::Bold),
                        Print("~ mv: "),
                        SetAttribute(Attribute::Reset),
                        SetForegroundColor(Color::Magenta),
                        Print(format!(
                            "{} -> {}",
                            dest.display(),
                            dest.with_extension("bkp").display()
                        )),
                        Print("\n"),
                        SetForegroundColor(Color::Reset),
                    )?;
                } else if *replace {
                    // Show remove formatted text
                    execute!(
                        io::stdout(),
                        SetForegroundColor(Color::Red),
                        SetAttribute(Attribute::Bold),
                        Print("- rm: "),
                        SetAttribute(Attribute::Reset),
                        SetForegroundColor(Color::Red),
                        Print(format!("{}", dest.display())),
                    )?;

                    if dest.is_dir() {
                        execute!(
                            io::stdout(),
                            SetForegroundColor(Color::Red),
                            SetAttribute(Attribute::Bold),
                            Print(format!(
                                " (this is a directory, all of it's contents will be deleted)"
                            ))
                        )?;
                    }

                    execute!(io::stdout(), Print("\n"), SetForegroundColor(Color::Reset))?;
                }

                // Show link formatted text
                execute!(
                    io::stdout(),
                    SetForegroundColor(Color::Cyan),
                    SetAttribute(Attribute::Bold),
                    Print("~ ln: "),
                    SetAttribute(Attribute::Reset),
                    SetForegroundColor(Color::Cyan),
                    Print(format!("{} -> {}", dest.display(), path.display())),
                    Print("\n"),
                    SetForegroundColor(Color::Reset),
                )?;

                Ok(())
            }
            Plan::Noop { children, .. } => children.iter().try_for_each(Plan::show),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_input() {
        let plan = Plan::new(
            &"./testdata/life".into(),
            &"./testdata/output".into(),
            false,
        );
        assert!(plan.is_err(), "input path should not exist");
    }
    #[test]
    fn missing_output() {
        let path: PathBuf = "./testdata/simple".into();
        let dest: PathBuf = "./testdata/life".into();
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
        let path: PathBuf = "./testdata/simple".into();
        let dest: PathBuf = "./testdata/output".into();

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
        let path: PathBuf = "./testdata/idempotent".into();
        let dest: PathBuf = "./testdata/output".into();

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
        let path: PathBuf = "./testdata/replace".into();
        let dest: PathBuf = "./testdata/output".into();

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

    #[test]
    fn not_folder() {
        let path: PathBuf = "./testdata/not_folder".into();
        let dest: PathBuf = "./testdata/output".into();

        let expected = Plan::Noop {
            path: path.clone(),
            dest: dest.clone(),
            children: vec![Plan::Link {
                path: path.join("not_folder"),
                dest: dest.join("not_folder"),
                backup: false,
                replace: true,
            }],
        };

        let plan = Plan::new(&path, &dest, false);
        assert!(plan.is_ok(), "everything should be fine");
        assert_eq!(plan.unwrap(), expected);
    }
}
