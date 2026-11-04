use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use opencnpj_core::RefTable;
use opencnpj_store::{postings::Field, reader::Snapshot};
use roaring::RoaringBitmap;

const REFERENCE_TABLES: [RefTable; 3] = [RefTable::Municipios, RefTable::Cnaes, RefTable::Naturezas];
const CODE_FIELDS: [Field; 3] = [Field::Municipio, Field::Cnae, Field::CnaeSecundaria];

pub(super) fn extract(path: &Path, out: &Path) -> Result<()> {
    let snapshot = Snapshot::open(path).with_context(|| format!("open snapshot {}", path.display()))?;
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new().prefix(".rfb-").tempdir_in(parent)?;
    for table in REFERENCE_TABLES {
        let name = table.name();
        let rows = snapshot.refs.get(name).with_context(|| format!("snapshot has no {name} reference table"))?;
        let mut rows: Vec<_> = rows.iter().collect();
        rows.sort_unstable();
        let mut file = output(staging.path(), &format!("rfb_{name}.tsv"))?;
        writeln!(file, "code\tlabel")?;
        for [code, label] in rows {
            write_cell(&mut file, code)?;
            write!(file, "\t")?;
            write_cell(&mut file, label)?;
            writeln!(file)?;
        }
        file.flush()?;
    }
    write_usage(&snapshot, staging.path())?;
    fs::rename(staging.path(), out)
        .with_context(|| format!("publish extraction to {} (the directory must be absent or empty)", out.display()))?;
    Ok(())
}

fn output(dir: &Path, name: &str) -> Result<BufWriter<File>> {
    let path = dir.join(name);
    Ok(BufWriter::new(File::create(&path).with_context(|| format!("create {}", path.display()))?))
}

fn write_cell(writer: &mut impl Write, value: &str) -> std::io::Result<()> {
    if value.contains(['\t', '\r', '\n', '"']) {
        write!(writer, "\"{}\"", value.replace('"', "\"\""))
    } else {
        writer.write_all(value.as_bytes())
    }
}

fn write_usage(snapshot: &Snapshot, dir: &Path) -> Result<()> {
    let mut file = output(dir, "used_codes.tsv")?;
    writeln!(file, "code\tfield\tdocument_count\tuf_distribution")?;
    let ufs = snapshot
        .prefix_terms(Field::Uf, b"", usize::MAX)
        .into_iter()
        .map(|(uf, offset)| Ok((String::from_utf8(uf)?, snapshot.postings_by_offset(offset)?)))
        .collect::<Result<Vec<_>>>()?;
    let mut covered = RoaringBitmap::new();
    for (uf, docs) in &ufs {
        ensure!(!uf.is_empty(), "an empty UF is reserved for establishments without a UF term");
        ensure!(docs.is_disjoint(&covered), "overlapping postings for UF {uf}");
        covered |= docs.as_ref();
    }
    for field in CODE_FIELDS {
        for (code, offset) in snapshot.prefix_terms(field, b"", usize::MAX) {
            let code = String::from_utf8(code)?;
            let docs = snapshot.postings_by_offset(offset)?;
            if docs.is_empty() {
                continue;
            }
            let mut distribution = BTreeMap::new();
            for (uf, uf_docs) in &ufs {
                let count = docs.intersection_len(uf_docs);
                if count > 0 {
                    distribution.insert(uf.as_str(), count);
                }
            }
            let without_uf = docs.len() - distribution.values().sum::<u64>();
            if without_uf > 0 {
                distribution.insert("", without_uf);
            }
            write_cell(&mut file, &code)?;
            write!(file, "\t{}\t{}\t", field.name(), docs.len())?;
            serde_json::to_writer(&mut file, &distribution)?;
            writeln!(file)?;
        }
    }
    file.flush()?;
    Ok(())
}
