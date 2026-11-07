use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use opencnpj_ingest::source::sha256_file;
use opencnpj_insights::{
    agg::{
        self, ActiveDetail, Aggregate, Cell, Key, MUNICIPALITY_MISSING, Manifest, Section, Table, UF_MISSING, flow_bounds, uf_code, uf_id,
        unpack_stock_detail,
    },
    catalog::municipalities::MunicipalityCatalogue,
};
use opencnpj_query::ir::{Competence, UfId};
use opencnpj_store::{
    Snapshot,
    numeric::{NumField, Range},
    postings::Field,
    search::{self, Limits, Query, RangeFilter},
};
use roaring::RoaringBitmap;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Build an aggregate artifact from a verified snapshot.
    Build {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "T1,T2,T3,T4,T5")]
        tables: String,
    },
}

#[derive(Args)]
pub(crate) struct DiffArgs {
    #[arg(long)]
    agg: PathBuf,
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    catalog: PathBuf,
    #[arg(long, default_value_t = 10000)]
    sample: usize,
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

pub(crate) fn run(command: Command) -> Result<()> {
    match command {
        Command::Build { snapshot, catalog, out, tables } => build(&snapshot, &catalog, &out, &tables),
    }
}

fn build(snapshot_path: &Path, catalogue_path: &Path, out: &Path, selection: &str) -> Result<()> {
    let mut tables = selection
        .split(',')
        .map(|name| match name {
            "T1" => Ok(Table::T1),
            "T2" => Ok(Table::T2),
            "T3" => Ok(Table::T3),
            "T4" => Ok(Table::T4),
            "T5" => Ok(Table::T5),
            _ => bail!("unknown aggregate table {name:?}"),
        })
        .collect::<Result<Vec<_>>>()?;
    tables.sort_unstable();
    let mut sha_path = out.as_os_str().to_owned();
    sha_path.push(".sha256");
    let sha_path = PathBuf::from(sha_path);
    ensure!(!out.exists() && !sha_path.exists(), "aggregate or checksum already exists");
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let snapshot_sha256 = sha256_file(snapshot_path)?;
    let municipality_sha256 = sha256_file(catalogue_path)?;
    let snapshot = Snapshot::open(snapshot_path)?;
    let catalogue = MunicipalityCatalogue::load(catalogue_path)?;
    let result = agg::builder::build(&snapshot, &catalogue, &tables)?;
    let (year, month) = snapshot.manifest.competence.split_once('-').context("snapshot competence must be YYYY-MM")?;
    let competence = Competence::from_year_month(year.parse()?, month.parse()?).context("invalid snapshot competence")?;
    let manifest = Manifest {
        competence,
        snapshot_sha256,
        municipality_sha256,
        tables,
        establishments: result.establishments,
        missing_registration_date: result.missing_registration_date,
        missing_closure_date: result.missing_closure_date,
    };
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    agg::write(BufWriter::new(file.as_file_mut()), &manifest, &result.sections)?;
    file.as_file().sync_all()?;
    let sha256 = sha256_file(file.path())?;
    let bytes = file.as_file().metadata()?.len();
    let name = out.file_name().and_then(|name| name.to_str()).context("aggregate filename must be UTF-8")?;
    let mut sidecar = tempfile::NamedTempFile::new_in(parent)?;
    writeln!(sidecar, "{sha256}  {name}")?;
    sidecar.as_file().sync_all()?;
    sidecar.persist_noclobber(&sha_path)?;
    if let Err(error) = file.persist_noclobber(out) {
        fs::remove_file(&sha_path).with_context(|| format!("aggregate publish failed ({error}), then checksum cleanup failed"))?;
        return Err(error.into());
    }
    println!("competence {competence}, establishments {}, {} bytes, SHA-256 {sha256}", manifest.establishments, bytes);
    for &table in &manifest.tables {
        let cells = &result.sections[&Section::Cells(table)];
        println!("{table:?}: {} cells, {} counts", cells.len(), cells.iter().map(|cell| cell.count).sum::<u64>());
    }
    println!(
        "missing registration dates: {}, missing closure dates: {}",
        manifest.missing_registration_date, manifest.missing_closure_date
    );
    Ok(())
}

pub(crate) fn diff(args: DiffArgs) -> Result<()> {
    ensure!(args.sample > 0, "sample must be positive");
    let aggregate = Aggregate::open(&args.agg)?;
    let manifest = aggregate.manifest();
    ensure!(sha256_file(&args.snapshot)? == manifest.snapshot_sha256, "aggregate belongs to another snapshot");
    ensure!(sha256_file(&args.catalog)? == manifest.municipality_sha256, "aggregate uses another municipality catalogue");
    let snapshot = Snapshot::open(&args.snapshot)?;
    let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
    snapshot.set_postings_cache_bytes(512 << 20);
    let mut oracle = Oracle { snapshot: &snapshot, catalogue: &catalogue, missing: HashMap::new() };
    let mut rng = SplitMix(args.seed);
    let mut total = 0;
    for (index, &table) in manifest.tables.iter().enumerate() {
        if table != Table::T5 {
            oracle.check_marginals(&aggregate, table)?;
        }
        let per_table = args.sample / manifest.tables.len() + usize::from(index < args.sample % manifest.tables.len());
        let sample = sample_cells(&aggregate, table, per_table, &mut rng)?;
        for cell in &sample {
            let actual = oracle.count(table, Key::unpack(cell.key))?;
            ensure!(actual == cell.count, "{table:?} key {:?}: aggregate {}, engine {actual}", Key::unpack(cell.key), cell.count);
        }
        total += sample.len();
        println!("{table:?}: {} sampled cells match the engine", sample.len());
    }
    println!("seed {}, {total} sampled cells, 0 mismatches", args.seed);
    Ok(())
}

struct Oracle<'a> {
    snapshot: &'a Snapshot,
    catalogue: &'a MunicipalityCatalogue,
    missing: HashMap<Field, RoaringBitmap>,
}

