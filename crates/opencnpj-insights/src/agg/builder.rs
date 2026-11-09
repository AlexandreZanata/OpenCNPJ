use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail, ensure};
use opencnpj_core::Table as SourceTable;
use opencnpj_query::ir::{Day, MunicipalityId, UfId};
use opencnpj_store::{Snapshot, docvalues::DocValuesColumn, numeric::NumField, postings::Field};
use rayon::prelude::*;

use super::{ActiveDetail, BLOCK_CELLS, Cell, Key, MUNICIPALITY_MISSING, Section, Table, UF_MISSING, flow_bucket, stock_detail, uf_id};
use crate::catalog::municipalities::MunicipalityCatalogue;

type Counts = HashMap<u64, u64>;

pub struct Build {
    pub sections: BTreeMap<Section, Vec<Cell>>,
    pub establishments: u64,
    pub missing_registration_date: u64,
    pub missing_closure_date: u64,
}

#[derive(Default)]
struct CountsByTable {
    cells: [Counts; 5],
    establishments: u64,
    missing_registration_date: u64,
    missing_closure_date: u64,
}

impl CountsByTable {
    fn add(&mut self, table: Table, key: Key) {
        *self.cells[table as usize - 1].entry(key.packed()).or_default() += 1;
    }

    fn merge(mut self, other: Self) -> Self {
        for (left, right) in self.cells.iter_mut().zip(other.cells) {
            merge_counts(left, right);
        }
        self.establishments += other.establishments;
        self.missing_registration_date += other.missing_registration_date;
        self.missing_closure_date += other.missing_closure_date;
        self
    }
}

pub fn build(snapshot: &Snapshot, catalogue: &MunicipalityCatalogue, tables: &[Table]) -> Result<Build> {
    ensure!(!tables.is_empty() && tables.windows(2).all(|pair| pair[0] < pair[1]), "select unique tables in order");
    let municipality_column = snapshot.doc_values(Field::Municipio).context("snapshot needs municipality doc values")?;
    let cnae_column = snapshot.doc_values(Field::Cnae).context("snapshot needs CNAE doc values")?;
    let mut municipalities = vec![MUNICIPALITY_MISSING];
    for code in &municipality_column.values {
        let tom = code.parse().with_context(|| format!("invalid municipality code {code:?}"))?;
        let id = catalogue.by_tom(tom).with_context(|| format!("TOM {code} is missing from the municipality catalogue"))?.id;
        ensure!(id != MUNICIPALITY_MISSING, "catalogue uses the reserved missing municipality id");
        municipalities.push(id);
    }
    let mut cnaes = vec![0];
    cnaes.extend(cnae_column.values.iter().map(|code| parse_cnae(code)).collect::<Result<Vec<_>>>()?);

    let head = field_values(snapshot, Field::MatrizFilial, 0, |code| match code {
        "1" => Some(1),
        "2" => Some(2),
        _ => None,
    })?;
    let uf = field_values(snapshot, Field::Uf, UF_MISSING, |code| uf_id(code).map(|uf| uf.0))?;
    let status = field_values(snapshot, Field::Situacao, 0, |code| match code {
        "01" => Some(1),
        "02" => Some(2),
        "03" => Some(3),
        "04" => Some(4),
        "08" => Some(5),
        _ => None,
    })?;
    let active_fields = if tables.contains(&Table::T1) {
        let porte = field_values(snapshot, Field::Porte, 0, |code| match code {
            "00" => Some(1),
            "01" => Some(2),
            "03" => Some(3),
            "05" => Some(4),
            _ => None,
        })?;
        Some((porte, field_values(snapshot, Field::Mei, 0, flag)?, field_values(snapshot, Field::Simples, 0, flag)?))
    } else {
        None
    };
    let count_t2 = tables.contains(&Table::T2);
    let opening = tables
        .contains(&Table::T3)
        .then(|| snapshot.numeric(NumField::InicioAtividade).context("snapshot needs opening dates"))
        .transpose()?;
    let closure =
        tables.contains(&Table::T4).then(|| snapshot.numeric(NumField::DataSituacao).context("snapshot needs status dates")).transpose()?;
    let buckets = if opening.is_some() || closure.is_some() { date_buckets() } else { Vec::new() };
    let bytes = snapshot.bytes();

    let mut counts = (0..snapshot.doc_count())
        .into_par_iter()
        .with_min_len(1 << 19)
        .try_fold(CountsByTable::default, |mut counts, doc| -> Result<_> {
            let i = doc as usize;
            if head[i] == 0 {
                return Ok(counts);
            }
            counts.establishments += 1;
            let mut key = Key {
                cnae: doc_value(cnae_column, &cnaes, bytes, doc)?,
                municipality: MunicipalityId(doc_value(municipality_column, &municipalities, bytes, doc)?),
                uf: UfId(uf[i]),
                detail: 0,
            };
            if status[i] == 2
                && let Some((porte, mei, simples)) = &active_fields
            {
                key.detail = ActiveDetail { head_office: head[i], porte: porte[i], mei: mei[i], simples: simples[i] }.pack();
                counts.add(Table::T1, key);
            }
            if count_t2 {
                key.detail = stock_detail(head[i], status[i]);
                counts.add(Table::T2, key);
            }
            if let Some(column) = opening {
                key.detail = date_bucket(&buckets, column.get(bytes, doc))?;
                if key.detail == 0 {
                    counts.missing_registration_date += 1;
                } else {
                    counts.add(Table::T3, key);
                }
            }
            if status[i] == 5
                && let Some(column) = closure
            {
                key.detail = date_bucket(&buckets, column.get(bytes, doc))?;
                if key.detail == 0 {
                    counts.missing_closure_date += 1;
                } else {
                    counts.add(Table::T4, key);
                }
            }
            Ok(counts)
        })
        .try_reduce(CountsByTable::default, |left, right| Ok(left.merge(right)))?;

    let expected = snapshot.manifest.counts.rows.get(SourceTable::Estabelecimentos.name()).copied().unwrap_or(0);
    ensure!(counts.establishments == expected, "matriz/filial postings cover {} of {expected} establishments", counts.establishments);
    if tables.contains(&Table::T5) {
        counts.cells[Table::T5 as usize - 1] = snapshot
            .prefix_terms(Field::CnaeSecundaria, b"", usize::MAX)
            .into_par_iter()
            .try_fold(Counts::default, |mut result, (code, offset)| -> Result<_> {
                let cnae = parse_cnae(std::str::from_utf8(&code)?)?;
                for doc in snapshot.postings_by_offset(offset)?.iter() {
                    let i = doc as usize;
                    if head[i] != 0 && status[i] == 2 {
                        let municipality = MunicipalityId(doc_value(municipality_column, &municipalities, bytes, doc)?);
                        let key = Key { cnae, municipality, uf: UfId(uf[i]), detail: 0 };
                        *result.entry(key.packed()).or_default() += 1;
                    }
                }
                Ok(result)
            })
            .try_reduce(Counts::default, |mut left, right| {
                merge_counts(&mut left, right);
                Ok(left)
            })?;
    }

    let mut sections = BTreeMap::new();
    for (table, cells) in Table::ALL.into_iter().zip(counts.cells) {
        if !tables.contains(&table) {
            continue;
        }
        if table != Table::T5 {
            let mut cnaes = Counts::new();
            let mut ufs = Counts::new();
            let mut municipalities = Counts::new();
            let mut uf_cnaes = Counts::new();
            for (&key, &count) in &cells {
                let key = Key::unpack(key);
                *cnaes.entry(u64::from(key.cnae)).or_default() += count;
                *ufs.entry(u64::from(key.uf.0)).or_default() += count;
                *municipalities.entry(u64::from(key.municipality.0)).or_default() += count;
                *uf_cnaes.entry(key.uf_cnae()).or_default() += count;
            }
            for (section, counts) in [
                (Section::Cnae(table), cnaes),
                (Section::Uf(table), ufs),
                (Section::Municipality(table), municipalities),
                (Section::UfCnae(table), uf_cnaes),
            ] {
                sections.insert(section, sorted(counts));
            }
        }
        let cells = sorted(cells);
        let mut offsets = Vec::new();
        let mut previous = None;
        for (index, cell) in cells.iter().enumerate() {
            let cnae = Key::unpack(cell.key).cnae;
            if previous != Some(cnae) {
                offsets.push(Cell { key: u64::from(cnae), count: (index / BLOCK_CELLS + 1) as u64 });
                previous = Some(cnae);
            }
        }
        sections.insert(Section::CnaeOffset(table), offsets);
        sections.insert(Section::Cells(table), cells);
    }
    Ok(Build {
        sections,
        establishments: counts.establishments,
        missing_registration_date: counts.missing_registration_date,
        missing_closure_date: counts.missing_closure_date,
    })
}

