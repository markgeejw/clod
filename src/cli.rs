use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(name = "clod", about = "Switch between Claude Code profiles", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Override `~/.clod` (mainly for tests; also reads `$CLOD_HOME`).
    #[arg(long, global = true, value_name = "DIR", env = "CLOD_HOME")]
    pub clod_home: Option<PathBuf>,

    /// Override `~/.claude` (mainly for tests).
    #[arg(long, global = true, value_name = "DIR")]
    pub claude_home: Option<PathBuf>,

    /// Path to the `claude` binary (defaults to `claude` on PATH).
    #[arg(long, global = true, value_name = "BIN", default_value = "claude")]
    pub claude_bin: String,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create `~/.clod/` if missing.
    Init,

    /// Create a new profile and link the shared assets from `~/.claude`.
    New { name: String },

    /// List profiles. The active one is marked with `*`.
    Ls,

    /// Set the active profile.
    Switch { name: String },

    /// Print the active profile.
    Current,

    /// Delete a profile.
    Rm {
        name: String,
        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Run `claude` with the active profile. Forwards extra args.
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        claude_args: Vec<OsString>,
    },

    /// Print a shell completion script to stdout.
    ///
    /// Examples:
    ///   clod completions fish > ~/.config/fish/completions/clod.fish
    ///   clod completions bash > /usr/local/etc/bash_completion.d/clod
    ///   clod completions zsh  > ~/.zsh/completions/_clod
    Completions { shell: Shell },
}
