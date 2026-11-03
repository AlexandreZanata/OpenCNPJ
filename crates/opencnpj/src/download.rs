//! RFB download (public Nextcloud WebDAV share) and the monthly update workflow.
//!
//! Integrity limits: the share exposes size, ETag and Last-Modified but no content hash,
//! and it is served over HTTPS without signatures. We verify size, ZIP CRC-32 (while
//! building) and record a locally computed SHA-256. That proves the bytes did not change
//! after download; it does not prove they came from Receita Federal.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

pub const DEFAULT_BASE: &str = "https://arquivos.receitafederal.gov.br/public.php/webdav";
/// Public read-only share token published by RFB (not a secret).
pub const DEFAULT_TOKEN: &str = "YggdBLfdninEJX9";

#[derive(clap::Args, Clone)]
pub struct DownloadArgs {
    /// Destination root; files go to DEST/YYYY-MM/.
    #[arg(long, default_value = "data/raw")]
    pub dest: PathBuf,
    /// Competence to fetch (default: latest complete one).
    #[arg(long)]
    pub month: Option<String>,
    #[arg(long, default_value_t = 4)]
    pub parallel: usize,
    #[arg(long, default_value_t = 8)]
    pub retries: u32,
    #[arg(long, env = "RFB_BASE_URL", default_value = DEFAULT_BASE)]
    pub base_url: String,
    #[arg(long, env = "RFB_SHARE_TOKEN", default_value = DEFAULT_TOKEN)]
    pub share_token: String,
    /// Only list what would be downloaded.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub name: String,
    pub url: String,
    pub bytes: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceManifest {
    pub competence: String,
    pub base_url: String,
    pub files: Vec<RemoteFile>,
    pub note: String,
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<d:{name}>");
    let i = s.find(&open)? + open.len();
    let j = s[i..].find(&format!("</d:{name}>"))? + i;
    Some(&s[i..j])
}

/// `(href, size, etag, last_modified)` per WebDAV response.
type DavEntry = (String, Option<u64>, Option<String>, Option<String>);

fn parse_propfind(xml: &str) -> Vec<DavEntry> {
    xml.split("<d:response>")
        .skip(1)
        .filter_map(|r| {
            let href = tag(r, "href")?.to_string();
            let size = tag(r, "getcontentlength").and_then(|s| s.parse().ok());
            let etag = tag(r, "getetag").map(|s| s.replace("&quot;", ""));
            let lm = tag(r, "getlastmodified").map(str::to_string);
            Some((href, size, etag, lm))
        })
        .collect()
}

async fn propfind(client: &reqwest::Client, a: &DownloadArgs, path: &str) -> Result<String> {
    let url = format!("{}/{}", a.base_url.trim_end_matches('/'), path);
    let resp = client
        .request(reqwest::Method::from_bytes(b"PROPFIND")?, &url)
        .basic_auth(&a.share_token, Some(""))
        .header("Depth", "1")
        .send()
        .await
        .with_context(|| format!("PROPFIND {url}"))?;
    if !(resp.status().is_success() || resp.status().as_u16() == 207) {
        bail!("PROPFIND {url}: HTTP {}", resp.status());
    }
    Ok(resp.text().await?)
}

fn is_month(s: &str) -> bool {
    s.len() == 7 && s.as_bytes()[4] == b'-' && s[..4].bytes().all(|b| b.is_ascii_digit()) && s[5..].bytes().all(|b| b.is_ascii_digit())
}

/// A release is complete when it has every fact table and every reference table.
fn complete(files: &[RemoteFile]) -> bool {
    let has = |p: &str| files.iter().any(|f| f.name.starts_with(p) && f.bytes > 0);
    ["Empresas", "Estabelecimentos", "Socios", "Simples", "Cnaes", "Motivos", "Municipios", "Naturezas", "Paises", "Qualificacoes"]
        .iter()
        .all(|p| has(p))
}

async fn list_month(client: &reqwest::Client, a: &DownloadArgs, month: &str) -> Result<Vec<RemoteFile>> {
    let xml = propfind(client, a, &format!("{month}/")).await?;
    let mut files: Vec<RemoteFile> = parse_propfind(&xml)
        .into_iter()
        .filter_map(|(href, size, etag, lm)| {
            let name = href.rsplit('/').next()?.to_string();
            (name.ends_with(".zip") && !name.contains("..") && !name.contains('/')).then(|| RemoteFile {
                url: format!("{}/{month}/{name}", a.base_url.trim_end_matches('/')),
                name,
                bytes: size.unwrap_or(0),
                etag,
                last_modified: lm,
                sha256: None,
            })
        })
        .collect();
    files.sort_by(|x, y| x.name.cmp(&y.name));
    Ok(files)
}

/// Latest month whose listing is complete.
pub async fn discover(client: &reqwest::Client, a: &DownloadArgs) -> Result<(String, Vec<RemoteFile>)> {
    if let Some(m) = &a.month {
        let files = list_month(client, a, m).await?;
        anyhow::ensure!(complete(&files), "release {m} is incomplete on the server");
        return Ok((m.clone(), files));
    }
    let xml = propfind(client, a, "").await?;
    let mut months: Vec<String> = parse_propfind(&xml)
        .into_iter()
        .filter_map(|(href, ..)| href.trim_end_matches('/').rsplit('/').next().map(str::to_string))
        .filter(|m| is_month(m))
        .collect();
    months.sort();
    for m in months.iter().rev() {
        let files = list_month(client, a, m).await?;
        if complete(&files) {
            return Ok((m.clone(), files));
        }
        eprintln!("skipping {m}: incomplete release");
    }
    bail!("no complete release found")
}

async fn fetch(client: &reqwest::Client, a: &DownloadArgs, f: &RemoteFile, dir: &Path) -> Result<String> {
    let dest = dir.join(&f.name);
    let sidecar = dir.join(format!("{}.sha256", f.name));
    if let (Ok(m), Ok(s)) = (std::fs::metadata(&dest), std::fs::read_to_string(&sidecar))
        && m.len() == f.bytes
        && let Some(h) = s.split_whitespace().next()
    {
        return Ok(h.to_string());
    }
    let part = dir.join(format!("{}.part", f.name));
    let mut attempt = 0;
    loop {
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let result: Result<()> = async {
            if have > f.bytes {
                std::fs::remove_file(&part)?;
                bail!("partial file larger than remote, restarting");
            }
            if have < f.bytes {
                let mut req = client.get(&f.url).basic_auth(&a.share_token, Some(""));
                if have > 0 {
                    req = req.header("Range", format!("bytes={have}-"));
                }
                let resp = req.send().await?;
                let status = resp.status();
                let resume = status.as_u16() == 206;
                if !(status.is_success()) {
                    bail!("GET {}: HTTP {status}", f.name);
                }
                let mut out = tokio::fs::OpenOptions::new().create(true).append(resume).write(true).truncate(!resume).open(&part).await?;
                let mut stream = resp.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    out.write_all(&chunk?).await?;
                }
                out.flush().await?;
                out.sync_all().await?;
            }
            let got = std::fs::metadata(&part)?.len();
            if got != f.bytes {
                bail!("{}: got {got} bytes, expected {}", f.name, f.bytes);
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => break,
            Err(e) if attempt < a.retries => {
                attempt += 1;
                let wait = Duration::from_secs((1u64 << attempt.min(6)).min(60));
                eprintln!("{}: {e}; retry {attempt}/{} in {}s", f.name, a.retries, wait.as_secs());
                tokio::time::sleep(wait).await;
            }
            Err(e) => return Err(e),
        }
    }
    let p = part.clone();
    let hash = tokio::task::spawn_blocking(move || opencnpj_ingest::source::sha256_file(&p)).await??;
    std::fs::rename(&part, &dest)?;
    std::fs::write(&sidecar, format!("{hash}  {}\n", f.name))?;
    Ok(hash)
}

/// Downloads a release; returns its directory and manifest.
pub async fn download(a: &DownloadArgs) -> Result<(PathBuf, SourceManifest)> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(120))
        .user_agent(concat!("opencnpj/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let (month, files) = discover(&client, a).await?;
    let dir = a.dest.join(&month);
    let total: u64 = files.iter().map(|f| f.bytes).sum();
    eprintln!("release {month}: {} files, {:.2} GB → {}", files.len(), total as f64 / 1e9, dir.display());
    if a.dry_run {
        for f in &files {
            println!("{}\t{}", f.name, f.bytes);
        }
        return Ok((dir, SourceManifest { competence: month, base_url: a.base_url.clone(), files, note: "dry run".into() }));
    }
    std::fs::create_dir_all(&dir)?;
    let sem = Arc::new(tokio::sync::Semaphore::new(a.parallel.max(1)));
    let t0 = std::time::Instant::now();
    let mut tasks = Vec::new();
    for f in files.clone() {
        let (client, a, dir, sem) = (client.clone(), a.clone(), dir.clone(), sem.clone());
        tasks.push(tokio::spawn(async move {
            let _p = sem.acquire().await.expect("semaphore");
            let r = fetch(&client, &a, &f, &dir).await;
            (f.name, r)
        }));
    }
    let mut files = files;
    for t in tasks {
        let (name, r) = t.await?;
        let hash = r.with_context(|| format!("downloading {name}"))?;
        if let Some(f) = files.iter_mut().find(|f| f.name == name) {
            f.sha256 = Some(hash);
        }
    }
    eprintln!("downloaded/verified {:.2} GB in {:.1}s", total as f64 / 1e9, t0.elapsed().as_secs_f64());
    let manifest = SourceManifest {
        competence: month,
        base_url: a.base_url.clone(),
        files,
        note: "sha256 computed locally after download; the server publishes no checksums".into(),
    };
    std::fs::write(dir.join("source-manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok((dir, manifest))
}

pub fn run(a: DownloadArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(download(&a))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Monthly update: download → compare → build → verify → atomic publish (keeps previous).
// ---------------------------------------------------------------------------------------

#[derive(clap::Args)]
pub struct UpdateArgs {
    /// Data directory: raw/, snapshots/, `current`, `previous`.
    #[arg(long, default_value = "data")]
    pub data_dir: PathBuf,
    #[arg(long, default_value = "balanced")]
    pub profile: String,
    /// Compare every source row against the new snapshot before publishing.
    #[arg(long)]
    pub verify_sources: bool,
    /// Snapshots to keep (current + previous at least).
    #[arg(long, default_value_t = 2)]
    pub keep: usize,
    #[command(flatten)]
    pub download: DownloadArgs,
}

pub fn current_snapshot(data_dir: &Path, pointer: &str) -> Option<PathBuf> {
    let name = std::fs::read_to_string(data_dir.join(pointer)).ok()?;
    let name = name.trim();
    (!name.contains("..")).then(|| data_dir.join(name))
}

/// Atomically points `<data_dir>/<pointer>` at `rel`.
pub fn set_pointer(data_dir: &Path, pointer: &str, rel: &str) -> Result<()> {
    let tmp = data_dir.join(format!(".{pointer}.tmp"));
    std::fs::write(&tmp, format!("{rel}\n"))?;
    std::fs::File::open(&tmp)?.sync_all()?;
    std::fs::rename(&tmp, data_dir.join(pointer))?;
    Ok(())
}

pub fn update(a: UpdateArgs, log: &(dyn Fn(&str) + Sync)) -> Result<()> {
    let mut dl = a.download.clone();
    dl.dest = a.data_dir.join("raw");
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let (dir, manifest) = rt.block_on(download(&dl))?;
    let snaps = a.data_dir.join("snapshots");
    std::fs::create_dir_all(&snaps)?;
    if let Some(cur) = current_snapshot(&a.data_dir, "current")
        && let Ok(s) = opencnpj_store::Snapshot::open(&cur)
    {
        let mut old: Vec<(String, String)> = s.manifest.sources.iter().map(|f| (f.path.clone(), f.sha256.clone())).collect();
        let mut new: Vec<(String, String)> =
            manifest.files.iter().map(|f| (f.name.clone(), f.sha256.clone().unwrap_or_default())).collect();
        old.sort();
        new.sort();
        if old == new && s.manifest.profile == a.profile {
            log(&format!("up to date: {} already built from identical sources", cur.display()));
            return Ok(());
        }
    }
    let rel = format!("snapshots/{}-{}.ocnpj", manifest.competence, a.profile);
    let out = a.data_dir.join(&rel);
    let cfg = opencnpj_ingest::build::BuildConfig {
        input: dir.clone(),
        output: out.clone(),
        tmp: a.data_dir.join("work").join(&manifest.competence),
        competence: manifest.competence.clone(),
        profile: opencnpj_ingest::build::Profile::preset(&a.profile).context("unknown profile")?,
        workers: crate::default_threads(),
        readers: 8,
        keep_partition: false,
        sample_every: None,
        plain_columns: false,
    };
    let report = opencnpj_ingest::build::build(&cfg, log)?;
    std::fs::write(out.with_extension("build.json"), serde_json::to_vec_pretty(&report)?)?;
    let snap = opencnpj_store::Snapshot::open(&out)?;
    snap.verify(|_, _| {})?;
    if a.verify_sources {
        crate::output::verify_against_sources(&snap, &dir, crate::default_threads(), log)?;
    }
    drop(snap);
    if let Ok(prev) = std::fs::read_to_string(a.data_dir.join("current"))
        && prev.trim() != rel
    {
        set_pointer(&a.data_dir, "previous", prev.trim())?;
    }
    set_pointer(&a.data_dir, "current", &rel)?;
    log(&format!("published {rel} (sha256 {})", report.sha256));
    prune(&a.data_dir, a.keep, log)?;
    Ok(())
}

/// Removes old snapshots, never the ones named by `current` or `previous`.
fn prune(data_dir: &Path, keep: usize, log: &(dyn Fn(&str) + Sync)) -> Result<()> {
    let protected: Vec<PathBuf> = ["current", "previous"].iter().filter_map(|p| current_snapshot(data_dir, p)).collect();
    let mut snaps: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(data_dir.join("snapshots"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ocnpj"))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    snaps.sort();
    let excess = snaps.len().saturating_sub(keep.max(2));
    for (_, p) in snaps.into_iter().take(excess) {
        if !protected.contains(&p) {
            std::fs::remove_file(&p)?;
            let _ = std::fs::remove_file(p.with_extension("ocnpj.sha256"));
            log(&format!("pruned {}", p.display()));
        }
    }
    Ok(())
}

/// Swaps `current` and `previous`.
pub fn rollback(data_dir: &Path) -> Result<String> {
    let cur = std::fs::read_to_string(data_dir.join("current")).context("no current snapshot")?;
    let prev = std::fs::read_to_string(data_dir.join("previous")).context("no previous snapshot to roll back to")?;
    opencnpj_store::Snapshot::open(&data_dir.join(prev.trim())).context("previous snapshot is not readable")?;
    set_pointer(data_dir, "current", prev.trim())?;
    set_pointer(data_dir, "previous", cur.trim())?;
    Ok(prev.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propfind_parsing() {
        let xml = r#"<d:multistatus><d:response><d:href>/public.php/webdav/2026-10/</d:href></d:response><d:response><d:href>/public.php/webdav/2026-10/Cnaes.zip</d:href><d:propstat><d:prop><d:getetag>&quot;abc&quot;</d:getetag><d:getcontentlength>22078</d:getcontentlength></d:prop></d:propstat></d:response></d:multistatus>"#;
        let v = parse_propfind(xml);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].1, Some(22078));
        assert_eq!(v[1].2.as_deref(), Some("abc"));
        assert!(is_month("2026-10") && !is_month("cnpj.tar.gz"));
    }

    #[test]
    fn pointer_swap() {
        let d = tempfile::tempdir().unwrap();
        set_pointer(d.path(), "current", "snapshots/a.ocnpj").unwrap();
        assert_eq!(current_snapshot(d.path(), "current").unwrap(), d.path().join("snapshots/a.ocnpj"));
        std::fs::write(d.path().join("current"), "../../etc/passwd").unwrap();
        assert!(current_snapshot(d.path(), "current").is_none());
    }
}
