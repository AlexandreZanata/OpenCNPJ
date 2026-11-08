//! Immutable record-level official-data store (`.ocrec`).
//!
//! Indicators use `.ocind`; record-level sources (fleet aggregates, health
//! establishments, schools, debt records) use this SQLite container. The store
//! keeps source-specific fields as JSON exactly as normalized by the adapter,
//! plus a small common identity layer for geography and CNPJ joins.

use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RECORD_EXTENSION: &str = "ocrec";
pub const RECORD_FORMAT_VERSION: u32 = 1;

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
    period TEXT NOT NULL,
    coverage TEXT NOT NULL,
    raw_sha256 TEXT NOT NULL,
    raw_bytes INTEGER NOT NULL,
    row_count INTEGER NOT NULL,
    accepted_rows INTEGER NOT NULL,
    out_of_scope_rows INTEGER NOT NULL,
    rejected_rows INTEGER NOT NULL,
    duplicate_rows INTEGER NOT NULL,
    state TEXT NOT NULL,
    schema_json TEXT NOT NULL,
    limitations_json TEXT NOT NULL
);
CREATE TABLE records (
    dataset_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    geography_level TEXT NOT NULL,
    geography_code TEXT NOT NULL,
    geography_name TEXT NOT NULL,
    cnpj TEXT NOT NULL,
    cnpj_root TEXT NOT NULL,
    period TEXT NOT NULL,
    data_json TEXT NOT NULL,
    PRIMARY KEY (dataset_id, record_id)
);
CREATE INDEX records_geography ON records(geography_level, geography_code);
CREATE INDEX records_cnpj ON records(cnpj);
CREATE INDEX records_cnpj_root ON records(cnpj_root);
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordManifest {
    pub format_version: u32,
    pub source_id: String,
    pub dataset_id: String,
    pub title: String,
    pub publisher: String,
    pub source_url: String,
    pub metadata_url: String,
    pub licence: String,
    pub licence_url: String,
    pub licence_status: String,
    pub attribution: String,
    pub retrieved_at: String,
    pub period: String,
    pub coverage: String,
    pub raw_sha256: String,
    pub raw_bytes: u64,
    pub row_count: u64,
    pub accepted_rows: u64,
    pub out_of_scope_rows: u64,
    pub rejected_rows: u64,
    pub duplicate_rows: u64,
    pub state: String,
    pub schema: serde_json::Value,
    pub limitations: Vec<String>,
}

impl RecordManifest {
    pub fn validate_metadata(&self) -> Result<()> {
        ensure!(self.format_version == RECORD_FORMAT_VERSION, "unsupported record format version {}", self.format_version);
        for (name, value) in [
            ("source_id", &self.source_id),
            ("dataset_id", &self.dataset_id),
            ("title", &self.title),
            ("publisher", &self.publisher),
            ("source_url", &self.source_url),
            ("licence", &self.licence),
            ("licence_url", &self.licence_url),
            ("licence_status", &self.licence_status),
            ("attribution", &self.attribution),
            ("retrieved_at", &self.retrieved_at),
            ("coverage", &self.coverage),
        ] {
            ensure!(!value.trim().is_empty(), "empty record manifest field {name}");
        }
        ensure!(self.source_url.starts_with("https://") || self.source_url.starts_with("http://"), "record source URL must be HTTP(S)");
        ensure!(self.licence_url.starts_with("https://") || self.licence_url.starts_with("http://"), "licence URL must be HTTP(S)");
        ensure!(!self.state.is_empty(), "record manifest has no state");
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_metadata()?;
        ensure!(
            self.row_count == self.accepted_rows + self.out_of_scope_rows + self.rejected_rows + self.duplicate_rows,
            "record row accounting mismatch: total {}, accepted {}, out_of_scope {}, rejected {}, duplicate {}",
            self.row_count,
            self.accepted_rows,
            self.out_of_scope_rows,
            self.rejected_rows,
            self.duplicate_rows
        );
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub record_id: String,
    pub geography_level: String,
    pub geography_code: String,
    pub geography_name: String,
    pub cnpj: String,
    pub cnpj_root: String,
    pub period: String,
    pub data: serde_json::Value,
}

impl Record {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.record_id.trim().is_empty(), "record has an empty ID");
        ensure!(!self.record_id.contains('\u{0}'), "record ID has a NUL byte");
        Ok(())
    }
}

pub struct RecordStore {
    path: PathBuf,
    connection: Connection,
    manifest: RecordManifest,
}

#[derive(Debug, Clone)]
pub struct PublishedRecords {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
    pub manifest: RecordManifest,
}

