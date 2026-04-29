use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use anyhow::{Context, Result};

use crate::paths::{exists, validate_profile_name, Paths};
use crate::state;

/// Names under `~/.claude` that should be shared across every profile.
/// Each one becomes a symlink inside the profile directory pointing back at
/// the canonical copy in `~/.claude`. Entries whose source doesn't exist are
/// skipped quietly so a fresh `~/.claude` doesn't blow up `clod new`.
const SHARED_ENTRIES: &[&str] = &[
    "skills",
    "plugins",
    "hooks",
    "agents",
    "commands",
    "CLAUDE.md",
    "settings.json",
];

pub fn init(paths: &Paths) -> Result<()> {
    fs::create_dir_all(paths.profiles_dir())
        .with_context(|| format!("creating {}", paths.profiles_dir().display()))?;
    Ok(())
}

pub fn create(paths: &Paths, name: &str) -> Result<()> {
    validate_profile_name(name)?;
    let dir = paths.profile_dir(name);
    if exists(&dir) {
        anyhow::bail!(
            "profile `{}` already exists at {}\n  remove it first with `clod rm {}`",
            name,
            dir.display(),
            name
        );
    }
    init(paths)?;
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    link_shared(paths, &dir)?;
    Ok(())
}

pub fn list(paths: &Paths) -> Result<Vec<String>> {
    let dir = paths.profiles_dir();
    if !exists(&dir) {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(s) = entry.file_name().to_str() {
                names.push(s.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

pub fn remove(paths: &Paths, name: &str, force: bool) -> Result<()> {
    validate_profile_name(name)?;

    if let Some(active) = state::read_active(paths)? {
        if active.name == name {
            anyhow::bail!(
                "refusing to remove `{}`: it is the active profile.\n  switch first: `clod switch <other>`",
                name
            );
        }
    }

    let dir = paths.profile_dir(name);
    if !exists(&dir) {
        anyhow::bail!("profile `{}` does not exist", name);
    }

    if !force && !confirm(&format!("delete profile `{}` at {}?", name, dir.display()))? {
        anyhow::bail!("aborted");
    }

    fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    use std::io::{self, BufRead, Write};
    print!("{} [y/N] ", prompt);
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn link_shared(paths: &Paths, profile_dir: &Path) -> Result<()> {
    for entry in SHARED_ENTRIES {
        let src = paths.claude_home.join(entry);
        if !exists(&src) {
            continue;
        }
        let dst = profile_dir.join(entry);
        // create_dir_all on the parent should be redundant (profile_dir exists)
        // but cheap to be defensive when entries contain `/` in the future
        symlink(&src, &dst)
            .with_context(|| format!("symlink {} -> {}", dst.display(), src.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn paths_in(tmp: &TempDir) -> Paths {
        Paths {
            clod_home: tmp.path().join(".clod"),
            claude_home: tmp.path().join(".claude"),
        }
    }

    #[test]
    fn create_links_shared_entries_that_exist() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        // seed a fake ~/.claude with two shared entries: a dir and a file
        fs::create_dir_all(paths.claude_home.join("skills")).unwrap();
        fs::write(paths.claude_home.join("CLAUDE.md"), "shared instructions").unwrap();

        create(&paths, "personal").unwrap();

        let prof = paths.profile_dir("personal");
        assert!(prof.exists());
        assert!(prof.join("skills").is_symlink());
        assert!(prof.join("CLAUDE.md").is_symlink());
        // resolves to the right target
        assert_eq!(
            fs::read_link(prof.join("skills")).unwrap(),
            paths.claude_home.join("skills")
        );

        // entries that don't exist in ~/.claude are skipped (no broken link)
        assert!(!prof.join("plugins").exists());
        assert!(!prof.join("plugins").is_symlink());
    }

    #[test]
    fn list_returns_profile_names_sorted() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();
        create(&paths, "work").unwrap();
        create(&paths, "personal").unwrap();
        let names = list(&paths).unwrap();
        assert_eq!(names, vec!["personal".to_string(), "work".to_string()]);
    }

    #[test]
    fn create_refuses_existing_profile() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();
        create(&paths, "p").unwrap();
        let err = create(&paths, "p").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }
}
