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
│   ├── dir
│   │   ├── a
│   │   └── b
│   └── file
├── dir
│   ├── a -> ~/dotfiles/dir/a
│   └── b -> ~/dotfiles/dir/b
└── file -> ~/dotfiles/file
```

Before running it for real, you could inspect what homer will do on a run by
running:

```
homer -v --dry-run
```

You can also create a `.homerignore` that will be used to ignore certain paths
whenever `homer` is ran.

```
homer 0.1.0
Christian Mutti <chrsmutti@gmail.com>
"Doh!" A CLI for managing your dotfiles!

USAGE:
    homer [FLAGS] [OPTIONS] [input]

FLAGS:
    -b, --backup     If a regular file is found at a location that a symlink or directory should be created, the file
                     will be backed up to a file with the same name, with a .bkp extension. Any old backup file will be
                     overwritten.
        --dry-run    Do not actually change anything. Use with --verbose to se all steps.
    -f, --force      Force symlink creation even if a regular file exists at the location (deletes the old file).
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Show verbose output about the operations.

OPTIONS:
        --ignore-file <ignore_file>    File containing ignore patterns, very similar to .gitingore. [default:
                                       .homerignore]

ARGS:
    <input>    Directory containing files to link into user's home directory. [default: ./home]
```

### This could be a bash script!!1!

Yes.

# License

`homer` is licensed under the [MIT License](https://opensource.org/licenses/MIT).
