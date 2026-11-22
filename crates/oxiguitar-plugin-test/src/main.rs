// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-plugin-test`: checks plugin files against the interface.
//!
//! ```text
//! oxiguitar-plugin-test [--with NEEDED.oxp]... FILE.oxp...
//! oxiguitar-plugin-test --build PACKAGE NAME [--release]
//! ```
//!
//! `--with` puts a plugin the checked ones need beside them (the NAM
//! plugin needs the MIDI plugin, say); only the FILEs are checked. The
//! second form builds the plugin crate `PACKAGE` of the workspace in the
//! current folder as `target/oxp/NAME.oxp`, then checks it.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: oxiguitar-plugin-test [--with NEEDED.oxp]... FILE.oxp...
       oxiguitar-plugin-test --build PACKAGE NAME [--release]";

/// What the command line asks for.
#[derive(Debug, PartialEq)]
enum Args {
    Help,
    /// Check `files`, with the plugins they need in `with`.
    Check {
        files: Vec<PathBuf>,
        with: Vec<PathBuf>,
    },
    /// Build `package` as `name`.oxp, then check it.
    Build {
        package: String,
        name: String,
        release: bool,
    },
}

fn parse(args: &[String]) -> Result<Args, String> {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => Ok(Args::Help),
        Some("--build") => {
            let (Some(package), Some(name)) = (args.get(1), args.get(2)) else {
                return Err("--build needs a package and a plugin name".into());
            };
            Ok(Args::Build {
                package: package.clone(),
                name: name.clone(),
                release: args.iter().any(|a| a == "--release"),
            })
        }
        Some(_) => {
            let (mut files, mut with) = (Vec::new(), Vec::new());
            let mut rest = args.iter();
            while let Some(a) = rest.next() {
                if a == "--with" {
                    let Some(needed) = rest.next() else {
                        return Err("--with needs a plugin file".into());
                    };
                    with.push(PathBuf::from(needed));
                } else {
                    files.push(PathBuf::from(a));
                }
            }
            if files.is_empty() {
                return Err("no plugin file to check".into());
            }
            Ok(Args::Check { files, with })
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (files, with) = match parse(&args) {
        Ok(Args::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Args::Check { files, with }) => (files, with),
        Ok(Args::Build {
            package,
            name,
            release,
        }) => {
            let here = std::env::current_dir().unwrap_or_default();
            match oxiguitar_plugin_test::build(&here, &package, &name, release) {
                Ok(path) => (vec![path], Vec::new()),
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let needed: Vec<&Path> = with.iter().map(PathBuf::as_path).collect();
    let mut ok = true;
    for file in files {
        println!("{}", file.display());
        let report = oxiguitar_plugin_test::check_with(&file, &needed);
        for p in &report.passed {
            println!("  ok    {p}");
        }
        for f in &report.failed {
            println!("  FAIL  {f}");
        }
        for m in &report.messages {
            println!("  said  {m}");
        }
        println!(
            "  manifest read in {:.2} ms",
            report.manifest_time.as_secs_f64() * 1000.0
        );
        ok &= report.ok();
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Plugin files to check, with any `--with` the plugins they need,
    /// in any order; `--build` as before; mistakes are said.
    #[test]
    fn the_command_line_reads() {
        assert_eq!(parse(&args(&[])), Ok(Args::Help));
        assert_eq!(parse(&args(&["--help"])), Ok(Args::Help));
        assert_eq!(
            parse(&args(&["nam.oxp", "--with", "midi.oxp", "x.oxp"])),
            Ok(Args::Check {
                files: vec!["nam.oxp".into(), "x.oxp".into()],
                with: vec!["midi.oxp".into()],
            })
        );
        assert_eq!(
            parse(&args(&["--build", "p", "n", "--release"])),
            Ok(Args::Build {
                package: "p".into(),
                name: "n".into(),
                release: true
            })
        );
        assert!(parse(&args(&["a.oxp", "--with"])).is_err());
        assert!(parse(&args(&["--with", "midi.oxp"])).is_err());
        assert!(parse(&args(&["--build", "p"])).is_err());
    }
}
