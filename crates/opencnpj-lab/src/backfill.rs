//! WP-1.5 historical backfill: build and verify one national T1/T2 aggregate at a time.
//! Port of the former `scripts/backfill-aggregates.py`; same layout under `<root>`
//! (`raw/`, `work/`, `logs/`, `aggregates/`), same manifest fields, same checks.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use clap::Args;
use opencnpj_ingest::source::sha256_file;
use serde::{Deserialize, Serialize};

#[derive(Args)]
pub(crate) struct BackfillArgs {
    /// First competence (YYYY-MM).
    #[arg(long, default_value = "2024-11")]
    first: String,
    /// Last competence (YYYY-MM).
    #[arg(long, default_value = "2026-10")]
    last: String,
    #[arg(long, default_value_t = 16)]
    workers: usize,
    #[arg(long, default_value_t = 4)]
    readers: usize,
    /// Validate completed archives and report national totals; build nothing.
    #[arg(long)]
    summarize: bool,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/backfill")]
    root: PathBuf,
    /// Directory with the release `opencnpj` and `opencnpj-intel` binaries.
    #[arg(long, default_value = "/data/opencnpj-target/release")]
    bin: PathBuf,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    catalog: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Verification {
    status: String,
    log: String,
}

/// Field order is the manifest's key order.
#[derive(Serialize, Deserialize)]
struct Record {
    competence: String,
    source_snapshot_sha256: String,
    municipality_catalogue_sha256: String,
    source_verification: Verification,
    aggregate_sha256: String,
    aggregate_bytes: u64,
    active_establishments: u64,
    all_establishments: u64,
    sampled_peak_working_bytes: u64,
    sampled_minimum_volume_free_bytes: u64,
}

fn months(first: &str, last: &str) -> Result<Vec<String>> {
    let parse = |value: &str| -> Result<(u16, u8)> {
        let (year, month) = value.split_once('-').context("expected YYYY-MM")?;
        let (year, month) = (year.parse::<u16>()?, month.parse::<u8>()?);
        ensure!(value.len() == 7 && (1..=12).contains(&month), "expected YYYY-MM, got {value}");
        Ok((year, month))
    };
    let (mut year, mut month) = parse(first)?;
    let end = parse(last)?;
    ensure!((year, month) <= end, "expected an ordered range");
    let mut out = Vec::new();
    loop {
        out.push(format!("{year:04}-{month:02}"));
        if (year, month) == end {
            return Ok(out);
        }
        (year, month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    }
}

/// Bytes allocated on disk by everything under `path` (0 when it does not exist).
#[cfg(unix)]
fn allocated_bytes(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let Ok(metadata) = fs::symlink_metadata(path) else { return 0 };
    let mut used = metadata.blocks() * 512;
    if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        // The builder renames and removes files while this runs: vanished entries count 0.
        used += entries.flatten().map(|entry| allocated_bytes(&entry.path())).sum::<u64>();
    }
    used
}

#[cfg(not(unix))]
fn allocated_bytes(_: &Path) -> u64 {
    0
}

/// Free bytes on the volume holding `path` (`df`, so no platform bindings are needed).
fn free_bytes(path: &Path) -> Result<u64> {
    let output = Command::new("df").args(["-B1", "--output=avail"]).arg(path).output().context("run df")?;
    ensure!(output.status.success(), "df failed for {}", path.display());
    String::from_utf8(output.stdout)?.lines().nth(1).context("df printed no value")?.trim().parse().context("parse df output")
}

fn quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|ch| ch.is_ascii_alphanumeric() || "@%+=:,./-_".contains(ch)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\"'\"'"))
    }
}

/// Runs `command` at idle CPU and I/O priority, appending its output to `log` and sampling
/// disk use every five seconds; fails when the command fails.
fn run(command: &[String], log: &Path, sample: &mut dyn FnMut() -> Result<()>) -> Result<()> {
    let mut full = vec!["nice", "-n", "19", "ionice", "-c3"].into_iter().map(str::to_string).collect::<Vec<_>>();
    full.extend(command.iter().cloned());
    let mut file = OpenOptions::new().create(true).append(true).open(log)?;
    writeln!(file, "\n$ {}", full.iter().map(|word| quote(word)).collect::<Vec<_>>().join(" "))?;
    let mut child =
        Command::new(&full[0]).args(&full[1..]).stdout(file.try_clone()?).stderr(file.try_clone()?).stdin(Stdio::null()).spawn()?;
    let status = loop {
        if let Err(error) = sample() {
            let _ = child.kill();
            return Err(error);
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_secs(5));
    };
    sample()?;
    writeln!(file, "exit status: {}", status.code().unwrap_or(-1))?;
    ensure!(status.success(), "command failed with status {status}; see {}", log.display());
    Ok(())
}

