//! Argv preprocessing so unknown args fall through to `claude`.
//!
//! `clod` only owns a small set of subcommands (`init`, `new`, `ls`, `switch`,
//! `current`, `rm`, `run`, `completions`, `help`) plus a few global options
//! (`--clod-home`, `--claude-home`, `--claude-bin`, `--help`, `--version`).
//!
//! Anything else is forwarded to `claude`. To make that ergonomic without
//! forcing the user to type `run --` every time, we rewrite the argv in front
//! of clap: when the first non-global token isn't one of ours, we insert
//! `run` before it. The `Run` subcommand has `trailing_var_arg` +
//! `allow_hyphen_values`, so the rest flows through unchanged.

use std::ffi::OsString;

/// Global options whose value lives in the next argv slot.
const VALUED_GLOBALS: &[&str] = &["--clod-home", "--claude-home", "--claude-bin", "--profile"];

/// Rewrite `argv` so unknown args route through the `run` subcommand.
///
/// `known_subcommands` should be built from `Cli::command().get_subcommands()`
/// at call sites; the function takes it as a parameter to keep this unit-testable.
pub fn rewrite(argv: Vec<OsString>, known_subcommands: &[String]) -> Vec<OsString> {
    if argv.is_empty() {
        return argv;
    }
    let mut out: Vec<OsString> = Vec::with_capacity(argv.len() + 1);
    out.push(argv[0].clone());

    // Consume our leading global flags so we can peek at the first "real" token.
    let mut i = 1;
    while i < argv.len() {
        match argv[i].to_str() {
            Some(s) if VALUED_GLOBALS.contains(&s) => {
                out.push(argv[i].clone());
                if i + 1 < argv.len() {
                    out.push(argv[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            Some(s)
                if VALUED_GLOBALS
                    .iter()
                    .any(|g| s.starts_with(&format!("{g}="))) =>
            {
                out.push(argv[i].clone());
                i += 1;
            }
            _ => break,
        }
    }

    if i >= argv.len() {
        return out;
    }

    let is_ours = match argv[i].to_str() {
        Some("--help" | "-h" | "--version" | "-V") => true,
        Some(s) => known_subcommands.iter().any(|k| k == s),
        None => false, // non-utf8 → not one of our subcommand names
    };
    if !is_ours {
        out.push(OsString::from("run"));
    }
    out.extend(argv[i..].iter().cloned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<String> {
        [
            "init",
            "new",
            "ls",
            "switch",
            "current",
            "rm",
            "run",
            "completions",
            "help",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn rewrite_strs(args: &[&str]) -> Vec<String> {
        let argv: Vec<OsString> = args.iter().map(|s| OsString::from(*s)).collect();
        rewrite(argv, &known())
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn bare_clod_unchanged() {
        assert_eq!(rewrite_strs(&["clod"]), vec!["clod"]);
    }

    #[test]
    fn known_subcommand_unchanged() {
        assert_eq!(
            rewrite_strs(&["clod", "switch", "work"]),
            vec!["clod", "switch", "work"]
        );
    }

    #[test]
    fn unknown_short_flag_routes_to_run() {
        assert_eq!(
            rewrite_strs(&["clod", "-p", "hi"]),
            vec!["clod", "run", "-p", "hi"]
        );
    }

    #[test]
    fn unknown_long_flag_routes_to_run() {
        assert_eq!(
            rewrite_strs(&["clod", "--print", "hi"]),
            vec!["clod", "run", "--print", "hi"]
        );
    }

    #[test]
    fn positional_routes_to_run() {
        assert_eq!(
            rewrite_strs(&["clod", "some-prompt.md"]),
            vec!["clod", "run", "some-prompt.md"]
        );
    }

    #[test]
    fn our_globals_pass_through_before_claude_args() {
        assert_eq!(
            rewrite_strs(&["clod", "--clod-home", "/tmp/c", "-p", "hi"]),
            vec!["clod", "--clod-home", "/tmp/c", "run", "-p", "hi"]
        );
    }

    #[test]
    fn equals_form_of_global_consumed() {
        assert_eq!(
            rewrite_strs(&["clod", "--clod-home=/tmp/c", "--print"]),
            vec!["clod", "--clod-home=/tmp/c", "run", "--print"]
        );
    }

    #[test]
    fn our_globals_before_known_subcommand_unchanged() {
        assert_eq!(
            rewrite_strs(&["clod", "--clod-home", "/tmp/c", "switch", "work"]),
            vec!["clod", "--clod-home", "/tmp/c", "switch", "work"]
        );
    }

    #[test]
    fn help_and_version_treated_as_ours() {
        assert_eq!(rewrite_strs(&["clod", "--help"]), vec!["clod", "--help"]);
        assert_eq!(rewrite_strs(&["clod", "-h"]), vec!["clod", "-h"]);
        assert_eq!(
            rewrite_strs(&["clod", "--version"]),
            vec!["clod", "--version"]
        );
    }

    #[test]
    fn resume_short_and_long_route_to_claude() {
        // -r / --resume are claude flags; clod must not steal them.
        assert_eq!(rewrite_strs(&["clod", "-r"]), vec!["clod", "run", "-r"]);
        assert_eq!(
            rewrite_strs(&["clod", "--resume"]),
            vec!["clod", "run", "--resume"]
        );
        assert_eq!(
            rewrite_strs(&["clod", "-r", "abc123"]),
            vec!["clod", "run", "-r", "abc123"]
        );
        assert_eq!(
            rewrite_strs(&["clod", "--clod-home", "/tmp/c", "--resume", "abc123"]),
            vec!["clod", "--clod-home", "/tmp/c", "run", "--resume", "abc123"]
        );
    }

    #[test]
    fn profile_global_consumed_before_unknown_arg() {
        assert_eq!(
            rewrite_strs(&["clod", "--profile", "work", "-p", "hi"]),
            vec!["clod", "--profile", "work", "run", "-p", "hi"]
        );
    }

    #[test]
    fn profile_global_equals_form_consumed() {
        assert_eq!(
            rewrite_strs(&["clod", "--profile=work", "--print"]),
            vec!["clod", "--profile=work", "run", "--print"]
        );
    }

    #[test]
    fn profile_global_before_known_subcommand_unchanged() {
        assert_eq!(
            rewrite_strs(&["clod", "--profile", "work", "current"]),
            vec!["clod", "--profile", "work", "current"]
        );
    }

    #[test]
    fn explicit_run_with_dash_dash_still_works() {
        assert_eq!(
            rewrite_strs(&["clod", "run", "--", "-p", "hi"]),
            vec!["clod", "run", "--", "-p", "hi"]
        );
    }

    #[test]
    fn missing_value_for_global_doesnt_panic() {
        // clap will surface the error; we just shouldn't panic or infinite loop.
        assert_eq!(
            rewrite_strs(&["clod", "--clod-home"]),
            vec!["clod", "--clod-home"]
        );
    }
}
