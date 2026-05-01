use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::paths::{exists, validate_profile_name, Paths};
use crate::state;

/// Resolve the profile to use, set `CLAUDE_CONFIG_DIR`, and exec `claude`.
/// `profile_override`, when `Some`, takes precedence over `$CLOD_PROFILE` and
/// the active file. Returns only on error — on success the current process is
/// replaced.
pub fn exec(
    paths: &Paths,
    claude_bin: &str,
    profile_override: Option<&str>,
    args: &[OsString],
) -> Result<()> {
    let name = match profile_override {
        Some(p) => {
            validate_profile_name(p)?;
            p.to_string()
        }
        None => state::resolve_active(paths)?.name,
    };
    let dir = paths.profile_dir(&name);
    if !exists(&dir) {
        anyhow::bail!(
            "profile `{}` is missing on disk ({}). create it: `clod new {}`",
            name,
            dir.display(),
            name
        );
    }
    let err = build_command(claude_bin, &dir, args).exec();
    // exec() only returns on failure
    Err(err).with_context(|| format!("failed to exec `{}`", claude_bin))
}

fn build_command(claude_bin: &str, profile_dir: &Path, args: &[OsString]) -> Command {
    let mut cmd = Command::new(claude_bin);
    cmd.env("CLAUDE_CONFIG_DIR", profile_dir);
    cmd.args(args);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_command_sets_env_and_args() {
        let dir = Path::new("/tmp/clod-test/profiles/personal");
        let args: Vec<OsString> = vec!["--print".into(), "hi".into()];
        let cmd = build_command("claude", dir, &args);

        let env: Vec<_> = cmd
            .get_envs()
            .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
            .collect();
        assert!(env
            .iter()
            .any(|(k, v)| k == "CLAUDE_CONFIG_DIR" && v == dir.as_os_str()));

        let argv: Vec<_> = cmd.get_args().collect();
        assert_eq!(argv, vec!["--print", "hi"]);
    }
}
