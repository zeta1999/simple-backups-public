mod schedule;

use anyhow::{bail, Result};
#[cfg(feature = "pqc")]
use backups_core::format_pair_payload;
use backups_core::JobConfig;
use backups_engine::{
    create_snapshot, forget_snapshots, gc_repo, prune_keep_last, restore_snapshot, verify_repo,
    RestoreOptions, SnapshotOptions,
};
use backups_store::Repository;
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "simple-backups")]
#[command(about = "Content-addressed incremental backups with optional PQC pairing")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create an empty repository
    Init {
        repo: PathBuf,
        #[arg(long)]
        description: Option<String>,
    },
    /// Create a snapshot of a source directory
    Snapshot {
        source: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(short, long)]
        message: Option<String>,
        #[arg(long = "exclude", action = clap::ArgAction::Append)]
        exclude: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// List snapshots in a repository
    List {
        #[arg(long)]
        repo: PathBuf,
    },
    /// Show a snapshot summary
    Show {
        id: String,
        #[arg(long)]
        repo: PathBuf,
    },
    /// Verify snapshot manifests and object hashes
    Verify {
        id: Option<String>,
        #[arg(long)]
        repo: PathBuf,
    },
    /// Restore a snapshot to a target directory
    Restore {
        id: String,
        target: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// Diff two snapshots (path-level)
    Diff {
        from: String,
        to: String,
        #[arg(long)]
        repo: PathBuf,
    },
    /// Delete objects not referenced by any snapshot
    Gc {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Forget (delete) one or more snapshot manifests
    Forget {
        /// Snapshot ids to forget
        ids: Vec<String>,
        #[arg(long)]
        repo: PathBuf,
        /// Also run gc after forgetting
        #[arg(long)]
        gc: bool,
    },
    /// Prune old snapshots, keeping only the newest N
    Prune {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        keep_last: usize,
        #[arg(long)]
        gc: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Run or inspect a YAML job config
    Job {
        #[command(subcommand)]
        sub: JobCommands,
    },
    /// Install / print scheduled job runners (cron / launchd)
    Schedule {
        #[command(subcommand)]
        sub: ScheduleCommands,
    },
    /// Generate a long-term ML-DSA-65 identity (requires --features pqc)
    IdentityGen {
        #[arg(long, default_value = "vault.bin")]
        vault: PathBuf,
    },
    /// Out-of-band PQC pairing (requires --features pqc)
    Pair {
        #[arg(long, default_value = "vault.bin")]
        vault: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long)]
        addr: String,
        #[arg(long)]
        code: Option<String>,
        /// Listen instead of connecting
        #[arg(long)]
        listen: bool,
        /// Address printed in the mobile QR payload (defaults to --addr)
        #[arg(long)]
        advertise: Option<String>,
    },
    /// Print a mobile QR pairing payload (requires --features pqc)
    PairQr {
        /// Advertised address phones should dial (host:port)
        #[arg(long)]
        addr: String,
        /// Optional code; generated if omitted
        #[arg(long)]
        code: Option<String>,
    },
    /// Push local objects to a paired peer (requires --features pqc)
    Push {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "vault.bin")]
        vault: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long)]
        addr: String,
        /// Push only the latest snapshot (default: all)
        #[arg(long)]
        latest: bool,
    },
    /// Pull objects from a paired peer (requires --features pqc)
    Pull {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "vault.bin")]
        vault: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long)]
        addr: String,
        /// Snapshot id to pull (default: latest)
        #[arg(long, default_value = "latest")]
        snapshot: String,
    },
    /// Serve push/pull for a paired peer (requires --features pqc)
    Serve {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long, default_value = "vault.bin")]
        vault: PathBuf,
        #[arg(long)]
        peer: String,
        #[arg(long, default_value = "127.0.0.1:9876")]
        listen: String,
        /// Exit after handling a single session
        #[arg(long)]
        once: bool,
    },
}

#[derive(Subcommand)]
enum JobCommands {
    /// Execute a job YAML file
    Run { file: PathBuf },
    /// Validate and print a job YAML file
    Show { file: PathBuf },
}

