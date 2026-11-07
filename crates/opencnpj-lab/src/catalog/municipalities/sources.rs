use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use opencnpj_ingest::source::sha256_file;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde::Deserialize;
use zip::ZipArchive;

#[derive(Deserialize)]
pub(super) struct Locality {
    pub id: u32,
    #[serde(rename = "nome")]
    pub name: String,
    #[serde(rename = "regiao-imediata")]
    pub immediate: ImmediateRegion,
}

#[derive(Deserialize)]
pub(super) struct ImmediateRegion {
    pub id: u32,
    #[serde(rename = "regiao-intermediaria")]
    pub intermediate: IntermediateRegion,
}

#[derive(Deserialize)]
pub(super) struct IntermediateRegion {
    pub id: u16,
    #[serde(rename = "UF")]
    pub uf: Uf,
}

#[derive(Deserialize)]
pub(super) struct Uf {
    #[serde(rename = "sigla")]
    pub abbreviation: String,
}

pub(super) fn localities(path: &Path) -> Result<BTreeMap<u32, Locality>> {
    let list: Vec<Locality> = read_json(path)?;
    let mut values = BTreeMap::new();
    for row in list {
        let id = row.id;
        ensure!(values.insert(id, row).is_none(), "duplicate IBGE locality {id}");
    }
    Ok(values)
}

#[derive(Deserialize)]
struct Aggregate {
    id: String,
    #[serde(rename = "resultados")]
    results: Vec<Series>,
}

#[derive(Deserialize)]
struct Series {
    series: Vec<PopulationRow>,
}

#[derive(Deserialize)]
struct PopulationRow {
    #[serde(rename = "localidade")]
    locality: PopulationLocality,
    #[serde(rename = "serie")]
    values: BTreeMap<u16, String>,
}

#[derive(Deserialize)]
struct PopulationLocality {
    id: String,
}

pub(super) fn population(path: &Path, variable: &str) -> Result<(u16, BTreeMap<u32, u32>)> {
    let aggregates: Vec<Aggregate> = read_json(path)?;
    let [aggregate] = aggregates.as_slice() else { bail!("expected one aggregate in {}", path.display()) };
    let [series] = aggregate.results.as_slice() else { bail!("expected one series in {}", path.display()) };
    ensure!(aggregate.id == variable, "unexpected variable {} in {}", aggregate.id, path.display());
    let mut year = None;
    let mut counts = BTreeMap::new();
    for row in &series.series {
        let id = row.locality.id.parse::<u32>()?;
        let Some((&period, value)) = row.values.first_key_value() else { bail!("missing population period for {id}") };
        ensure!(row.values.len() == 1 && year.is_none_or(|y| y == period), "mixed population periods in {}", path.display());
        year = Some(period);
        let count = value.parse::<u32>().with_context(|| format!("population for {id} in {}", path.display()))?;
        ensure!(count > 0 && counts.insert(id, count).is_none(), "invalid or duplicate population for {id}");
    }
    Ok((year.context("empty population series")?, counts))
}

#[derive(Deserialize)]
struct SourceManifest {
    retrieved_on: String,
    sources: Vec<Source>,
}

#[derive(Deserialize)]
struct Source {
    file: String,
    url: String,
    bytes: u64,
    sha256: String,
    terms_url: String,
    terms_note: String,
}

pub(super) fn manifest(raw: &Path) -> Result<(serde_json::Value, PathBuf)> {
    let value: serde_json::Value = read_json(&raw.join("manifest.json"))?;
    let parsed: SourceManifest = serde_json::from_value(value.clone())?;
    ensure!(!parsed.retrieved_on.is_empty(), "missing source retrieval date");
    let mut files = BTreeSet::new();
    for source in &parsed.sources {
        let name = source.file.as_str();
        ensure!(Path::new(name).file_name().is_some_and(|file| file == name), "invalid source filename {name:?}");
        ensure!(files.insert(name), "duplicate source file {name}");
        ensure!(
            source.url.starts_with("https://") && source.terms_url.starts_with("https://") && !source.terms_note.is_empty(),
            "missing provenance for {name}"
        );
        let path = raw.join(name);
        ensure!(
            fs::metadata(&path)?.len() == source.bytes && sha256_file(&path)? == source.sha256,
            "source checksum or size mismatch for {name}"
        );
    }
    for required in
        ["tabmun.csv", "ibge-municipios.json", "ibge-census-2022.json", "ibge-neighbours-2024.ods", "ibge-localities-2022-gpkg.zip"]
    {
        ensure!(files.contains(required), "manifest does not cover {required}");
    }
    let estimates: Vec<_> = files.into_iter().filter(|name| name.starts_with("ibge-estimate-") && name.ends_with(".json")).collect();
    let [estimate] = estimates.as_slice() else { bail!("manifest must cover exactly one IBGE estimate file") };
    Ok((value, raw.join(estimate)))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    serde_json::from_reader(BufReader::new(file)).with_context(|| format!("parse {}", path.display()))
}

