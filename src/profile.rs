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

/// Entries in a profile that hold session history. When `share-history` is
/// enabled for a profile, these are symlinked into `~/.clod/shared/` so other
/// share-history profiles see the same data and can `--resume` each other's
/// sessions. `projects/` is the directory of conversation logs `--resume`
/// reads from; `history.jsonl` is the typed-prompt recall list.
///
/// Each tuple is `(name, kind)`. `Dir` entries are created as empty
/// directories on first share; `File` entries as empty files.
const HISTORY_ENTRIES: &[(&str, EntryKind)] = &[
    ("projects", EntryKind::Dir),
    ("history.jsonl", EntryKind::File),
];

#[derive(Copy, Clone)]
enum EntryKind {
    Dir,
    File,
}

pub fn init(paths: &Paths) -> Result<()> {
    fs::create_dir_all(paths.profiles_dir())
        .with_context(|| format!("creating {}", paths.profiles_dir().display()))?;
    Ok(())
}

pub fn create(paths: &Paths, name: &str, share_history: bool) -> Result<()> {
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
    if share_history {
        share_history_for(paths, name, false)?;
    }
    Ok(())
}

/// Convert a profile to share session history with other share-history
/// profiles. Symlinks each entry in `HISTORY_ENTRIES` from the profile dir
/// into `~/.clod/shared/`.
///
/// When both sides hold data, they're merged rather than clobbered:
/// directories are walked and individual files moved into the shared store
/// (session UUIDs make filename collisions astronomically unlikely);
/// `history.jsonl`-style append-only logs are concatenated. The only thing
/// that can fail a merge is a true file-level collision, which `--force`
/// resolves by overwriting the shared copy with the profile's.
pub fn share_history_for(paths: &Paths, name: &str, force: bool) -> Result<()> {
    validate_profile_name(name)?;
    let profile = paths.profile_dir(name);
    if !exists(&profile) {
        anyhow::bail!("profile `{}` does not exist", name);
    }
    let shared = paths.shared_dir();
    fs::create_dir_all(&shared).with_context(|| format!("creating {}", shared.display()))?;

    for (entry, kind) in HISTORY_ENTRIES {
        let src = profile.join(entry);
        let dst = shared.join(entry);
        adopt_into_shared(&src, &dst, *kind, force)?;
    }
    Ok(())
}

fn adopt_into_shared(
    profile_path: &Path,
    shared_path: &Path,
    kind: EntryKind,
    force: bool,
) -> Result<()> {
    // Already a symlink to the shared target → nothing to do.
    if profile_path.is_symlink() {
        if let Ok(target) = fs::read_link(profile_path) {
            if target == shared_path {
                return Ok(());
            }
        }
        // Symlink to something else — replace it. Removing a symlink doesn't
        // touch the target.
        fs::remove_file(profile_path)
            .with_context(|| format!("removing existing symlink {}", profile_path.display()))?;
    }

    let shared_exists = exists(shared_path);
    let profile_has_data = exists(profile_path);

    match (shared_exists, profile_has_data) {
        (false, false) => match kind {
            EntryKind::Dir => fs::create_dir_all(shared_path)
                .with_context(|| format!("creating {}", shared_path.display()))?,
            EntryKind::File => fs::write(shared_path, b"")
                .with_context(|| format!("creating {}", shared_path.display()))?,
        },
        (false, true) => {
            // Adopt the profile's data as the canonical shared copy.
            fs::rename(profile_path, shared_path).with_context(|| {
                format!(
                    "moving {} to {}",
                    profile_path.display(),
                    shared_path.display()
                )
            })?;
        }
        (true, false) => {
            // Shared already populated and profile has no copy — just symlink.
        }
        (true, true) => match kind {
            EntryKind::Dir => {
                merge_dir_into(profile_path, shared_path, force)?;
                // profile_path should now be empty (every file moved into shared)
                fs::remove_dir_all(profile_path).with_context(|| {
                    format!("removing now-empty {}", profile_path.display())
                })?;
            }
            EntryKind::File => {
                append_file_into(profile_path, shared_path)?;
                fs::remove_file(profile_path)
                    .with_context(|| format!("removing {}", profile_path.display()))?;
            }
        },
    }

    symlink(shared_path, profile_path).with_context(|| {
        format!(
            "symlink {} -> {}",
            profile_path.display(),
            shared_path.display()
        )
    })?;
    Ok(())
}