fn checked_record(root: &Path, month: &str, catalogue_sha: &str) -> Result<Record> {
    let archive = root.join("aggregates");
    let manifest = archive.join(format!("{month}.json"));
    let aggregate = archive.join(format!("{month}.ocagg"));
    ensure!(manifest.is_file(), "missing archive manifest: {}", manifest.display());
    let record: Record = serde_json::from_str(&fs::read_to_string(&manifest)?)?;
    let sidecar = fs::read_to_string(archive.join(format!("{month}.ocagg.sha256")))?;
    let matches = record.competence == month
        && record.municipality_catalogue_sha256 == catalogue_sha
        && record.source_verification.status == "passed"
        && sha256_file(&aggregate)? == record.aggregate_sha256
        && sidecar.split_whitespace().next() == Some(record.aggregate_sha256.as_str());
    ensure!(matches, "archive does not match {}", manifest.display());
    Ok(record)
}

fn backfill(args: &BackfillArgs, month: &str, catalogue_sha: &str) -> Result<()> {
    let root = &args.root;
    let sources = root.join("raw").join(month);
    let work = root.join("work").join(month);
    let logs = root.join("logs").join(month);
    let archive = root.join("aggregates");
    let aggregate = archive.join(format!("{month}.ocagg"));
    let checksum = archive.join(format!("{month}.ocagg.sha256"));
    let manifest = archive.join(format!("{month}.json"));
    let snapshot = work.join(format!("{month}.ocnpj"));
    let disk_log = logs.join("disk.tsv");
    let path = |value: &Path| value.display().to_string();

    if !manifest.exists() {
        for directory in [&work, &logs, &archive] {
            fs::create_dir_all(directory)?;
        }
        let mut sample = || -> Result<()> {
            // Allocated bytes of the ZIPs, scratch files and snapshot together.
            let used = allocated_bytes(&sources) + allocated_bytes(&work);
            let mut stream = OpenOptions::new().create(true).append(true).open(&disk_log)?;
            writeln!(stream, "{used}\t{}", free_bytes(root)?)?;
            Ok(())
        };
        let opencnpj = path(&args.bin.join("opencnpj"));
        let raw_root = path(sources.parent().context("raw directory")?);
        run(
            &[opencnpj.clone(), "download".into(), "--month".into(), month.into(), "--dest".into(), raw_root],
            &logs.join("download.log"),
            &mut sample,
        )?;
        // Intermediate build files belong to this runner and can be rebuilt from the ZIPs.
        if !work.join(format!("{month}.ocnpj.sha256")).exists() {
            let scratch = work.join("scratch");
            if scratch.exists() {
                fs::remove_dir_all(&scratch)?;
            }
            let _ = fs::remove_file(&snapshot);
            let command = [
                opencnpj.clone(),
                "build".into(),
                "--input".into(),
                path(&sources),
                "--output".into(),
                path(&snapshot),
                "--competence".into(),
                month.into(),
                "--profile".into(),
                "balanced".into(),
                "--tmp".into(),
                path(&scratch),
                "--workers".into(),
                args.workers.to_string(),
                "--readers".into(),
                args.readers.to_string(),
                "--report".into(),
                path(&logs.join("build.json")),
            ];
            run(&command, &logs.join("build.log"), &mut sample)?;
        }
        let verify_log = logs.join("verify.log");
        let command =
            [opencnpj, "verify".into(), path(&snapshot), "--sources".into(), path(&sources), "--workers".into(), args.workers.to_string()];
        run(&command, &verify_log, &mut sample)?;
        let text = fs::read_to_string(&verify_log)?;
        let verified = text.rsplit_once("\n$ ").map_or(text.as_str(), |(_, last)| last);
        let snapshot_sha = verified
            .split("sha256 ok: ")
            .nth(1)
            .map(|rest| rest.chars().take_while(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()).collect::<String>())
            .filter(|hash| hash.len() == 64);
        let Some(snapshot_sha) = snapshot_sha else { bail!("incomplete source verification; see {}", verify_log.display()) };
        ensure!(
            verified.matches("fingerprint MATCH").count() == 4 && verified.contains("verify passed in"),
            "incomplete source verification; see {}",
            verify_log.display()
        );
        for stale in [&aggregate, &checksum] {
            let _ = fs::remove_file(stale);
        }
        let aggregate_log = logs.join("aggregate.log");
        let command = [
            path(&args.bin.join("opencnpj-intel")),
            "agg".into(),
            "build".into(),
            "--snapshot".into(),
            path(&snapshot),
            "--catalog".into(),
            path(&args.catalog),
            "--out".into(),
            path(&aggregate),
            "--tables".into(),
            "T1,T2".into(),
        ];
        run(&command, &aggregate_log, &mut sample)?;
        let mut totals = [None, None];
        for line in fs::read_to_string(&aggregate_log)?.lines() {
            // "T1: <cells> cells, <counts> counts"
            let Some((table, rest)) = line.split_once(": ") else { continue };
            let index = match table {
                "T1" => 0,
                "T2" => 1,
                _ => continue,
            };
            let parts: Vec<&str> = rest.split(' ').collect();
            if let [cells, "cells,", counts, "counts"] = parts.as_slice()
                && cells.parse::<u64>().is_ok()
                && let Ok(counts) = counts.parse::<u64>()
            {
                totals[index] = Some(counts);
            }
        }
        let (Some(active), Some(all)) = (totals[0], totals[1]) else { bail!("invalid national totals; see {}", aggregate_log.display()) };
        ensure!(0 < active && active <= all, "invalid national totals; see {}", aggregate_log.display());
        let samples: Vec<(u64, u64)> = fs::read_to_string(&disk_log)?
            .lines()
            .map(|line| {
                let (used, free) = line.split_once('\t').context("disk sample")?;
                Ok((used.parse()?, free.parse()?))
            })
            .collect::<Result<_>>()?;
        let record = Record {
            competence: month.into(),
            source_snapshot_sha256: snapshot_sha,
            municipality_catalogue_sha256: catalogue_sha.into(),
            source_verification: Verification { status: "passed".into(), log: verify_log.strip_prefix(root)?.display().to_string() },
            aggregate_sha256: fs::read_to_string(&checksum)?.split_whitespace().next().context("empty checksum")?.into(),
            aggregate_bytes: fs::metadata(&aggregate)?.len(),
            active_establishments: active,
            all_establishments: all,
            sampled_peak_working_bytes: samples.iter().map(|sample| sample.0).max().context("no disk samples")?,
            sampled_minimum_volume_free_bytes: samples.iter().map(|sample| sample.1).min().context("no disk samples")?,
        };
        fs::copy(sources.join("source-manifest.json"), logs.join("source-manifest.json"))?;
        let staging = archive.join(format!("{month}.json.tmp"));
        fs::write(&staging, serde_json::to_string_pretty(&record)? + "\n")?;
        fs::rename(&staging, &manifest)?;
    }
    let record = checked_record(root, month, catalogue_sha)?;
    for directory in [&sources, &work] {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
    }
    println!("{}", serde_json::to_string(&record)?);
    Ok(())
}

