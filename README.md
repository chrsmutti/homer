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
    homer [FLAGS] [input]

FLAGS:
    -b, --backup                If a regular file is found at a location that a symlink should be created, the file will
                                be backed up to a file with the same name, with a .bkp extension. Any old backup file
                                will be overwritten.
    -f, --force                 Force symlink creation even if a regular file exists at the location (deletes the old
                                file).
    -h, --help                  Prints help information
        --overwrite-symlinks    If a symlink to another path exists, overwrite it.
    -V, --version               Prints version information
    -v, --verbose               Show verbose output about the operations.

ARGS:
    <input>    Directory containing files to link into user's home directory. (Defaults to ./home)
```

### This could be a bash script!!1!

Yes.
