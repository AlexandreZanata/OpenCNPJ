//! JSON manifest embedded in every snapshot: schema, provenance, counts and build settings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::compress::BlockCodec;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub format: String,
    pub format_version: [u16; 2],
    /// Competence of the source data, `YYYY-MM`.
    pub competence: String,
    /// RFB snapshot id found in member names (e.g. `D61010`), when known.
    pub source_snapshot: Option<String>,
    pub created_by: String,
    pub profile: String,
    pub block_codec: BlockCodec,
    pub block_target_bytes: usize,
    pub zstd_dictionary_bytes: usize,
    pub schema: Vec<TableSchema>,
    pub sources: Vec<SourceFile>,
    pub counts: Counts,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TableSchema {
    pub table: String,
    pub columns: Vec<ColumnSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColumnSchema {
    pub name: String,
    pub kind: opencnpj_core::Kind,
    /// Entries in the global dictionary (Dict columns only).
    pub dictionary_len: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    /// CSV member inside the ZIP.
    pub member: Option<String>,
    pub member_bytes: Option<u64>,
    pub rows: u64,
    pub rejected_lines: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Counts {
    /// Parsed rows per fact table (every source row is either here or in `rejected_lines`).
    pub rows: BTreeMap<String, u64>,
    pub rejected_lines: u64,
    pub companies: u64,
    pub documents: u64,
    pub blocks: u64,
    /// Companies present in a table other than `empresas` but without an `empresas` row.
    pub companies_without_empresa: u64,
    /// Same `cnpj_basico` appearing in more than one `empresas` row.
    pub duplicate_empresa_keys: u64,
    /// Same full CNPJ appearing in more than one `estabelecimentos` row.
    pub duplicate_estabelecimento_keys: u64,
    pub terms: u64,
    pub section_bytes: BTreeMap<String, u64>,
}
