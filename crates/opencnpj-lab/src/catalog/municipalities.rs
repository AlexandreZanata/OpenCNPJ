mod sources;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use opencnpj_core::{RefTable, text::latin1_to_string};
use opencnpj_ingest::{parse::split_fields, source::sha256_file};
use opencnpj_insights::catalog::municipalities::{FOREIGN_TOM, Geography, Municipality, MunicipalityCatalogue, Place, fold_name};
use opencnpj_store::{Snapshot, postings::Field};
use serde_json::json;

const OVERRIDES: &str = include_str!("../../../opencnpj-insights/catalog/overrides.tsv");
const ALIASES: &str = include_str!("../../../opencnpj-insights/catalog/municipality_aliases.tsv");
const ARTIFACTS: [&str; 3] = ["municipalities.tsv", "overrides.tsv", "rfb-name-differences.tsv"];
// IBGE includes these state operational areas in the 2024 boundary workbook.
const OPERATIONAL_AREAS: [u32; 2] = [4300001, 4300002];

struct Identity {
    ibge: Option<u32>,
    uf: String,
    source_name: String,
}

pub(super) fn build(snapshot: &Path, raw: &Path, out: &Path) -> Result<()> {
    let (source_manifest, estimate) = sources::manifest(raw)?;
    let snapshot_sha = sha256_file(snapshot)?;
    let snapshot = Snapshot::open(snapshot)?;
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new().prefix(".municipalities-").tempdir_in(parent)?;
    let scratch = staging.path();

    let localities = sources::localities(&raw.join("ibge-municipios.json"))?;
    let (census_year, census) = sources::population(&raw.join("ibge-census-2022.json"), "93")?;
    ensure!(census_year == 2022, "expected the 2022 census");
    let (estimate_year, estimates) = sources::population(&estimate, "9324")?;
    let seats = sources::seats(&raw.join("ibge-localities-2022-gpkg.zip"), scratch)?;
    let neighbours = sources::neighbours(&raw.join("ibge-neighbours-2024.ods"))?;
    let aliases = aliases()?;
    let mut identities = siafi(&raw.join("tabmun.csv"))?;
    identities.extend(overrides()?);
    identities.retain(|_, row| row.ibge.is_none_or(|id| localities.contains_key(&id)));

    let used = snapshot.prefix_terms(Field::Municipio, b"", usize::MAX);
    for (code, _) in &used {
        let code = std::str::from_utf8(code)?;
        ensure!(identities.contains_key(&tom_code(code)?), "used TOM {code} needs an official mapping or override");
    }
    let ids = identities
        .values()
        .enumerate()
        .filter_map(|(index, row)| row.ibge.map(|ibge| Ok((ibge, u16::try_from(index)?))))
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure!(ids.keys().eq(localities.keys()), "official locality missing a TOM mapping");
    ensure!(aliases.keys().all(|id| ids.contains_key(id)), "alias refers to an unknown IBGE municipality");
    ensure!(
        neighbours.keys().all(|id| ids.contains_key(id) || OPERATIONAL_AREAS.contains(id)),
        "IBGE neighbour workbook contains an unclassified geographic code"
    );

    let refs = snapshot.refs.get(RefTable::Municipios.name()).context("snapshot has no municipality reference table")?;
    let mut rfb_names = BTreeMap::new();
    for [code, label] in refs {
        ensure!(rfb_names.insert(tom_code(code)?, label.as_str()).is_none(), "duplicate RFB municipality reference {code}");
    }
    let mut differences = BufWriter::new(File::create(scratch.join(ARTIFACTS[2]))?);
    writeln!(differences, "tom\tibge\tuf\trfb_name\tibge_name")?;
    let mut difference_count = 0;
    let mut rows = Vec::with_capacity(identities.len());
    for (index, (tom, identity)) in identities.into_iter().enumerate() {
        let id = u16::try_from(index)?;
        let mut variants = BTreeSet::from([fold_name(&identity.source_name)]);
        let rfb_name = rfb_names.get(&tom);
        if let Some(name) = rfb_name {
            variants.insert(fold_name(name));
        }
        let (name, place) = if let Some(ibge) = identity.ibge {
            let locality = &localities[&ibge];
            ensure!(identity.uf == locality.immediate.intermediate.uf.abbreviation, "UF mismatch for TOM {tom:04} / IBGE {ibge}");
            let seat = seats.get(&ibge).with_context(|| format!("missing official seat for {ibge}"))?;
            let neighbours = neighbours
                .get(&ibge)
                .with_context(|| format!("missing official neighbour row for {ibge}"))?
                .iter()
                .filter(|code| !OPERATIONAL_AREAS.contains(code))
                .map(|code| ids.get(code).copied().with_context(|| format!("unknown neighbour {code} of {ibge}")))
                .collect::<Result<BTreeSet<_>>>()?;
            if let Some(extra) = aliases.get(&ibge) {
                variants.extend(extra.iter().map(|name| fold_name(name)));
            }
            if let Some(rfb_name) = rfb_name.filter(|rfb_name| fold_name(rfb_name) != fold_name(&locality.name)) {
                write!(differences, "{tom:04}\t{ibge}\t{}\t", identity.uf)?;
                super::rfb::write_cell(&mut differences, rfb_name)?;
                write!(differences, "\t")?;
                super::rfb::write_cell(&mut differences, &locality.name)?;
                writeln!(differences)?;
                difference_count += 1;
            }
            let geography = Geography {
                ibge,
                uf: identity.uf,
                census_2022: census.get(&ibge).copied(),
                population_estimate: *estimates.get(&ibge).with_context(|| format!("missing official estimate for {ibge}"))?,
                estimate_year,
                latitude: seat.latitude,
                longitude: seat.longitude,
                neighbours: neighbours.into_iter().collect(),
                immediate_region: locality.immediate.id,
                intermediate_region: locality.immediate.intermediate.id,
                capital: seat.capital,
            };
            (locality.name.clone(), Place::Municipality(geography))
        } else {
            ensure!(tom == FOREIGN_TOM && identity.uf == "EX", "invalid foreign override for {tom:04}");
            (identity.source_name, Place::Abroad)
        };
        let folded = fold_name(&name);
        variants.remove("");
        variants.remove(&folded);
        rows.push(Municipality { id, tom, name, folded, variants: variants.into_iter().collect(), place });
    }

    differences.flush()?;
    drop(differences);
    let catalogue = MunicipalityCatalogue::from_rows(rows)?;
    catalogue.write(BufWriter::new(File::create(scratch.join(ARTIFACTS[0]))?))?;
    fs::write(scratch.join(ARTIFACTS[1]), OVERRIDES)?;
    let mut artifacts = BTreeMap::new();
    for name in ARTIFACTS {
        let path = scratch.join(name);
        artifacts.insert(name, json!({"sha256": sha256_file(&path)?, "bytes": fs::metadata(&path)?.len()}));
    }
    let manifest = json!({
        "version": 1,
        "snapshot": {"competence": snapshot.manifest.competence, "sha256": snapshot_sha},
        "census_year": census_year,
        "estimate_year": estimate_year,
        "neighbour_excluded_operational_areas": OPERATIONAL_AREAS,
        "source_manifest": source_manifest,
        "artifacts": artifacts,
    });
    let mut file = BufWriter::new(File::create(scratch.join("manifest.json"))?);
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    writeln!(file)?;
    file.flush()?;
    drop(file);

    fs::rename(scratch, out).with_context(|| format!("publish catalogue to {} (the directory must be absent or empty)", out.display()))?;
    println!("catalogue: {} rows, {} used TOM codes, {difference_count} RFB/IBGE name differences", catalogue.rows().len(), used.len());
    Ok(())
}