#[derive(Subcommand)]
enum ScheduleCommands {
    /// Print crontab line / launchd plist for a job
    Print { file: PathBuf },
    /// Install a macOS LaunchAgent (no-op hint on Linux)
    Install { file: PathBuf },
    /// Remove a previously installed LaunchAgent
    Uninstall { file: PathBuf },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Init { repo, description } => {
            let r = Repository::init(&repo, description)?;
            println!("Initialized repository at {}", r.root().display());
        }
        Commands::Snapshot {
            source,
            repo,
            message,
            exclude,
            dry_run,
        } => {
            let repo = Repository::open(&repo)?;
            let opts = SnapshotOptions {
                message,
                exclude,
                dry_run,
            };
            let (m, stats) = create_snapshot(&repo, &source, &opts)?;
            println!(
                "snapshot {}  files={} new={} reused={} bytes_stored={}{}",
                m.id,
                stats.files_total,
                stats.files_new,
                stats.files_reused,
                stats.bytes_stored,
                if dry_run { " (dry-run)" } else { "" }
            );
        }
        Commands::List { repo } => {
            let repo = Repository::open(&repo)?;
            for id in repo.list_snapshots()? {
                let m = repo.load_snapshot(&id)?;
                let msg = m.message.as_deref().unwrap_or("");
                println!(
                    "{}\t{}\tfiles={}\t{}",
                    m.id,
                    m.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
                    m.file_count(),
                    msg
                );
            }
        }
        Commands::Show { id, repo } => {
            let repo = Repository::open(&repo)?;
            let m = repo.load_snapshot(&id)?;
            println!("id:      {}", m.id);
            println!("created: {}", m.created_at);
            println!("source:  {}", m.source);
            println!("parent:  {}", m.parent.as_deref().unwrap_or("-"));
            println!("message: {}", m.message.as_deref().unwrap_or("-"));
            println!("files:   {}", m.file_count());
            println!("entries: {}", m.files.len());
            println!("hash:    {}", m.content_hash);
        }
        Commands::Verify { id, repo } => {
            let repo = Repository::open(&repo)?;
            let report = verify_repo(&repo, id.as_deref())?;
            println!(
                "checked {} snapshot(s), {} object(s)",
                report.snapshots_checked, report.objects_checked
            );
            if report.errors.is_empty() {
                println!("ok");
            } else {
                for e in &report.errors {
                    eprintln!("error: {e}");
                }
                bail!("{} verification error(s)", report.errors.len());
            }
        }
        Commands::Restore {
            id,
            target,
            repo,
            path,
            dry_run,
        } => {
            let repo = Repository::open(&repo)?;
            let m = repo.load_snapshot(&id)?;
            let n = restore_snapshot(
                &repo,
                &m,
                &target,
                &RestoreOptions {
                    path_filter: path,
                    dry_run,
                },
            )?;
            println!(
                "restored {n} file(s)/symlink(s) from {}{}",
                m.id,
                if dry_run { " (dry-run)" } else { "" }
            );
        }
        Commands::Diff { from, to, repo } => {
            let repo = Repository::open(&repo)?;
            let a = repo.load_snapshot(&from)?;
            let b = repo.load_snapshot(&to)?;
            diff_manifests(&a, &b);
        }
        Commands::Gc { repo, dry_run } => {
            let repo = Repository::open(&repo)?;
            let report = gc_repo(&repo, dry_run)?;
            println!(
                "gc{}: seen={} kept={} deleted={} bytes_freed={}",
                if dry_run { " (dry-run)" } else { "" },
                report.objects_seen,
                report.objects_kept,
                report.objects_deleted,
                report.bytes_freed
            );
        }
        Commands::Forget { ids, repo, gc } => {
            if ids.is_empty() {
                bail!("pass at least one snapshot id");
            }
            let repo = Repository::open(&repo)?;
            let report = forget_snapshots(&repo, &ids)?;
            println!(
                "forgot {} snapshot(s); latest={}",
                report.forgotten.len(),
                report.latest.as_deref().unwrap_or("-")
            );
            if gc {
                let g = gc_repo(&repo, false)?;
                println!(
                    "gc: deleted={} bytes_freed={}",
                    g.objects_deleted, g.bytes_freed
                );
            }
        }
        Commands::Prune {
            repo,
            keep_last,
            gc,
            dry_run,
        } => {
            let repo = Repository::open(&repo)?;
            if dry_run {
                let ids = repo.list_snapshots()?;
                if ids.len() > keep_last {
                    let drop_count = ids.len() - keep_last;
                    for id in ids.iter().take(drop_count) {
                        println!("would forget {id}");
                    }
                    println!("would keep {} newest snapshot(s)", keep_last);
                } else {
                    println!(
                        "nothing to prune (have {}, keep_last={keep_last})",
                        ids.len()
                    );
                }
            } else {
                let report = prune_keep_last(&repo, keep_last)?;
                println!(
                    "pruned {} snapshot(s); latest={}",
                    report.forgotten.len(),
                    report.latest.as_deref().unwrap_or("-")
                );
                if gc {
                    let g = gc_repo(&repo, false)?;
                    println!(
                        "gc: deleted={} bytes_freed={}",
                        g.objects_deleted, g.bytes_freed
                    );
                }
            }
        }
        Commands::Job { sub } => match sub {
            JobCommands::Show { file } => {
                let job = JobConfig::load(&file)?;
                println!("name:     {}", job.name);
                println!("source:   {}", job.source_path()?.display());
                println!("repo:     {}", job.repo_path()?.display());
                println!("schedule: {}", job.schedule.as_deref().unwrap_or("-"));
                println!(
                    "keep_last:{}",
                    job.keep_last
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "-".into())
                );
                println!("gc_prune: {}", job.gc_after_prune);
                println!("exclude:  {:?}", job.exclude);
            }
            JobCommands::Run { file } => {
                let job = JobConfig::load(&file)?;
                run_job(&job)?;
            }
        },
        Commands::Schedule { sub } => match sub {
            ScheduleCommands::Print { file } => {
                let job = JobConfig::load(&file)?;
                schedule::print_units(&job, &file)?;
            }
            ScheduleCommands::Install { file } => {
                let job = JobConfig::load(&file)?;
                schedule::install(&job, &file)?;
            }
            ScheduleCommands::Uninstall { file } => {
                let job = JobConfig::load(&file)?;
                schedule::uninstall(&job)?;
            }
        },
        Commands::IdentityGen { vault } => {
            #[cfg(not(feature = "pqc"))]
            {
                let _ = vault;
                require_pqc()?;
            }
            #[cfg(feature = "pqc")]
            {
                let password = backups_transfer::prompt_password(!vault.exists())?;
                let mut mgr = backups_transfer::VaultManager::new(&vault);
                mgr.create_or_open(password.as_bytes())?;
                let id = mgr.get_or_create_identity()?;
                println!("Identity ready in {}", vault.display());
                println!("Verifying key (hex): {}", hex_encode(&id.verifying_key()));
            }
        }
        Commands::PairQr { addr, code } => {
            #[cfg(not(feature = "pqc"))]
            {
                let _ = (addr, code);
                require_pqc()?;
            }
            #[cfg(feature = "pqc")]
            {
                let code = match code {
                    Some(c) => c,
                    None => backups_transfer::generate_pairing_code()?,
                };
                let payload = format_pair_payload(&addr, &code);
                println!("{payload}");
                eprintln!("Share this string / QR with the phone. Code: {code}");
            }
        }
        Commands::Pair {
            vault,
            peer,
            addr,
            code,
            listen,
            advertise,
        } => {
            #[cfg(not(feature = "pqc"))]
            {
                let _ = (vault, peer, addr, code, listen, advertise);
                require_pqc()?;
            }
            #[cfg(feature = "pqc")]
            {
                use backups_transfer::PairRole;
                let password = backups_transfer::prompt_password(false)?;
                let code = match code {
                    Some(c) => c,
                    None => backups_transfer::generate_pairing_code()?,
                };
                let qr_addr = advertise.as_deref().unwrap_or(&addr);
                // Always print the phone payload so QR / paste pairing is one step.
                let payload = format_pair_payload(qr_addr, &code);
                println!("{payload}");
                eprintln!("Pairing code: {code}");
                let role = if listen {
                    eprintln!("Listening on {addr} for pairing...");
                    PairRole::Listener
                } else {
                    eprintln!("Connecting to {addr} for pairing...");
                    PairRole::Initiator
                };
                let peer_vk = backups_transfer::pair_peers(
                    &vault,
                    password.as_bytes(),
                    &peer,
                    &addr,
                    &code,
                    role,
                )
                .await?;
                println!(
                    "Paired peer '{peer}'. Pinned verifying key: {}",
                    hex_encode(&peer_vk)
                );
            }
        }
        Commands::Push {
            repo,
            vault,
            peer,
            addr,
            latest,
        } => {
            #[cfg(not(feature = "pqc"))]
            {
                let _ = (repo, vault, peer, addr, latest);
                require_pqc()?;
            }
            #[cfg(feature = "pqc")]
            {
                let password = backups_transfer::prompt_password(false)?;
                let repo = Repository::open(&repo)?;
                let stats = backups_transfer::push_to_peer(
                    &repo,
                    &vault,
                    password.as_bytes(),
                    &peer,
                    &addr,
                    latest,
                )
                .await?;
                println!(
                    "push ok: snapshots={} objects={} bytes={}",
                    stats.snapshots, stats.objects_sent, stats.bytes_sent
                );
            }
        }
        Commands::Pull {
            repo,
            vault,
            peer,
            addr,
            snapshot,
        } => {
            #[cfg(not(feature = "pqc"))]
            {
                let _ = (repo, vault, peer, addr, snapshot);
                require_pqc()?;
            }
            #[cfg(feature = "pqc")]
            {
                let password = backups_transfer::prompt_password(false)?;
                let repo = Repository::open(&repo)?;
                let stats = backups_transfer::pull_from_peer(
                    &repo,
                    &vault,
                    password.as_bytes(),
                    &peer,
                    &addr,
                    &snapshot,
                )
                .await?;
                println!(
                    "pull ok: snapshots={} objects={} bytes={}",
                    stats.snapshots, stats.objects_received, stats.bytes_received
                );
            }
        }
        Commands::Serve {
            repo,
            vault,
            peer,
            listen,
            once,
        } => {
            #[cfg(not(feature = "pqc"))]
            {
                let _ = (repo, vault, peer, listen, once);
                require_pqc()?;
            }
            #[cfg(feature = "pqc")]
            {
                let password = backups_transfer::prompt_password(false)?;
                let repo = Repository::open(&repo)?;
                backups_transfer::serve_peer(
                    &repo,
                    &vault,
                    password.as_bytes(),
                    &peer,
                    &listen,
                    once,
                )
                .await?;
            }
        }
    }
    Ok(())
}

