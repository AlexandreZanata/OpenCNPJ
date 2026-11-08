//! Immutable `.ocind` indicator store.
//!
//! The container is a read-only SQLite database with an embedded manifest. A
//! new import is written to a temporary file and published with no-clobber
//! semantics, so a failed refresh leaves the previous artifact untouched.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use sha2::{Digest, Sha256};

use super::{
    model::{CellStatus, DatasetManifest, Observation},
    registry::Registry,
};

pub const INDICATOR_EXTENSION: &str = "ocind";
pub const FORMAT_VERSION: u32 = 1;

const SCHEMA: &str = r#"
CREATE TABLE manifest (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    format_version INTEGER NOT NULL,
    source_id TEXT NOT NULL,
    dataset_id TEXT NOT NULL,
    title TEXT NOT NULL,
    publisher TEXT NOT NULL,
    source_url TEXT NOT NULL,
    metadata_url TEXT NOT NULL,
    licence TEXT NOT NULL,
    licence_url TEXT NOT NULL,
    licence_status TEXT NOT NULL,
    attribution TEXT NOT NULL,
    retrieved_at TEXT NOT NULL,
    publication TEXT,
    coverage TEXT NOT NULL,
    unit TEXT NOT NULL,
    period_from TEXT,
    period_to TEXT,
    raw_sha256 TEXT NOT NULL,
    raw_bytes INTEGER NOT NULL,
    row_count INTEGER NOT NULL,
    accepted_rows INTEGER NOT NULL,
    rejected_rows INTEGER NOT NULL,
    duplicate_rows INTEGER NOT NULL,
    malformed_rows INTEGER NOT NULL,
    state TEXT NOT NULL,
    limitations TEXT NOT NULL
);
CREATE TABLE observations (
    dataset_id TEXT NOT NULL,
    variable_id TEXT NOT NULL,
    variable_label TEXT NOT NULL,
    unit TEXT NOT NULL,
    classification_id TEXT NOT NULL,
    classification_label TEXT NOT NULL,
    category_id TEXT NOT NULL,
    category_label TEXT NOT NULL,
    geography_level TEXT NOT NULL,
    geography_code TEXT NOT NULL,
    geography_name TEXT NOT NULL,
    period TEXT NOT NULL,
    value REAL,
    value_status TEXT NOT NULL,
    source_url TEXT NOT NULL,
    PRIMARY KEY (
        dataset_id, variable_id, classification_id, category_id,
        geography_level, geography_code, period
    )
);
CREATE INDEX observations_variable ON observations(variable_id, category_id, period);
"#;

const OBSERVATION_COLUMNS: &str = "dataset_id, variable_id, variable_label, unit, classification_id, classification_label, category_id, category_label, geography_level, geography_code, geography_name, period, value, value_status, source_url";

/// A verified, immutable indicator database.
pub struct Store {
    path: PathBuf,
    connection: Connection,
    manifest: DatasetManifest,
}

/// Result of publishing a new store.
#[derive(Debug, Clone)]
pub struct Published {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub manifest: DatasetManifest,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("open indicator store {}", path.display()))?;
        let manifest = read_manifest(&connection).with_context(|| format!("read manifest of {}", path.display()))?;
        manifest.validate().map_err(anyhow::Error::msg)?;
        Ok(Self { path, connection, manifest })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn manifest(&self) -> &DatasetManifest {
        &self.manifest
    }

    pub fn observations(&self, variable_id: &str, category: Option<(&str, &str)>) -> Result<Vec<Observation>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {OBSERVATION_COLUMNS} FROM observations
             WHERE variable_id = ?1
               AND (?2 IS NULL OR (classification_id = ?2 AND category_id = ?3))
             ORDER BY geography_code, period, category_id"
        ))?;
        let rows = statement.query_map(params![variable_id, category.map(|c| c.0), category.map(|c| c.1)], observation_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().with_context(|| format!("read {} observations", self.path.display()))
    }

    pub fn row_count(&self) -> Result<u64> {
        let count: i64 = self.connection.query_row("SELECT COUNT(*) FROM observations", [], |row| row.get(0))?;
        u64::try_from(count).context("negative observation count")
    }

    /// Full structural and row-accounting verification.
    pub fn verify(&self) -> Result<()> {
        let check: String = self.connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        ensure!(check == "ok", "SQLite integrity check failed for {}: {check}", self.path.display());
        self.manifest.validate().map_err(anyhow::Error::msg)?;
        let rows = self.row_count()?;
        ensure!(
            rows == self.manifest.accepted_rows,
            "{} has {rows} rows but its manifest declares {}",
            self.path.display(),
            self.manifest.accepted_rows
        );
        Ok(())
    }
}