/// Recursively move every file under `src` into the corresponding location
/// under `dst`, creating subdirectories as needed. On a true file-level
/// collision, refuse unless `force` (in which case the profile's copy
/// overwrites the shared copy).
fn merge_dir_into(src: &Path, dst: &Path, force: bool) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("creating {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry?;
        let src_child = entry.path();
        let dst_child = dst.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            merge_dir_into(&src_child, &dst_child, force)?;
            // remove the now-empty source dir; ignore the rare case where
            // a non-empty subdir survived (the recursive call would have
            // already errored in that case).
            let _ = fs::remove_dir(&src_child);
        } else {
            if dst_child.exists() {
                if !force {
                    anyhow::bail!(
                        "collision merging into shared store: {} already exists.\n  pass --force to overwrite shared with the profile's copy.",
                        dst_child.display()
                    );
                }
                if dst_child.is_dir() {
                    fs::remove_dir_all(&dst_child)
                        .with_context(|| format!("removing {}", dst_child.display()))?;
                } else {
                    fs::remove_file(&dst_child)
                        .with_context(|| format!("removing {}", dst_child.display()))?;
                }
            }
            fs::rename(&src_child, &dst_child).with_context(|| {
                format!("moving {} to {}", src_child.display(), dst_child.display())
            })?;
        }
    }
    Ok(())
}