#[cfg(not(feature = "pqc"))]
fn require_pqc() -> Result<()> {
    bail!("this command requires a build with --features pqc")
}

#[cfg(feature = "pqc")]
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn run_job(job: &JobConfig) -> Result<()> {
    let source = job.source_path()?;
    let repo_path = job.repo_path()?;
    if !repo_path.join("config.yaml").exists() {
        Repository::init(&repo_path, Some(job.name.clone()))?;
        eprintln!("initialized repo {}", repo_path.display());
    }
    let repo = Repository::open(&repo_path)?;
    let opts = SnapshotOptions {
        message: job
            .message
            .clone()
            .or_else(|| Some(format!("job:{}", job.name))),
        exclude: job.exclude.clone(),
        dry_run: false,
    };
    let (m, stats) = create_snapshot(&repo, &source, &opts)?;
    println!(
        "job {} → snapshot {} (new={} reused={})",
        job.name, m.id, stats.files_new, stats.files_reused
    );
    if let Some(keep) = job.keep_last {
        let report = prune_keep_last(&repo, keep)?;
        if !report.forgotten.is_empty() {
            println!(
                "job {}: pruned {} old snapshot(s) (keep_last={keep})",
                job.name,
                report.forgotten.len()
            );
        }
        if job.gc_after_prune {
            let g = gc_repo(&repo, false)?;
            if g.objects_deleted > 0 {
                println!(
                    "job {}: gc deleted {} object(s), freed {} bytes",
                    job.name, g.objects_deleted, g.bytes_freed
                );
            }
        }
    }
    Ok(())
}

fn diff_manifests(a: &backups_core::SnapshotManifest, b: &backups_core::SnapshotManifest) {
    let ka: BTreeSet<_> = a.files.keys().collect();
    let kb: BTreeSet<_> = b.files.keys().collect();
    for p in ka.difference(&kb) {
        println!("- {p}");
    }
    for p in kb.difference(&ka) {
        println!("+ {p}");
    }
    for p in ka.intersection(&kb) {
        let ea = &a.files[*p];
        let eb = &b.files[*p];
        let changed = ea.kind != eb.kind
            || ea.object != eb.object
            || ea.symlink_target != eb.symlink_target
            || ea.size != eb.size;
        if changed {
            println!("M {p}");
        }
    }
}
