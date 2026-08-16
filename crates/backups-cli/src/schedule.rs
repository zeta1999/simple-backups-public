use anyhow::{Context, Result};
use backups_core::JobConfig;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;

pub fn print_units(job: &JobConfig, job_file: &Path) -> Result<()> {
    let job_file = fs::canonicalize(job_file).unwrap_or_else(|_| job_file.to_path_buf());
    let bin = current_bin()?;
    let cron = job.schedule.as_deref().unwrap_or("0 2 * * *");

    println!("# crontab (Linux / generic)");
    println!("{cron} {} job run {}", bin.display(), job_file.display());
    println!();
    println!("# systemd user timer unit sketch");
    println!(
        "# ~/.config/systemd/user/simple-backups-{}.service",
        job.name
    );
    println!("[Unit]");
    println!("Description=simple-backups job {}", job.name);
    println!("[Service]");
    println!("Type=oneshot");
    println!("ExecStart={} job run {}", bin.display(), job_file.display());
    println!();
    println!("# companion .timer with OnCalendar= derived from cron manually");
    println!();
    println!("# macOS LaunchAgent");
    println!("{}", launchd_plist(job, &bin, &job_file)?);
    Ok(())
}

pub fn install(job: &JobConfig, job_file: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let job_file = fs::canonicalize(job_file).unwrap_or_else(|_| job_file.to_path_buf());
        let bin = current_bin()?;
        let plist_path =
            launch_agents_dir()?.join(format!("com.simpletools.backups.{}.plist", job.name));
        let body = launchd_plist(job, &bin, &job_file)?;
        fs::write(&plist_path, body)?;
        let _ = Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .status();
        let status = Command::new("launchctl")
            .args(["load", &plist_path.to_string_lossy()])
            .status()
            .context("launchctl load")?;
        if !status.success() {
            anyhow::bail!("launchctl load failed");
        }
        println!("Installed LaunchAgent {}", plist_path.display());
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "On this OS, add the crontab line from `schedule print` for job '{}'.",
            job.name
        );
        print_units(job, job_file)?;
        Ok(())
    }
}

pub fn uninstall(job: &JobConfig) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist_path =
            launch_agents_dir()?.join(format!("com.simpletools.backups.{}.plist", job.name));
        if plist_path.exists() {
            let _ = Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()])
                .status();
            fs::remove_file(&plist_path)?;
            println!("Removed {}", plist_path.display());
        } else {
            println!("No LaunchAgent found at {}", plist_path.display());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "Remove the crontab / systemd unit for job '{}' manually.",
            job.name
        );
        Ok(())
    }
}

fn current_bin() -> Result<PathBuf> {
    env::current_exe().context("resolve current executable")
}

#[cfg(target_os = "macos")]
fn launch_agents_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("HOME")?;
    let dir = home.join("Library/LaunchAgents");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn launchd_plist(job: &JobConfig, bin: &Path, job_file: &Path) -> Result<String> {
    let label = format!("com.simpletools.backups.{}", job.name);
    // launchd has no cron; use StartCalendarInterval for daily 02:00 as default,
    // or StartInterval when schedule is missing. Parse simple "M H * * *" only.
    let (minute, hour) = parse_daily_cron(job.schedule.as_deref());
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{bin}</string>
    <string>job</string>
    <string>run</string>
    <string>{job}</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>
    <integer>{hour}</integer>
    <key>Minute</key>
    <integer>{minute}</integer>
  </dict>
  <key>RunAtLoad</key>
  <false/>
</dict>
</plist>
"#,
        bin = bin.display(),
        job = job_file.display(),
    ))
}

fn parse_daily_cron(schedule: Option<&str>) -> (u32, u32) {
    // "M H * * *" → (minute, hour); fallback 02:00
    let Some(s) = schedule else {
        return (0, 2);
    };
    let parts: Vec<_> = s.split_whitespace().collect();
    if parts.len() >= 2 {
        if let (Ok(m), Ok(h)) = (parts[0].parse(), parts[1].parse()) {
            return (m, h);
        }
    }
    (0, 2)
}