impl RecordStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("open record store {}", path.display()))?;
        let manifest = read_manifest(&connection).with_context(|| format!("read manifest of {}", path.display()))?;
        manifest.validate()?;
        Ok(Self { path, connection, manifest })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn manifest(&self) -> &RecordManifest {
        &self.manifest
    }

    pub fn record_count(&self) -> Result<u64> {
        let count: i64 = self.connection.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))?;
        u64::try_from(count).context("negative record count")
    }

    pub fn records(&self) -> Result<Vec<Record>> {
        let mut statement = self.connection.prepare(
            "SELECT record_id, geography_level, geography_code, geography_name, cnpj, cnpj_root, period, data_json
             FROM records ORDER BY record_id",
        )?;
        let rows = statement.query_map([], record_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().with_context(|| format!("read records from {}", self.path.display()))
    }

    pub fn records_for_geography(&self, level: &str, code: &str) -> Result<Vec<Record>> {
        let mut statement = self.connection.prepare(
            "SELECT record_id, geography_level, geography_code, geography_name, cnpj, cnpj_root, period, data_json
             FROM records WHERE geography_level = ?1 AND geography_code = ?2 ORDER BY record_id",
        )?;
        let rows = statement.query_map(params![level, code], record_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().with_context(|| format!("read records from {}", self.path.display()))
    }

    pub fn records_for_cnpj(&self, cnpj: &str) -> Result<Vec<Record>> {
        let mut statement = self.connection.prepare(
            "SELECT record_id, geography_level, geography_code, geography_name, cnpj, cnpj_root, period, data_json
             FROM records WHERE cnpj = ?1 ORDER BY record_id",
        )?;
        let rows = statement.query_map(params![cnpj], record_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().with_context(|| format!("read records from {}", self.path.display()))
    }

    pub fn verify(&self) -> Result<()> {
        let check: String = self.connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        ensure!(check == "ok", "SQLite integrity check failed for {}: {check}", self.path.display());
        self.manifest.validate()?;
        let rows = self.record_count()?;
        ensure!(
            rows == self.manifest.accepted_rows,
            "{} has {rows} records but its manifest declares {}",
            self.path.display(),
            self.manifest.accepted_rows
        );
        Ok(())
    }
}

fn read_manifest(connection: &Connection) -> Result<RecordManifest> {
    connection
        .query_row(
            "SELECT format_version, source_id, dataset_id, title, publisher, source_url, metadata_url,
                    licence, licence_url, licence_status, attribution, retrieved_at, period, coverage,
                    raw_sha256, raw_bytes, row_count, accepted_rows, out_of_scope_rows, rejected_rows,
                    duplicate_rows, state, schema_json, limitations_json
             FROM manifest WHERE id = 1",
            [],
            |row| {
                let schema_json: String = row.get(22)?;
                let limitations_json: String = row.get(23)?;
                Ok(RecordManifest {
                    format_version: row.get(0)?,
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
                    period: row.get(12)?,
                    coverage: row.get(13)?,
                    raw_sha256: row.get(14)?,
                    raw_bytes: u64::try_from(row.get::<_, i64>(15)?).unwrap_or(0),
                    row_count: u64::try_from(row.get::<_, i64>(16)?).unwrap_or(0),
                    accepted_rows: u64::try_from(row.get::<_, i64>(17)?).unwrap_or(0),
                    out_of_scope_rows: u64::try_from(row.get::<_, i64>(18)?).unwrap_or(0),
                    rejected_rows: u64::try_from(row.get::<_, i64>(19)?).unwrap_or(0),
                    duplicate_rows: u64::try_from(row.get::<_, i64>(20)?).unwrap_or(0),
                    state: row.get(21)?,
                    schema: serde_json::from_str(&schema_json).unwrap_or(serde_json::Value::Null),
                    limitations: serde_json::from_str(&limitations_json).unwrap_or_default(),
                })
            },
        )
        .optional()?
        .context("missing record manifest row")
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Record> {
    let data_json: String = row.get(7)?;
    Ok(Record {
        record_id: row.get(0)?,
        geography_level: row.get(1)?,
        geography_code: row.get(2)?,
        geography_name: row.get(3)?,
        cnpj: row.get(4)?,
        cnpj_root: row.get(5)?,
        period: row.get(6)?,
        data: serde_json::from_str(&data_json).unwrap_or(serde_json::Value::Null),
    })
}

/// Publish an immutable record store. `manifest.accepted_rows` and
/// `manifest.duplicate_rows` are computed from the inserted records.
pub fn write_records(
    path: impl AsRef<Path>,
    manifest: &RecordManifest,
    records: impl IntoIterator<Item = Record>,
) -> Result<PublishedRecords> {
    manifest.validate_metadata()?;
    let path = path.as_ref().to_path_buf();
    ensure!(!path.exists(), "refusing to overwrite existing record store {}", path.display());
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let file = tempfile::NamedTempFile::new_in(parent)?;
    let mut final_manifest = manifest.clone();
    {
        let mut connection = Connection::open(file.path())?;
        connection.pragma_update(None, "journal_mode", "OFF")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.execute_batch(SCHEMA)?;
        let transaction = connection.transaction()?;
        let mut inserted = 0_u64;
        let mut duplicates = 0_u64;
        {
            let mut statement = transaction.prepare(
                "INSERT OR IGNORE INTO records
                 (dataset_id, record_id, geography_level, geography_code, geography_name, cnpj, cnpj_root, period, data_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for record in records {
                record.validate()?;
                let changed = statement.execute(params![
                    final_manifest.dataset_id,
                    record.record_id,
                    record.geography_level,
                    record.geography_code,
                    record.geography_name,
                    record.cnpj,
                    record.cnpj_root,
                    record.period,
                    serde_json::to_string(&record.data)?,
                ])?;
                if changed == 1 {
                    inserted += 1;
                } else {
                    duplicates += 1;
                }
            }
        }
        final_manifest.accepted_rows = inserted;
        final_manifest.duplicate_rows = duplicates;
        final_manifest.validate()?;
        insert_manifest(&transaction, &final_manifest)?;
        transaction.commit()?;
        let check: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        ensure!(check == "ok", "new record store failed integrity check: {check}");
        connection.close().map_err(|(_, error)| error)?;
    }
    file.as_file().sync_all()?;
    let bytes = file.as_file().metadata()?.len();
    let sha256 = sha256_file(file.path())?;
    file.persist_noclobber(&path).with_context(|| format!("publish {}", path.display()))?;
    Ok(PublishedRecords { path, sha256, bytes, manifest: final_manifest })
}

fn insert_manifest(connection: &Connection, manifest: &RecordManifest) -> Result<()> {
    connection.execute(
        "INSERT INTO manifest VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
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
            manifest.period,
            manifest.coverage,
            manifest.raw_sha256,
            i64::try_from(manifest.raw_bytes)?,
            i64::try_from(manifest.row_count)?,
            i64::try_from(manifest.accepted_rows)?,
            i64::try_from(manifest.out_of_scope_rows)?,
            i64::try_from(manifest.rejected_rows)?,
            i64::try_from(manifest.duplicate_rows)?,
            manifest.state,
            serde_json::to_string(&manifest.schema)?,
            serde_json::to_string(&manifest.limitations)?,
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

/// Group record counts for compact coverage reports.
pub fn count_by_geography(records: &[Record]) -> BTreeMap<(String, String), usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry((record.geography_level.clone(), record.geography_code.clone())).or_default() += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(dataset: &str, rows: u64) -> RecordManifest {
        RecordManifest {
            format_version: 1,
            source_id: "test".into(),
            dataset_id: dataset.into(),
            title: dataset.into(),
            publisher: "Test".into(),
            source_url: "https://example.test/data".into(),
            metadata_url: "https://example.test/meta".into(),
            licence: "test".into(),
            licence_url: "https://example.test/licence".into(),
            licence_status: "review_required".into(),
            attribution: "Test".into(),
            retrieved_at: "unix:0".into(),
            period: "2026".into(),
            coverage: "MT".into(),
            raw_sha256: "0".repeat(64),
            raw_bytes: 1,
            row_count: rows,
            accepted_rows: 0,
            out_of_scope_rows: 0,
            rejected_rows: 0,
            duplicate_rows: 0,
            state: "queryable".into(),
            schema: serde_json::json!({"columns":["a"]}),
            limitations: vec![],
        }
    }

    fn record(id: &str) -> Record {
        Record {
            record_id: id.into(),
            geography_level: "N6".into(),
            geography_code: "5107925".into(),
            geography_name: "Sorriso".into(),
            cnpj: "12345678000199".into(),
            cnpj_root: "12345678".into(),
            period: "2026".into(),
            data: serde_json::json!({"id": id}),
        }
    }

    #[test]
    fn store_roundtrip_and_duplicate_accounting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.ocrec");
        let published = write_records(&path, &manifest("test", 3), vec![record("a"), record("b"), record("a")]).unwrap();
        assert_eq!(published.manifest.accepted_rows, 2);
        assert_eq!(published.manifest.duplicate_rows, 1);
        assert_eq!(published.manifest.row_count, 3);
        let store = RecordStore::open(&path).unwrap();
        store.verify().unwrap();
        assert_eq!(store.record_count().unwrap(), 2);
        assert_eq!(store.records_for_cnpj("12345678000199").unwrap().len(), 2);
    }

    #[test]
    fn store_refuses_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.ocrec");
        write_records(&path, &manifest("test", 1), vec![record("a")]).unwrap();
        let before = fs::read(&path).unwrap();
        assert!(write_records(&path, &manifest("test", 1), vec![record("b")]).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