#[derive(Clone, Copy)]
pub(super) struct Seat {
    pub latitude: f64,
    pub longitude: f64,
    pub capital: bool,
}

pub(super) fn seats(path: &Path, scratch: &Path) -> Result<BTreeMap<u32, Seat>> {
    let mut zip = ZipArchive::new(File::open(path)?)?;
    let mut gpkg = tempfile::NamedTempFile::new_in(scratch)?;
    io::copy(&mut zip.by_name("BR_localidades_2022.gpkg")?, gpkg.as_file_mut())?;
    let db = rusqlite::Connection::open_with_flags(gpkg.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = db.prepare(
        "SELECT CD_MUN, LAT_LOCALIDADE, LONG_LOCALIDADE, SCT_LOCALIDADE IN ('Capital Estadual', 'Capital Federal')
         FROM BR_localidades_2022
         WHERE SCT_LOCALIDADE IN ('Sede Municipal', 'Capital Estadual', 'Capital Federal')
            OR CT_LOCALIDADE = 'Distrito Estadual de Fernando de Noronha'",
    )?;
    let mut out = BTreeMap::<u32, Seat>::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id = row.get::<_, String>(0)?.parse()?;
        let seat = Seat { latitude: row.get(1)?, longitude: row.get(2)?, capital: row.get::<_, Option<bool>>(3)?.unwrap_or(false) };
        if let Some(previous) = out.get_mut(&id) {
            ensure!(
                previous.latitude == seat.latitude && previous.longitude == seat.longitude,
                "conflicting IBGE seat coordinates for {id}"
            );
            previous.capital |= seat.capital;
        } else {
            out.insert(id, seat);
        }
    }
    Ok(out)
}

pub(super) fn neighbours(path: &Path) -> Result<BTreeMap<u32, BTreeSet<u32>>> {
    let mut zip = ZipArchive::new(File::open(path)?)?;
    let mut xml = Reader::from_reader(BufReader::new(zip.by_name("content.xml")?));
    let mut buf = Vec::new();
    let mut in_sheet = false;
    let mut header = [String::new(), String::new(), String::new(), String::new()];
    let mut first_row = true;
    let mut column = 0;
    let mut values = [None; 4];
    let mut out = BTreeMap::<u32, BTreeSet<u32>>::new();
    loop {
        match xml.read_event_into(&mut buf)? {
            Event::Start(tag) if tag.name().as_ref() == b"table:table" => {
                in_sheet = attribute(&tag, b"table:name", xml.decoder())?.as_deref() == Some("Municípios Limítrofes");
            }
            Event::Start(tag) if in_sheet && tag.name().as_ref() == b"table:table-row" => {
                column = 0;
                values = [None; 4];
            }
            Event::Start(tag) | Event::Empty(tag) if in_sheet && tag.name().as_ref() == b"table:table-cell" => {
                if column < 4 {
                    values[column] = attribute(&tag, b"office:value", xml.decoder())?.map(|value| value.parse()).transpose()?;
                }
                let width =
                    attribute(&tag, b"table:number-columns-repeated", xml.decoder())?.map_or(Ok(1), |width| width.parse::<usize>())?;
                ensure!(width > 0, "invalid ODS cell repetition");
                column = column.saturating_add(width);
            }
            Event::Text(text) if in_sheet && first_row && (1..=4).contains(&column) => {
                header[column - 1].push_str(text.decode()?.trim());
            }
            Event::End(tag) if in_sheet && tag.name().as_ref() == b"table:table-row" => {
                if first_row {
                    ensure!(header == ["CD_MUN", "NM_MUN", "N_LIM", "CD_LIM"], "unexpected IBGE neighbours header");
                    first_row = false;
                } else {
                    match values {
                        [Some(from), _, Some(0), None] => {
                            out.entry(from).or_default();
                        }
                        [Some(from), _, Some(count), Some(to)] if count > 0 => {
                            ensure!(from != to && out.entry(from).or_default().insert(to), "invalid or duplicate neighbour {from} -> {to}");
                        }
                        [None, None, None, None] => {}
                        _ => bail!("invalid IBGE neighbour row {values:?}"),
                    }
                }
            }
            Event::End(tag) if in_sheet && tag.name().as_ref() == b"table:table" => break,
            Event::Eof => bail!("IBGE neighbours sheet missing or truncated"),
            _ => {}
        }
        buf.clear();
    }
    ensure!(!first_row && !out.is_empty(), "empty IBGE neighbours sheet");
    for (&from, links) in &out {
        for to in links {
            ensure!(out.get(to).is_some_and(|back| back.contains(&from)), "missing reverse IBGE neighbour {to} -> {from}");
        }
    }
    Ok(out)
}

fn attribute(tag: &BytesStart<'_>, key: &[u8], decoder: quick_xml::encoding::Decoder) -> Result<Option<String>> {
    Ok(tag
        .try_get_attribute(key)?
        .map(|value| value.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder).map(|value| value.into_owned()))
        .transpose()?)
}
