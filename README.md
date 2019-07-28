# homer

"Doh!" A CLI for managing your dotfiles!

## Usage

You can run `homer` from a directory containing a `home` directory, or pass in
an input directory.

- All files from the input directory will be linked into `$HOME`.
- All directories from the input directory will be created into `$HOME`.

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

Before running it for real, you could inspect what homer will do on a run by
running:

```
homer -v --dry-run
```

### This could be a bash script!!1!

Yes.