fn doc_value<T: Copy>(column: &DocValuesColumn, values: &[T], bytes: &[u8], doc: u32) -> Result<T> {
    let id = column.id(bytes, doc).context("document outside the forward column")?;
    values.get(id as usize).copied().context("invalid forward-column dictionary id")
}

fn field_values(snapshot: &Snapshot, field: Field, missing: u8, parse: impl Fn(&str) -> Option<u8>) -> Result<Vec<u8>> {
    let mut values = vec![missing; snapshot.doc_count() as usize];
    for (term, offset) in snapshot.prefix_terms(field, b"", usize::MAX) {
        let term = std::str::from_utf8(&term)?;
        let Some(value) = parse(term) else { bail!("unexpected {} term {term:?}", field.name()) };
        for doc in snapshot.postings_by_offset(offset)?.iter() {
            let slot = values.get_mut(doc as usize).context("postings document outside snapshot")?;
            ensure!(*slot == missing, "document {doc} has multiple {} values", field.name());
            *slot = value;
        }
    }
    Ok(values)
}

fn flag(value: &str) -> Option<u8> {
    match value {
        "N" => Some(1),
        "S" => Some(2),
        _ => None,
    }
}

fn parse_cnae(code: &str) -> Result<u32> {
    ensure!(code.len() == 7 && code.bytes().all(|b| b.is_ascii_digit()) && code != "0000000", "invalid CNAE subclass {code:?}");
    Ok(code.parse()?)
}

fn merge_counts(left: &mut Counts, mut right: Counts) {
    if left.len() < right.len() {
        std::mem::swap(left, &mut right);
    }
    for (key, count) in right {
        *left.entry(key).or_default() += count;
    }
}

fn sorted(counts: Counts) -> Vec<Cell> {
    let mut cells = counts.into_iter().map(|(key, count)| Cell { key, count }).collect::<Vec<_>>();
    cells.par_sort_unstable_by_key(|cell| cell.key);
    cells
}

fn date_buckets() -> Vec<u32> {
    let mut dates = vec![0; Day::from_ymd(9999, 12, 31).expect("valid final day").0 as usize + 1];
    let mut end = dates.len();
    for year in (1900..=9999).rev() {
        for month in (1..=12).rev() {
            let start = Day::from_ymd(year, month, 1).expect("valid first day").0 as usize;
            dates[start..end].fill(flow_bucket(year, month));
            end = start;
        }
    }
    dates
}

fn date_bucket(dates: &[u32], day: u64) -> Result<u32> {
    let index = usize::try_from(day)?;
    dates.get(index).copied().context("invalid snapshot date value")
}
