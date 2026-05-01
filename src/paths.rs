use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Paths {
    pub clod_home: PathBuf,
    pub claude_home: PathBuf,
}

impl Paths {
    pub fn resolve(
        clod_home_override: Option<PathBuf>,
        claude_home_override: Option<PathBuf>,
    ) -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Ok(Self {
            clod_home: clod_home_override.unwrap_or_else(|| home.join(".clod")),
            claude_home: claude_home_override.unwrap_or_else(|| home.join(".claude")),
        })
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.clod_home.join("profiles")
    }

    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(name)
    }

    pub fn shared_dir(&self) -> PathBuf {
        self.clod_home.join("shared")
    }

    pub fn active_file(&self) -> PathBuf {
        self.clod_home.join("active")
    }
}

pub fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("profile name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        anyhow::bail!("profile name cannot contain path separators or be `.`/`..`");
    }
    Ok(())
}

pub fn exists(p: &Path) -> bool {
    // symlink_metadata so we don't follow broken symlinks into "missing"
    p.symlink_metadata().is_ok()
}
