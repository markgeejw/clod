use std::fs;

use anyhow::{Context, Result};

use crate::paths::{validate_profile_name, Paths};

pub enum ActiveSource {
    Env,
    File,
}

pub struct Active {
    pub name: String,
    pub source: ActiveSource,
}

pub fn read_active(paths: &Paths) -> Result<Option<Active>> {
    if let Ok(name) = std::env::var("CLOD_PROFILE") {
        let name = name.trim().to_string();
        if !name.is_empty() {
            return Ok(Some(Active {
                name,
                source: ActiveSource::Env,
            }));
        }
    }

    let path = paths.active_file();
    match fs::read_to_string(&path) {
        Ok(s) => {
            let name = s.trim().to_string();
            if name.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Active {
                    name,
                    source: ActiveSource::File,
                }))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn resolve_active(paths: &Paths) -> Result<Active> {
    read_active(paths)?.ok_or_else(|| {
        anyhow::anyhow!("no active profile; run `clod switch <name>` (or set CLOD_PROFILE)")
    })
}

pub fn write_active(paths: &Paths, name: &str) -> Result<()> {
    validate_profile_name(name)?;
    let path = paths.active_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(&path, format!("{}\n", name))
        .with_context(|| format!("writing {}", path.display()))?;
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
    fn read_returns_none_when_unset() {
        let tmp = TempDir::new().unwrap();
        // SAFETY: tests in this module don't run in parallel with anything that reads CLOD_PROFILE
        std::env::remove_var("CLOD_PROFILE");
        let active = read_active(&paths_in(&tmp)).unwrap();
        assert!(active.is_none());
    }

    #[test]
    fn write_then_read_round_trip() {
        let tmp = TempDir::new().unwrap();
        std::env::remove_var("CLOD_PROFILE");
        let paths = paths_in(&tmp);
        write_active(&paths, "work").unwrap();
        let active = read_active(&paths).unwrap().unwrap();
        assert_eq!(active.name, "work");
        assert!(matches!(active.source, ActiveSource::File));
    }

    #[test]
    fn env_overrides_file() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);
        write_active(&paths, "work").unwrap();
        std::env::set_var("CLOD_PROFILE", "personal");
        let active = read_active(&paths).unwrap().unwrap();
        assert_eq!(active.name, "personal");
        assert!(matches!(active.source, ActiveSource::Env));
        std::env::remove_var("CLOD_PROFILE");
    }
}
