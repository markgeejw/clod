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

    /// Run with this profile, overriding `$CLOD_PROFILE` and `~/.clod/active`
    /// for this invocation. Only consulted when launching `claude`.
    #[arg(long, global = true, value_name = "NAME")]
    pub profile: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create `~/.clod/` if missing.
    Init,

    /// Create a new profile and link the shared assets from `~/.claude`.
    New {
        name: String,
        /// Also link this profile's session history (`projects/`, `history.jsonl`)
        /// to a shared location so other share-history profiles can `--resume`
        /// the same sessions. Useful when switching profiles after hitting an
        /// account limit.
        #[arg(long, short = 'S')]
        share_history: bool,
    },

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

    /// Share session history (`projects/`, `history.jsonl`) for an existing
    /// profile by symlinking those entries into `~/.clod/shared/`.
    ///
    /// If the shared store is empty, this profile's history seeds it. If the
    /// shared store already has data, this profile's history is *merged*
    /// into it (sessions are keyed by UUID so collisions are extremely
    /// unlikely; `history.jsonl` is appended). `--force` is only needed if
    /// a true file-level collision occurs.
    ShareHistory {
        name: String,
        /// On a file-level collision during merge, overwrite the shared copy
        /// with the profile's copy. Rarely needed.
        #[arg(long)]
        force: bool,
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
