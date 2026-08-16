use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

pub struct ExcludeSet {
    set: GlobSet,
}

impl ExcludeSet {
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut b = GlobSetBuilder::new();
        // Always skip VCS / OS junk unless user wants everything.
        for p in [
            "**/.git/**",
            "**/.DS_Store",
            "**/Thumbs.db",
            "**/.simple-backups/**",
        ] {
            b.add(Glob::new(p).context(p)?);
        }
        for p in patterns {
            b.add(Glob::new(p).with_context(|| format!("bad exclude glob: {p}"))?);
        }
        Ok(Self { set: b.build()? })
    }

    pub fn matches(&self, rel: &str) -> bool {
        self.set.is_match(rel)
    }
}