fn summarize(records: &[Record]) -> Result<()> {
    let active: Vec<u64> = records.iter().map(|record| record.active_establishments).collect();
    let (lower, upper) = (*active.iter().min().context("no records")?, *active.iter().max().context("no records")?);
    let levels: Vec<char> = "▁▂▃▄▅▆▇█".chars().collect();
    println!("National T1 ({lower} to {upper}):");
    let span = (upper - lower).max(1) as f64;
    // Python's round(): ties go to the even level.
    let level = |count: u64| (((count - lower) as f64 * (levels.len() - 1) as f64) / span).round_ties_even() as usize;
    println!("{}", active.iter().map(|count| levels[level(*count)]).collect::<String>());
    println!("competence\tactive\tall\tactive_change_pct\tsampled_peak_working_bytes");
    let mut previous: Option<u64> = None;
    for record in records {
        let change = previous.map_or_else(
            || "-".to_string(),
            |previous| format!("{:+.3}%", 100.0 * (record.active_establishments as f64 - previous as f64) / previous as f64),
        );
        println!(
            "{}\t{}\t{}\t{change}\t{}",
            record.competence, record.active_establishments, record.all_establishments, record.sampled_peak_working_bytes
        );
        previous = Some(record.active_establishments);
    }
    println!("verified_competences {}", records.len());
    println!("sampled_peak_working_bytes {}", records.iter().map(|record| record.sampled_peak_working_bytes).max().unwrap_or(0));
    println!(
        "sampled_minimum_volume_free_bytes {}",
        records.iter().map(|record| record.sampled_minimum_volume_free_bytes).min().unwrap_or(0)
    );
    Ok(())
}

/// Removes the lock file when the run ends, successfully or not.
struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn run_backfill(args: BackfillArgs) -> Result<()> {
    ensure!(args.workers > 0 && args.readers > 0, "expected positive worker and reader counts");
    let months = months(&args.first, &args.last)?;
    let catalogue_sha = sha256_file(&args.catalog)?;
    if args.summarize {
        let records = months.iter().map(|month| checked_record(&args.root, month, &catalogue_sha)).collect::<Result<Vec<_>>>()?;
        return summarize(&records);
    }
    ensure!(cfg!(unix), "the backfill measures disk use with Unix metadata and runs on Linux only");
    fs::create_dir_all(&args.root)?;
    let lock_path = args.root.join("backfill.lock");
    let mut lock = match OpenOptions::new().write(true).create_new(true).open(&lock_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("a backfill is already running (remove {} if it is stale)", lock_path.display())
        }
        Err(error) => return Err(error.into()),
    };
    writeln!(lock, "{}", std::process::id())?;
    let _guard = Lock(lock_path);
    for month in &months {
        backfill(&args, month, &catalogue_sha)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::months;

    #[test]
    fn month_ranges() {
        assert_eq!(months("2025-11", "2026-02").unwrap(), ["2025-11", "2025-12", "2026-01", "2026-02"]);
        assert_eq!(months("2026-10", "2026-10").unwrap(), ["2026-10"]);
        assert!(months("2026-10", "2026-09").is_err());
        assert!(months("2026-13", "2026-12").is_err());
    }
}