impl Oracle<'_> {
    fn check_marginals(&self, aggregate: &Aggregate, table: Table) -> Result<()> {
        let mut query = table_query(table);
        if let Some(field) = date_field(table) {
            query.ranges.push(RangeFilter { field, range: Some(Range { lo: 1, hi: u64::MAX }) });
        }
        let total = search::candidates(self.snapshot, &query, &Limits::DEFAULT)?.all.context("missing table query filter")?.len();
        for (field, section, missing) in [
            (Field::Cnae, Section::Cnae(table), 0),
            (Field::Uf, Section::Uf(table), u64::from(UF_MISSING)),
            (Field::Municipio, Section::Municipality(table), u64::from(MUNICIPALITY_MISSING)),
        ] {
            let mut expected = BTreeMap::new();
            for (code, count) in search::group_counts(self.snapshot, &query, field, &Limits::DEFAULT)? {
                let key = match field {
                    Field::Cnae => code.parse::<u64>()?,
                    Field::Uf => u64::from(uf_id(&code).context("unknown UF in oracle")?.0),
                    Field::Municipio => u64::from(self.catalogue.by_tom(code.parse()?).context("unknown TOM in oracle")?.id),
                    _ => unreachable!(),
                };
                ensure!(expected.insert(key, count).is_none(), "duplicate {field:?} key in engine");
            }
            let absent = total.checked_sub(expected.values().sum()).context("group counts exceed the engine total")?;
            if absent > 0 {
                expected.insert(missing, absent);
            }
            let mut actual = BTreeMap::new();
            for cell in aggregate.cells(section) {
                let Cell { key, count } = cell?;
                actual.insert(key, count);
            }
            for key in expected.keys().chain(actual.keys()) {
                ensure!(
                    expected.get(key) == actual.get(key),
                    "{section:?} key {key}: aggregate {:?}, engine {:?}",
                    actual.get(key),
                    expected.get(key)
                );
            }
            println!("{section:?}: {} groups, {total} counts, match the engine", actual.len());
        }
        self.check_uf_cnae(aggregate, table, &query)
    }

    /// UF × main-CNAE marginals: one engine group-by on CNAE per UF. Keys with a missing UF
    /// are skipped (the engine cannot filter on "no UF"); keys with a missing CNAE carry the
    /// UF's remainder.
    fn check_uf_cnae(&self, aggregate: &Aggregate, table: Table, query: &Query) -> Result<()> {
        let mut actual = BTreeMap::new();
        for cell in aggregate.cells(Section::UfCnae(table)) {
            let Cell { key, count } = cell?;
            if (key >> 24) as u8 != UF_MISSING {
                actual.insert(key, count);
            }
        }
        let mut expected = BTreeMap::new();
        for uf in 0..UF_MISSING {
            let Some(code) = uf_code(UfId(uf)) else { continue };
            let mut per_uf = query.clone();
            per_uf.filters.push((Field::Uf, vec![code.to_string()]));
            let Some(docs) = search::candidates(self.snapshot, &per_uf, &Limits::DEFAULT)?.all else { continue };
            if docs.is_empty() {
                continue;
            }
            let base = u64::from(uf) << 24;
            let mut with_cnae = 0;
            for (cnae, count) in search::group_counts(self.snapshot, &per_uf, Field::Cnae, &Limits::DEFAULT)? {
                expected.insert(base | cnae.parse::<u64>()?, count);
                with_cnae += count;
            }
            let absent = docs.len().checked_sub(with_cnae).context("CNAE groups exceed the UF total")?;
            if absent > 0 {
                expected.insert(base, absent);
            }
        }
        for key in expected.keys().chain(actual.keys()) {
            ensure!(
                expected.get(key) == actual.get(key),
                "UfCnae({table:?}) key uf {} cnae {}: aggregate {:?}, engine {:?}",
                key >> 24,
                key & 0xFF_FFFF,
                actual.get(key),
                expected.get(key)
            );
        }
        println!("UfCnae({table:?}): {} groups match the engine", actual.len());
        Ok(())
    }

    fn count(&mut self, table: Table, key: Key) -> Result<u64> {
        let mut query = table_query(table);
        let mut missing = Vec::new();
        let cnae_field = if table == Table::T5 { Field::CnaeSecundaria } else { Field::Cnae };
        add_optional(&mut query, &mut missing, cnae_field, (key.cnae != 0).then(|| format!("{:07}", key.cnae)));
        add_optional(&mut query, &mut missing, Field::Uf, uf_code(key.uf).map(str::to_owned));
        let municipality = if key.municipality.0 == MUNICIPALITY_MISSING {
            None
        } else {
            Some(format!("{:04}", self.catalogue.get(key.municipality.0).context("invalid aggregate municipality")?.tom))
        };
        add_optional(&mut query, &mut missing, Field::Municipio, municipality);
        match table {
            Table::T1 => {
                let detail = ActiveDetail::unpack(key.detail);
                add_optional(&mut query, &mut missing, Field::MatrizFilial, code_at(&["", "1", "2"], detail.head_office)?);
                add_optional(&mut query, &mut missing, Field::Porte, code_at(&["", "00", "01", "03", "05"], detail.porte)?);
                add_optional(&mut query, &mut missing, Field::Mei, code_at(&["", "N", "S"], detail.mei)?);
                add_optional(&mut query, &mut missing, Field::Simples, code_at(&["", "N", "S"], detail.simples)?);
            }
            Table::T2 => {
                let (head, status) = unpack_stock_detail(key.detail);
                add_optional(&mut query, &mut missing, Field::MatrizFilial, code_at(&["", "1", "2"], head)?);
                add_optional(&mut query, &mut missing, Field::Situacao, code_at(&["", "01", "02", "03", "04", "08"], status)?);
            }
            Table::T3 | Table::T4 => {
                let (from, to) = flow_bounds(key.detail).context("invalid aggregate flow bucket")?;
                query.ranges.push(RangeFilter {
                    field: date_field(table).expect("flow table"),
                    range: Some(Range { lo: u64::from(from.0), hi: u64::from(to.0) }),
                });
            }
            Table::T5 => {}
        }
        let mut docs = search::candidates(self.snapshot, &query, &Limits::DEFAULT)?.all.context("missing engine cell filters")?;
        for field in missing {
            let absent = match self.missing.entry(field) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mut absent = RoaringBitmap::from_iter(0..self.snapshot.doc_count());
                    for (_, offset) in self.snapshot.prefix_terms(field, b"", usize::MAX) {
                        absent -= self.snapshot.postings_by_offset(offset)?.as_ref();
                    }
                    entry.insert(absent)
                }
            };
            docs &= &*absent;
        }
        Ok(docs.len())
    }
}

