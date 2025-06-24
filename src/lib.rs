use std::{env, io, io::Write, path::PathBuf};

use gumdrop::Options;

use crate::{
    diff_format::ColorMode,
    error::{Error, Result},
    unused::UnusedDiagnosticKind,
};

mod cauterize;
mod diff_format;
mod error;
mod resolver;
mod unused;
mod vcs;

const SUBCOMMAND_NAME: &str = "minify";

#[derive(Debug, Options)]
struct MinifyOptions {
    #[options(help = "No output printed to stdout")]
    quiet: bool,

    #[options(help = "Package to minify", meta = "SPEC")]
    package: Vec<String>,
    #[options(no_short, help = "Minify all packages in the workspace")]
    workspace: bool,
    #[options(no_short, help = "Exclude packages from the minify", meta = "SPEC")]
    exclude: Vec<String>,

    #[options(help = "File to minify", meta = "SPEC")]
    file: Vec<String>,
    #[options(help = "Ignore files from the minify", meta = "SPEC")]
    ignore: Vec<String>,

    #[options(
        help = "specify which kinds of diagnostics to apply (all by default)",
        meta = "< FUNCTION | CONST | STATIC | STRUCT | ENUM | UNION | TYPE_ALIAS | \
                ASSOCIATED_FUNCTION | MACRO_DEFINITION >"
    )]
    kinds: Vec<UnusedDiagnosticKind>,

    #[options(no_short, help = "Apply changes instead of outputting a diff")]
    apply: bool,

    #[options(help = "Print help message")]
    help: bool,

    #[options(no_short, help = "Coloring: auto, always, never", meta = "WHEN")]
    color: ColorMode,

    #[options(no_short, help = "Path to Cargo.toml", meta = "PATH")]
    manifest_path: Option<String>,

    #[options(no_short, help = "Fix code even if the working directory is dirty")]
    allow_dirty: bool,

    #[options(no_short, help = "Fix code even if there are staged files in the VCS")]
    allow_staged: bool,

    #[options(no_short, help = "Also operate if no version control system was found")]
    allow_no_vcs: bool,
}

pub fn run() {
    // Drop the first actual argument if it is equal to our subcommand
    // (i.e. we are being called via 'cargo')
    let mut args = env::args().peekable();
    args.next();

    if args.peek().map(|s| s.as_str()) == Some(SUBCOMMAND_NAME) {
        args.next();
    }

    let mini_help = || {
        eprintln!();
        eprintln!("For more information, try '--help'");
    };

    let status_code = match execute(&args.collect::<Vec<_>>()) {
        Err(Error::Io(err)) => {
            eprintln!("IO error: {}", err);
            3
        }
        Err(Error::Utf8(err)) => {
            eprintln!("Encoding error: {}", err);
            2
        }
        Err(Error::Args(err)) => {
            eprintln!("error: {}", err);
            mini_help();
            1
        }
        Err(Error::CommandLine(err)) => {
            eprintln!("error: {}", err);
            mini_help();
            1
        }
        _ => 0,
    };

    io::stdout().flush().unwrap();

    std::process::exit(status_code);
}

pub fn execute(args: &[String]) -> Result<()> {
    let opts = MinifyOptions::parse_args_default(args)?;
    let manifest_path = opts.manifest_path.as_ref().map(PathBuf::from);
    let crate_resolution = CrateResolutionOptions::from_options(&opts)?;
    let file_resolution = FileResolutionOptions::from_options(&opts)?;

    if opts.help {
        println!("{}", MinifyOptions::usage());
    } else {
        let unused = unused::get_unused(
            manifest_path.as_deref(),
            &crate_resolution,
            &file_resolution,
            &opts.kinds,
        )?;
        let changes: Vec<_> = cauterize::process_diagnostics(unused, manifest_path.as_ref()).collect();

        if !opts.quiet {
            if changes.is_empty() {
                eprintln!("no unused code that can be minified")
            } else {
                for change in &changes {
                    diff_format::println(change, opts.color);
                }
            }
        }

        let cargo_root = resolver::get_cargo_metadata(manifest_path.as_deref())?.workspace_root;

        if opts.apply {
            use vcs::Status;
            match vcs::status(&cargo_root) {
                Status::Error(e) => {
                    eprintln!("git problem: {}", e)
                }
                Status::NoVCS if !opts.allow_no_vcs => {
                    eprintln!(
                        "no VCS found for this package and `cargo minify` can potentially perform \
                         destructive changes; if you'd like to suppress this error pass \
                         `--allow-no-vcs`"
                    );
                }
                Status::Unclean { dirty, staged }
                    if !(dirty.is_empty() || opts.allow_dirty)
                        || !(staged.is_empty() || opts.allow_staged) =>
                {
                    eprintln!("working directory contains dirty/staged files:");
                    for file in dirty {
                        eprintln!("\t{} (dirty)", file)
                    }
                    for file in staged {
                        eprintln!("\t{} (staged)", file)
                    }
                    eprintln!(
                        "please fix this or ignore this warning with --allow-dirty and/or \
                         --allow-staged"
                    );
                }
                _ => {
                    // TODO: Remove unwrap
                    cauterize::commit_changes(changes).unwrap();
                }
            }
        } else if !changes.is_empty() {
            println!("run with --apply to apply these changes")
        }
    }

    Ok(())
}

pub enum CrateResolutionOptions<'a> {
    Root,
    Workspace { exclude: &'a [String] },
    Package { packages: &'a [String] },
}

impl<'a> CrateResolutionOptions<'a> {
    fn from_options(opts: &'a MinifyOptions) -> Result<Self> {
        match (
            opts.workspace,
            !opts.package.is_empty(),
            !opts.exclude.is_empty(),
        ) {
            (true, false, true) | (true, false, false) => Ok(CrateResolutionOptions::Workspace {
                exclude: &opts.exclude,
            }),
            (false, true, false) => Ok(CrateResolutionOptions::Package {
                packages: &opts.package,
            }),
            (false, false, false) => Ok(CrateResolutionOptions::Root),
            (true, true, false) | (false, true, true) | (true, true, true) => Err(Error::Args(
                "either specify --workspace and optionally --exclude specific targets, or specify \
                 specific targets with --package",
            )),
            (false, false, true) => Err(Error::Args(
                "--exclude can only be used in conjunction with --workspace",
            )),
        }
    }
}

pub enum FileResolutionOptions<'a> {
    Only(&'a [String]),
    AllBut(&'a [String]),
}

impl<'a> FileResolutionOptions<'a> {
    fn from_options(opts: &'a MinifyOptions) -> Result<Self> {
        match (!opts.file.is_empty(), !opts.ignore.is_empty()) {
            (false, false) | (false, true) => Ok(FileResolutionOptions::AllBut(&opts.ignore)),
            (true, false) => Ok(FileResolutionOptions::Only(&opts.file)),
            (true, true) => Err(Error::Args(
                "either specify --ignore to minify all files except",
            )),
        }
    }

    pub fn is_included(&self, file_name: &str) -> bool {
        match self {
            FileResolutionOptions::Only(files) => files
                .iter()
                .any(|file| glob_match::glob_match(file, file_name)),
            FileResolutionOptions::AllBut(ignored) => ignored
                .iter()
                .all(|ignore| !glob_match::glob_match(ignore, file_name)),
        }
    }
}