/// Append the contents of `src` onto `dst` (used for append-only logs like
/// `history.jsonl` where order doesn't matter and both sides have value).
fn append_file_into(src: &Path, dst: &Path) -> Result<()> {
    use std::io::Write;
    let data =
        fs::read(src).with_context(|| format!("reading {}", src.display()))?;
    let mut out = fs::OpenOptions::new()
        .append(true)
        .open(dst)
        .with_context(|| format!("opening {} for append", dst.display()))?;
    out.write_all(&data)
        .with_context(|| format!("appending to {}", dst.display()))?;
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

        create(&paths, "personal", false).unwrap();

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
        create(&paths, "work", false).unwrap();
        create(&paths, "personal", false).unwrap();
        let names = list(&paths).unwrap();
        assert_eq!(names, vec!["personal".to_string(), "work".to_string()]);
    }

    #[test]
    fn create_refuses_existing_profile() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();
        create(&paths, "p", false).unwrap();
        let err = create(&paths, "p", false).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn share_history_on_fresh_profile_creates_empty_shared_entries() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();

        create(&paths, "p", true).unwrap();

        let prof = paths.profile_dir("p");
        let shared = paths.shared_dir();
        assert!(shared.join("projects").is_dir());
        assert!(shared.join("history.jsonl").is_file());
        assert!(prof.join("projects").is_symlink());
        assert!(prof.join("history.jsonl").is_symlink());
        assert_eq!(
            fs::read_link(prof.join("projects")).unwrap(),
            shared.join("projects")
        );
    }

    #[test]
    fn share_history_adopts_first_profiles_data_then_links_second() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();

        // personal is created plain, accumulates session data, then opts into sharing
        create(&paths, "personal", false).unwrap();
        let p_proj = paths.profile_dir("personal").join("projects");
        fs::create_dir_all(p_proj.join("-some-cwd")).unwrap();
        fs::write(p_proj.join("-some-cwd/abc.jsonl"), "log line").unwrap();
        fs::write(
            paths.profile_dir("personal").join("history.jsonl"),
            "prompt entry\n",
        )
        .unwrap();

        share_history_for(&paths, "personal", false).unwrap();

        // personal's data moved into shared and personal entry is now a symlink
        let shared = paths.shared_dir();
        assert!(paths.profile_dir("personal").join("projects").is_symlink());
        assert_eq!(
            fs::read_to_string(shared.join("projects/-some-cwd/abc.jsonl")).unwrap(),
            "log line"
        );
        assert_eq!(
            fs::read_to_string(shared.join("history.jsonl")).unwrap(),
            "prompt entry\n"
        );

        // work joins later: shared has data, work has none → just symlinks
        create(&paths, "work", true).unwrap();
        let w_proj = paths.profile_dir("work").join("projects");
        assert!(w_proj.is_symlink());
        // and sees personal's session through the shared link
        assert_eq!(
            fs::read_to_string(w_proj.join("-some-cwd/abc.jsonl")).unwrap(),
            "log line"
        );
    }

    #[test]
    fn share_history_merges_when_both_sides_have_data() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();

        // personal has session data and seeds the shared store
        create(&paths, "personal", false).unwrap();
        fs::create_dir_all(paths.profile_dir("personal").join("projects/-cwd-a")).unwrap();
        fs::write(
            paths.profile_dir("personal").join("projects/-cwd-a/aaa.jsonl"),
            "personal session",
        )
        .unwrap();
        fs::write(
            paths.profile_dir("personal").join("history.jsonl"),
            "personal-prompt-1\n",
        )
        .unwrap();
        share_history_for(&paths, "personal", false).unwrap();

        // work has its OWN session data — different cwd and a different UUID
        // under the same cwd; both should survive the merge
        create(&paths, "work", false).unwrap();
        fs::create_dir_all(paths.profile_dir("work").join("projects/-cwd-a")).unwrap();
        fs::write(
            paths.profile_dir("work").join("projects/-cwd-a/bbb.jsonl"),
            "work session in shared cwd",
        )
        .unwrap();
        fs::create_dir_all(paths.profile_dir("work").join("projects/-cwd-b")).unwrap();
        fs::write(
            paths.profile_dir("work").join("projects/-cwd-b/ccc.jsonl"),
            "work-only cwd",
        )
        .unwrap();
        fs::write(
            paths.profile_dir("work").join("history.jsonl"),
            "work-prompt-1\n",
        )
        .unwrap();

        share_history_for(&paths, "work", false).unwrap();

        // both profiles' sessions live in the shared store
        let shared = paths.shared_dir();
        assert_eq!(
            fs::read_to_string(shared.join("projects/-cwd-a/aaa.jsonl")).unwrap(),
            "personal session"
        );
        assert_eq!(
            fs::read_to_string(shared.join("projects/-cwd-a/bbb.jsonl")).unwrap(),
            "work session in shared cwd"
        );
        assert_eq!(
            fs::read_to_string(shared.join("projects/-cwd-b/ccc.jsonl")).unwrap(),
            "work-only cwd"
        );
        // history.jsonl was appended, not overwritten
        let history = fs::read_to_string(shared.join("history.jsonl")).unwrap();
        assert!(history.contains("personal-prompt-1"));
        assert!(history.contains("work-prompt-1"));

        // both profiles see the same merged store
        assert!(paths.profile_dir("work").join("projects").is_symlink());
        assert!(paths
            .profile_dir("personal")
            .join("projects/-cwd-b/ccc.jsonl")
            .exists());
    }

    #[test]
    fn share_history_refuses_on_uuid_collision_without_force() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();

        create(&paths, "personal", false).unwrap();
        fs::create_dir_all(paths.profile_dir("personal").join("projects/-cwd")).unwrap();
        fs::write(
            paths.profile_dir("personal").join("projects/-cwd/dup.jsonl"),
            "personal version",
        )
        .unwrap();
        share_history_for(&paths, "personal", false).unwrap();

        create(&paths, "work", false).unwrap();
        fs::create_dir_all(paths.profile_dir("work").join("projects/-cwd")).unwrap();
        fs::write(
            paths.profile_dir("work").join("projects/-cwd/dup.jsonl"),
            "work version",
        )
        .unwrap();

        let err = share_history_for(&paths, "work", false).unwrap_err();
        assert!(err.to_string().contains("collision"), "err: {err:#}");

        // --force overwrites with the profile's copy
        share_history_for(&paths, "work", true).unwrap();
        assert_eq!(
            fs::read_to_string(paths.shared_dir().join("projects/-cwd/dup.jsonl")).unwrap(),
            "work version"
        );
    }

    #[test]
    fn share_history_is_idempotent_when_already_correctly_linked() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        fs::create_dir_all(&paths.claude_home).unwrap();

        create(&paths, "p", true).unwrap();
        // calling again should be a no-op (no errors, no data churn)
        share_history_for(&paths, "p", false).unwrap();

        let prof = paths.profile_dir("p");
        assert!(prof.join("projects").is_symlink());
        assert!(prof.join("history.jsonl").is_symlink());
    }
}