fn read_manifest(connection: &Connection) -> Result<DatasetManifest> {
    connection
        .query_row(
            "SELECT format_version, source_id, dataset_id, title, publisher, source_url, metadata_url,
                    licence, licence_url, licence_status, attribution, retrieved_at, publication,
                    coverage, unit, period_from, period_to, raw_sha256, raw_bytes, row_count,
                    accepted_rows, rejected_rows, duplicate_rows, malformed_rows, state, limitations
             FROM manifest WHERE id = 1",
            [],
            |row| {
                let limitations: String = row.get(25)?;
                Ok(DatasetManifest {
                    format_version: row.get::<_, u32>(0)?,
                    source_id: row.get(1)?,
                    dataset_id: row.get(2)?,
                    title: row.get(3)?,
                    publisher: row.get(4)?,
                    source_url: row.get(5)?,
                    metadata_url: row.get(6)?,
                    licence: row.get(7)?,
                    licence_url: row.get(8)?,
                    licence_status: row.get(9)?,
                    attribution: row.get(10)?,
                    retrieved_at: row.get(11)?,
                    publication: row.get(12)?,
                    coverage: row.get(13)?,
                    unit: row.get(14)?,
                    period_from: row.get(15)?,
                    period_to: row.get(16)?,
                    raw_sha256: row.get(17)?,
                    raw_bytes: get_u64(row, 18)?,
                    row_count: get_u64(row, 19)?,
                    accepted_rows: get_u64(row, 20)?,
                    rejected_rows: get_u64(row, 21)?,
                    duplicate_rows: get_u64(row, 22)?,
                    malformed_rows: get_u64(row, 23)?,
                    state: row.get(24)?,
                    limitations: serde_json::from_str(&limitations).unwrap_or_default(),
                })
            },
        )
        .optional()?
        .context("missing manifest row")
}

fn get_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn observation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Observation> {
    let status: String = row.get(13)?;
    Ok(Observation {
        dataset_id: row.get(0)?,
        variable_id: row.get(1)?,
        variable_label: row.get(2)?,
        unit: row.get(3)?,
        classification_id: row.get(4)?,
        classification_label: row.get(5)?,
        category_id: row.get(6)?,
        category_label: row.get(7)?,
        geography_level: row.get(8)?,
        geography_code: row.get(9)?,
        geography_name: row.get(10)?,
        period: row.get(11)?,
        value: row.get(12)?,
        status: CellStatus::parse(&status).unwrap_or(CellStatus::Malformed),
        source_url: row.get(14)?,
    })
}

/// Publish a new immutable store. The destination must not exist.
pub fn write_store(
    path: impl AsRef<Path>,
    manifest: &DatasetManifest,
    observations: impl IntoIterator<Item = Observation>,
) -> Result<Published> {
    manifest.validate().map_err(anyhow::Error::msg)?;
    let path = path.as_ref().to_path_buf();
    ensure!(!path.exists(), "refusing to overwrite existing indicator store {}", path.display());
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let file = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut connection = Connection::open(file.path())?;
        connection.pragma_update(None, "journal_mode", "OFF")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(SCHEMA)?;
        let transaction = connection.transaction()?;
        insert_manifest(&transaction, manifest)?;
        let mut count = 0_u64;
        {
            let mut statement = transaction.prepare(&format!(
                "INSERT INTO observations ({OBSERVATION_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
            ))?;
            for observation in observations {
                statement.execute(params![
                    observation.dataset_id,
                    observation.variable_id,
                    observation.variable_label,
                    observation.unit,
                    observation.classification_id,
                    observation.classification_label,
                    observation.category_id,
                    observation.category_label,
                    observation.geography_level,
                    observation.geography_code,
                    observation.geography_name,
                    observation.period,
                    observation.value,
                    observation.status.as_str(),
                    observation.source_url,
                ])?;
                count += 1;
            }
        }
        ensure!(
            count == manifest.accepted_rows,
            "writer received {count} observations but the manifest accounts for {}",
            manifest.accepted_rows
        );
        transaction.commit()?;
        let check: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        ensure!(check == "ok", "new indicator store failed integrity check: {check}");
        connection.close().map_err(|(_, error)| error)?;
    }
    file.as_file().sync_all()?;
    let bytes = file.as_file().metadata()?.len();
    let sha256 = sha256_file(file.path())?;
    file.persist_noclobber(&path).with_context(|| format!("publish {}", path.display()))?;
    Ok(Published { path, sha256, bytes, manifest: manifest.clone() })
}

