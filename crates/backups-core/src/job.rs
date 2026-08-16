use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    pub name: String,
    pub source: String,
    pub repo: String,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// After each successful job run, keep only this many newest snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_last: Option<usize>,
    /// When pruning via `keep_last`, also garbage-collect unreferenced objects.
    #[serde(default)]
    pub gc_after_prune: bool,
}

impl JobConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read job config {}", path.display()))?;
        let job: Self = serde_yaml::from_str(&text)
            .with_context(|| format!("parse job config {}", path.display()))?;
        Ok(job)
    }

    pub fn source_path(&self) -> Result<PathBuf> {
        expand_path(&self.source)
    }

    pub fn repo_path(&self) -> Result<PathBuf> {
        expand_path(&self.repo)
    }
}

/// Expand `~` and `$VAR` prefixes commonly used in job files.
pub fn expand_path(raw: &str) -> Result<PathBuf> {
    let s = raw.trim();
    if s.is_empty() {
        anyhow::bail!("empty path");
    }
    if let Some(rest) = s.strip_prefix("~/") {
        let home = dirs::home_dir().context("HOME not set")?;
        return Ok(home.join(rest));
    }
    if s == "~" {
        return dirs::home_dir().context("HOME not set");
    }
    if let Some(rest) = s.strip_prefix('$') {
        let (var, tail) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, ""),
        };
        let val = std::env::var(var).with_context(|| format!("env ${var} not set"))?;
        if tail.is_empty() {
            return Ok(PathBuf::from(val));
        }
        return Ok(PathBuf::from(val).join(tail));
    }
    Ok(PathBuf::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_job() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("job.yaml");
        let mut f = fs::File::create(&p).unwrap();
        writeln!(
            f,
            "name: t\nsource: /tmp/src\nrepo: /tmp/repo\nexclude:\n  - \"*.tmp\"\nschedule: \"0 2 * * *\"\n"
        )
        .unwrap();
        let j = JobConfig::load(&p).unwrap();
        assert_eq!(j.name, "t");
        assert_eq!(j.exclude, vec!["*.tmp"]);
        assert_eq!(j.schedule.as_deref(), Some("0 2 * * *"));
    }
}
