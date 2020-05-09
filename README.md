# homer

[![Actions Status](https://github.com/chrsmutti/homer/workflows/Rust/badge.svg)](https://github.com/chrsmutti/homer/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

"Doh!" A CLI for managing your dotfiles!

## Installation

#### Standalone

`homer` can be easily installed as an executable. Download the latest
[compiled binaries](https://github.com/chrsmutti/homer/releases) and put it
anywhere in your executable path.

#### Source

Prerequisites for building from source are:

- [Rust](https://www.rust-lang.org/tools/install)

Clone this repository and run `cargo`:

```sh
git clone github.com/chrsmutti/homer
cd homer
cargo build --release

# Optionally copy it to a dir in your $PATH.
cp target/release/homer ~/.local/bin
```

## Usage

Create a `dotfiles` directory anywhere you'd like, this directory can be a git
repo, for easy sharing and versioning. Inside it create a `home` directory.

Whenever you run `homer` inside the `dotfiles` directory, the same structure
present in the `home` directory will be created into your `$HOME` dir. Every
dir found will be created if it does not already exist, and every file will
be linked.

The resulting structure should be as follows:

```bash
$HOME
├── dotfiles
│   └── home
│       ├── dir
│       │   ├── a
│       │   └── b
│       └── file
├── dir
│   ├── a -> ~/dotfiles/dir/a
│   └── b -> ~/dotfiles/dir/b
└── file -> ~/dotfiles/file
```

An action plan will always be shown, and then the user can choose to accept the changes
or reject them (you can pass the `--force` flag to auto-accept the prompt).

```
homer 0.2.1
"Doh!" A CLI for managing your dotfiles!

USAGE:
    homer [FLAGS] [OPTIONS] --output <output>

FLAGS:
    -f, --force        Force the program to run without prompting for confirmation
    -h, --help         Prints help information
        --no-backup    Disable backup, an action plan will be created, when other files block symlink creation they will
                       be deleted instead of moved to a safe backup location
    -V, --version      Prints version information

OPTIONS:
    -i, --input <input>        Directory containing files to link into user's home directory [default: ./home]
    -o, --output <output>      Directory the files will be linked to, defaults to $HOME [env: HOME]
        --scripts <scripts>    Directory containing scripts that will be run after the plan is completed. If force flag
                               is passed, no confirmation prompt will be shown
```

### This could be a bash script!!1!

Yes.

# License

`homer` is licensed under the [MIT License](https://opensource.org/licenses/MIT).