fn table_query(table: Table) -> Query {
    let mut query = Query { filters: vec![(Field::MatrizFilial, vec!["1".into(), "2".into()])], ..Query::default() };
    let status = match table {
        Table::T1 | Table::T5 => Some("02"),
        Table::T4 => Some("08"),
        _ => None,
    };
    if let Some(status) = status {
        query.filters.push((Field::Situacao, vec![status.into()]));
    }
    query
}

fn date_field(table: Table) -> Option<NumField> {
    match table {
        Table::T3 => Some(NumField::InicioAtividade),
        Table::T4 => Some(NumField::DataSituacao),
        _ => None,
    }
}

fn add_optional(query: &mut Query, missing: &mut Vec<Field>, field: Field, value: Option<String>) {
    if let Some(value) = value {
        query.filters.push((field, vec![value]));
    } else {
        missing.push(field);
    }
}

fn code_at(codes: &[&str], index: u8) -> Result<Option<String>> {
    let code = codes.get(usize::from(index)).context("invalid categorical value in aggregate")?;
    Ok((!code.is_empty()).then(|| (*code).into()))
}

fn sample_cells(aggregate: &Aggregate, table: Table, size: usize, rng: &mut SplitMix) -> Result<Vec<Cell>> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let mut sample = Vec::new();
    for (index, cell) in aggregate.cells(Section::Cells(table)).enumerate() {
        let cell = cell?;
        if sample.len() < size {
            sample.push(cell);
        } else {
            let slot = (rng.next() % (index as u64 + 1)) as usize;
            if slot < size {
                sample[slot] = cell;
            }
        }
    }
    Ok(sample)
}

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
}