fn to_i64(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).with_context(|| format!("{field} exceeds SQLite integer range"))
}

fn insert_manifest(connection: &Connection, manifest: &DatasetManifest) -> Result<()> {
    let limitations = serde_json::to_string(&manifest.limitations)?;
    connection.execute(
        "INSERT INTO manifest VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
        params![
            manifest.format_version,
            manifest.source_id,
            manifest.dataset_id,
            manifest.title,
            manifest.publisher,
            manifest.source_url,
            manifest.metadata_url,
            manifest.licence,
            manifest.licence_url,
            manifest.licence_status,
            manifest.attribution,
            manifest.retrieved_at,
            manifest.publication,
            manifest.coverage,
            manifest.unit,
            manifest.period_from,
            manifest.period_to,
            manifest.raw_sha256,
            to_i64(manifest.raw_bytes, "raw bytes")?,
            to_i64(manifest.row_count, "row count")?,
            to_i64(manifest.accepted_rows, "accepted rows")?,
            to_i64(manifest.rejected_rows, "rejected rows")?,
            to_i64(manifest.duplicate_rows, "duplicate rows")?,
            to_i64(manifest.malformed_rows, "malformed rows")?,
            manifest.state,
            limitations,
        ],
    )?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(fs::File::open(path).with_context(|| format!("open {}", path.display()))?);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// All indicator stores found in one directory.
pub struct StoreSet {
    stores: Vec<Store>,
}

impl StoreSet {
    pub fn open_dir(directory: impl AsRef<Path>) -> Result<Self> {
        let directory = directory.as_ref();
        let mut paths = Vec::new();
        for entry in fs::read_dir(directory).with_context(|| format!("read {}", directory.display()))? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) == Some(INDICATOR_EXTENSION) {
                paths.push(path);
            }
        }
        paths.sort();
        ensure!(!paths.is_empty(), "no .{INDICATOR_EXTENSION} stores in {}", directory.display());
        let stores = paths.into_iter().map(Store::open).collect::<Result<Vec<_>>>()?;
        Ok(Self { stores })
    }

    pub fn stores(&self) -> &[Store] {
        &self.stores
    }

    pub fn manifests(&self) -> impl Iterator<Item = &DatasetManifest> {
        self.stores.iter().map(Store::manifest)
    }

    pub fn manifests_for<'a>(&'a self, dataset_ids: &'a BTreeSet<String>) -> Vec<&'a DatasetManifest> {
        self.stores.iter().map(Store::manifest).filter(|m| dataset_ids.contains(&m.dataset_id)).collect()
    }

    /// Collect observations for one curated variable and optional category.
    pub fn observations(&self, variable_id: &str, category: Option<(&str, &str)>) -> Result<Vec<Observation>> {
        let mut output = Vec::new();
        for store in &self.stores {
            output.extend(store.observations(variable_id, category)?);
        }
        Ok(output)
    }

    /// Reject duplicate cells across stores instead of silently choosing one.
    pub fn unique_observations(&self, variable_id: &str, category: Option<(&str, &str)>) -> Result<Vec<Observation>> {
        let mut seen = BTreeMap::new();
        for observation in self.observations(variable_id, category)? {
            if let Some(previous) = seen.insert(observation.key(), observation.clone()) {
                bail!("duplicate cell {} between {} and {}", observation.key(), previous.dataset_id, observation.dataset_id);
            }
        }
        Ok(seen.into_values().collect())
    }

    pub fn verify(&self) -> Result<()> {
        for store in &self.stores {
            store.verify()?;
        }
        Ok(())
    }
}

/// Dataset metadata for a registry dataset that has not been imported yet.
pub fn missing_dataset_message(registry: &Registry, dataset_id: &str) -> String {
    match registry.dataset(dataset_id) {
        Some(dataset) => {
            format!("A fonte '{}' está registrada, mas nenhum ativo .ocind foi importado. Importe-a antes de consultar.", dataset.title)
        }
        None => format!("A fonte '{dataset_id}' não está registrada."),
    }
}