fn siafi(path: &Path) -> Result<BTreeMap<u16, Identity>> {
    let bytes = fs::read(path)?;
    let mut fields = Vec::new();
    let mut rows = BTreeMap::new();
    for (line, record) in bytes.split(|&b| b == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        split_fields(record.strip_suffix(b"\r").unwrap_or(record), &mut fields);
        let [tom, _, name, uf, ibge] = fields[..] else { bail!("unexpected Tesouro layout on line {}", line + 1) };
        let tom = tom_code(std::str::from_utf8(tom)?)?;
        let ibge = std::str::from_utf8(ibge)?.parse()?;
        let uf = std::str::from_utf8(uf)?.to_owned();
        let mut source_name = String::new();
        latin1_to_string(name, &mut source_name);
        let identity = Identity { ibge: Some(ibge), uf, source_name: source_name.trim().into() };
        ensure!(rows.insert(tom, identity).is_none(), "duplicate Tesouro TOM {tom:04}");
    }
    Ok(rows)
}

fn overrides() -> Result<BTreeMap<u16, Identity>> {
    let mut rows = BTreeMap::new();
    for line in OVERRIDES.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        let [tom, ibge, uf, name, source] = fields[..] else { bail!("invalid municipality override {line:?}") };
        ensure!(!source.is_empty(), "missing source for override {tom}");
        let tom = tom_code(tom)?;
        let identity = Identity { ibge: (!ibge.is_empty()).then(|| ibge.parse()).transpose()?, uf: uf.into(), source_name: name.into() };
        ensure!(rows.insert(tom, identity).is_none(), "duplicate override TOM {tom:04}");
    }
    Ok(rows)
}

fn aliases() -> Result<BTreeMap<u32, Vec<&'static str>>> {
    let mut aliases = BTreeMap::<u32, Vec<_>>::new();
    for line in ALIASES.lines().skip(1) {
        let (id, alias) = line.split_once('\t').context("invalid municipality alias")?;
        aliases.entry(id.parse()?).or_default().push(alias);
    }
    Ok(aliases)
}

fn tom_code(code: &str) -> Result<u16> {
    ensure!(code.len() == 4 && code.bytes().all(|b| b.is_ascii_digit()), "invalid TOM code {code:?}");
    Ok(code.parse()?)
}
