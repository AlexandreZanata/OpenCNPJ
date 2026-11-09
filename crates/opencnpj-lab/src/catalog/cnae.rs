//! WP-1.3 CNAE catalogue: hierarchy, legacy RFB codes, sector aliases and reports, built
//! from local, versioned source files. Port of the former `scripts/build-cnae.py`; the
//! outputs are byte-identical to it (checked when the port was made, see
//! `docs/intelligence/results/2026-11-07-reviewer-tasks.md`).

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use quick_xml::{Reader, events::Event};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::{UnicodeNormalization, char::canonical_combining_class};
use zip::ZipArchive;

const SOURCES: [(&str, &str); 3] = [
    ("ibge-subclasses.json", "https://servicodados.ibge.gov.br/api/v2/cnae/subclasses"),
    ("concla-2.3.zip", "https://ftp.ibge.gov.br/Informacoes_Gerais_e_Referencia/Classificacoes/CNAE/cnae2_3/cnae_subclasses_2_3_ods.zip"),
    (
        "concla-2.2.zip",
        "https://ftp.ibge.gov.br/Informacoes_Gerais_e_Referencia/Classificacoes/CNAE/cnae2_2/cnae_2_2_subclasses_ods_20150609.zip",
    ),
];
const EXCEPTION_SOURCES: [(&str, &str); 4] = [
    ("concla-2010", "https://www.gov.br/mme/pt-br/arquivos/do-29-06-2010-s1.pdf"),
    ("concla-2007", "https://cnae.ibge.gov.br/images/concla/resolucoes_e_atas/ataconcla20_22032007.pdf"),
    ("concla-2.2-notes", "https://cnae.ibge.gov.br/images/concla/downloads/cnae-subclasses-2-2-notas-explicativas.pdf"),
    ("rfb-reference", "rfb_cnaes.tsv"),
];
/// Activity prefixes stripped to make head phrases, in regex alternation order.
const HEAD_PREFIXES: [&str; 7] = [
    "comercio varejista especializado de ",
    "comercio varejista de ",
    "comercio atacadista especializado de ",
    "comercio atacadista de ",
    "fabricacao de ",
    "cultivo de ",
    "criacao de ",
];

fn catalog_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../opencnpj-insights/catalog")
}

fn is_exact_code(text: &str) -> bool {
    text.len() == 7 && text.bytes().all(|b| b.is_ascii_digit())
}

/// `dddd-d/dd`, optionally followed by ASCII spaces and `*` → seven digits (the old
/// script's `(\d{4})-(\d)/(\d{2})(?:\s*\*)?` full match).
fn code_from_cell(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.len() < 9 {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && bytes[4] == b'-' && digits(5..6) && bytes[6] == b'/' && digits(7..9)) {
        return None;
    }
    let rest = &text[9..];
    if !rest.is_empty() && rest.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']) != "*" {
        return None;
    }
    Some(format!("{}{}{}", &text[0..4], &text[5..6], &text[7..9]))
}

/// TSV reader that understands the quoting of Python's `csv` module.
fn tsv_rows(path: &Path) -> Result<Vec<BTreeMap<String, String>>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut records = parse_tsv(&text).into_iter();
    let header = records.next().context("empty TSV")?;
    Ok(records.map(|fields| header.iter().cloned().zip(fields.into_iter().chain(std::iter::repeat(String::new()))).collect()).collect())
}

fn parse_tsv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut started = false;
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => quoted = true,
            '\t' => row.push(std::mem::take(&mut field)),
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' => {}
            _ => field.push(ch),
        }
        started = true;
    }
    if started && (!field.is_empty() || !row.is_empty()) {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// TSV writer matching Python's `csv.writer(delimiter="\t", lineterminator="\n")`.
fn write_tsv<R: AsRef<[String]>>(path: &Path, header: &[&str], rows: impl IntoIterator<Item = R>) -> Result<()> {
    let field = |value: &str| -> String {
        if value.contains(['\t', '"', '\n', '\r']) { format!("\"{}\"", value.replace('"', "\"\"")) } else { value.to_string() }
    };
    let mut out = header.iter().map(|value| field(value)).collect::<Vec<_>>().join("\t");
    out.push('\n');
    for row in rows {
        out.push_str(&row.as_ref().iter().map(|value| field(value)).collect::<Vec<_>>().join("\t"));
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("write {}", path.display()))
}

/// Rows of the single sheet of the workbook `member` inside the ZIP at `path`, `width`
/// cells each: direct `table-row` children of the sheet, their direct cell children, cell
/// text = all text inside the cell, stripped; repeated cells expanded; short rows padded.
fn ods_rows(path: &Path, member: &str, width: usize) -> Result<Vec<Vec<String>>> {
    let mut archive = ZipArchive::new(fs::File::open(path).with_context(|| format!("open {}", path.display()))?)?;
    let mut names = Vec::new();
    for name in archive.file_names() {
        let name = name?;
        if name.contains(member) {
            names.push(name.into_owned());
        }
    }
    ensure!(names.len() == 1, "{}: expected exactly one {member:?} workbook", path.display());
    let mut workbook = Vec::new();
    archive.by_name(&names[0])?.read_to_end(&mut workbook)?;
    let mut inner = ZipArchive::new(Cursor::new(workbook))?;
    let mut content = String::new();
    inner.by_name("content.xml")?.read_to_string(&mut content)?;
    let mut xml = Reader::from_str(&content);
    let is_cell = |name: &[u8]| matches!(name, b"table:table-cell" | b"table:covered-table-cell");
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut sheets = 0;
    let mut rows = Vec::new();
    let mut row: Option<Vec<String>> = None;
    // Open cell: (text so far, repetitions, stack depth of the cell element).
    let mut cell: Option<(String, usize, usize)> = None;
    loop {
        match xml.read_event()? {
            Event::Start(tag) => {
                let name = tag.name().as_ref().to_vec();
                let parent = stack.last().map(Vec::as_slice);
                if name == b"table:table" {
                    sheets += 1;
                } else if name == b"table:table-row" && parent == Some(b"table:table".as_slice()) {
                    row = Some(Vec::new());
                } else if is_cell(&name) && row.is_some() && cell.is_none() && parent == Some(b"table:table-row".as_slice()) {
                    cell = Some((String::new(), repeated(&tag, xml.decoder())?, stack.len() + 1));
                }
                stack.push(name);
            }
            Event::Empty(tag) => {
                let qualified = tag.name();
                let name = qualified.as_ref();
                let parent = stack.last().map(Vec::as_slice);
                if name == b"table:table-row" && parent == Some(b"table:table".as_slice()) {
                    rows.push(vec![String::new(); width]);
                } else if is_cell(name)
                    && cell.is_none()
                    && parent == Some(b"table:table-row".as_slice())
                    && let Some(cells) = row.as_mut()
                {
                    add_cell(cells, String::new(), repeated(&tag, xml.decoder())?, width);
                }
            }
            Event::Text(text) => {
                if let Some((buffer, _, _)) = cell.as_mut() {
                    buffer.push_str(&text.decode()?);
                }
            }
            Event::GeneralRef(reference) => {
                if let Some((buffer, _, _)) = cell.as_mut() {
                    match reference.resolve_char_ref()? {
                        Some(ch) => buffer.push(ch),
                        None => {
                            let name = reference.decode()?;
                            buffer
                                .push_str(quick_xml::escape::resolve_xml_entity(&name).with_context(|| format!("unknown entity {name}"))?);
                        }
                    }
                }
            }
            Event::End(_) => {
                if let Some((_, _, depth)) = &cell
                    && *depth == stack.len()
                {
                    let (text, count, _) = cell.take().expect("open cell");
                    if let Some(cells) = row.as_mut() {
                        add_cell(cells, text, count, width);
                    }
                }
                let name = stack.pop().context("unbalanced ODS XML")?;
                if name == b"table:table-row"
                    && stack.last().map(Vec::as_slice) == Some(b"table:table".as_slice())
                    && let Some(mut cells) = row.take()
                {
                    cells.resize(width, String::new());
                    rows.push(cells);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(sheets == 1, "{}: expected one worksheet in {member}", path.display());
    Ok(rows)
}

fn repeated(tag: &quick_xml::events::BytesStart<'_>, decoder: quick_xml::encoding::Decoder) -> Result<usize> {
    Ok(match tag.try_get_attribute("table:number-columns-repeated")? {
        Some(value) => value.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, decoder)?.parse()?,
        None => 1,
    })
}

fn add_cell(cells: &mut Vec<String>, text: String, count: usize, width: usize) {
    if cells.len() < width {
        let text = text.trim().to_string();
        let count = count.min(width - cells.len());
        cells.extend(std::iter::repeat_n(text, count));
    }
}

/// Source subclass → destination subclass → partial flag.
fn correspondences(path: &Path, member: &str) -> Result<BTreeMap<String, BTreeMap<String, bool>>> {
    let mut mappings: BTreeMap<String, BTreeMap<String, bool>> = BTreeMap::new();
    let mut previous: Option<String> = None;
    for cells in ods_rows(path, member, 3)? {
        let (left, right, flag) = (&cells[0], &cells[1], &cells[2]);
        let origin = code_from_cell(left);
        let target = code_from_cell(right);
        if origin.is_some() || !left.is_empty() {
            previous = origin;
        }
        if let Some(target) = target {
            let source = previous.clone().with_context(|| format!("{}: destination {target} has no source code", path.display()))?;
            let partial = right.contains('*') || flag == "*";
            let entry = mappings.entry(source).or_default();
            let old = entry.get(&target).copied().unwrap_or(false);
            entry.insert(target, partial || old);
        }
    }
    ensure!(!mappings.is_empty(), "{}: empty correspondence sheet", path.display());
    Ok(mappings)
}

#[derive(Deserialize)]
struct Node {
    id: String,
    descricao: String,
}

#[derive(Deserialize)]
struct Division {
    id: String,
    descricao: String,
    secao: Node,
}

#[derive(Deserialize)]
struct Group {
    id: String,
    descricao: String,
    divisao: Division,
}

#[derive(Deserialize)]
struct Class {
    id: String,
    descricao: String,
    grupo: Group,
}

#[derive(Deserialize)]
struct Subclass {
    id: String,
    descricao: String,
    atividades: Vec<String>,
    classe: Class,
}

type Hierarchy = (BTreeMap<String, (String, String, String)>, BTreeMap<String, Subclass>, usize);

fn read_hierarchy(path: &Path) -> Result<Hierarchy> {
    let subclasses: Vec<Subclass> = serde_json::from_str(&fs::read_to_string(path)?)?;
    let mut nodes = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    let mut phrases = 0;
    for subclass in subclasses {
        let code = subclass.id.clone();
        ensure!(is_exact_code(&code) && !by_code.contains_key(&code), "invalid or repeated subclass {code}");
        phrases += subclass.atividades.len();
        let c = &subclass.classe;
        let (g, d) = (&c.grupo, &c.grupo.divisao);
        let s = &d.secao;
        for (key, level, parent, description) in [
            (&subclass.id, "subclass", &c.id, &subclass.descricao),
            (&c.id, "class", &g.id, &c.descricao),
            (&g.id, "group", &d.id, &g.descricao),
            (&d.id, "division", &s.id, &d.descricao),
            (&s.id, "section", &String::new(), &s.descricao),
        ] {
            let node = (level.to_string(), parent.clone(), description.split_whitespace().collect::<Vec<_>>().join(" "));
            let existing = nodes.entry(key.clone()).or_insert_with(|| node.clone());
            ensure!(*existing == node, "inconsistent hierarchy for {key}");
        }
        by_code.insert(code, subclass);
    }
    ensure!(!by_code.is_empty(), "IBGE API has no subclasses");
    Ok((nodes, by_code, phrases))
}

/// RFB code → (relation, targets, source, note).
type Legacy = BTreeMap<String, (String, Vec<String>, String, String)>;

fn legacy_map(raw: &Path, reference: &BTreeMap<String, String>, official: &BTreeSet<String>) -> Result<Legacy> {
    let two_to_three = correspondences(&raw.join("concla-2.3.zip"), "2_2 x")?;
    let one_to_two = correspondences(&raw.join("concla-2.2.zip"), "correspondencia2.ods")?;
    let sources: BTreeSet<&str> = EXCEPTION_SOURCES.iter().map(|(name, _)| *name).collect();
    let mut overrides: Legacy = BTreeMap::new();
    for row in tsv_rows(&catalog_dir().join("cnae_legacy_exceptions.tsv"))? {
        let code = row["code"].clone();
        let targets: Vec<String> = row["targets"].split(',').filter(|value| !value.is_empty()).map(str::to_string).collect();
        ensure!(
            !overrides.contains_key(&code) && reference.contains_key(&code) && !official.contains(&code),
            "unexpected exception {code}"
        );
        ensure!(
            targets.iter().all(|target| official.contains(target)) && (!targets.is_empty() || row["relation"] == "unreported"),
            "invalid exception targets for {code}"
        );
        ensure!(
            row["source"].split(';').all(|source| sources.contains(source)) && !row["note"].is_empty(),
            "missing exception provenance for {code}"
        );
        overrides.insert(code, (row["relation"].clone(), targets, row["source"].clone(), row["note"].clone()));
    }
    let mut mapped = BTreeMap::new();
    for code in reference.keys().filter(|code| !official.contains(*code)) {
        if let Some(row) = overrides.get(code) {
            mapped.insert(code.clone(), row.clone());
            continue;
        }
        let direct = two_to_three.get(code).cloned().unwrap_or_default();
        let (targets, source) = if !direct.is_empty() && direct.keys().all(|target| official.contains(target)) {
            (direct, "concla-2.3")
        } else {
            let mut targets: BTreeMap<String, bool> = BTreeMap::new();
            for (interim, first_partial) in one_to_two.get(code).cloned().unwrap_or_default() {
                let links = two_to_three.get(&interim).cloned().unwrap_or_else(|| BTreeMap::from([(interim.clone(), false)]));
                ensure!(
                    links.keys().all(|target| official.contains(target)),
                    "obsolete intermediate target for RFB CNAE {code}: {interim}"
                );
                for (target, second_partial) in links {
                    let old = targets.get(&target).copied().unwrap_or(false);
                    targets.insert(target, first_partial || second_partial || old);
                }
            }
            (targets, "concla-2.2;concla-2.3")
        };
        ensure!(!targets.is_empty(), "RFB CNAE {code} has no documented mapping");
        let relation = if targets.values().any(|partial| *partial) {
            "partial"
        } else if targets.len() > 1 {
            "split"
        } else {
            "successor"
        };
        mapped.insert(
            code.clone(),
            (
                relation.to_string(),
                targets.into_keys().collect(),
                source.to_string(),
                "CONCLA correspondence. Retain the original RFB code for execution.".to_string(),
            ),
        );
    }
    Ok(mapped)
}

/// Python's `fold`: NFKD of the casefolded text without combining marks, keeping only
/// `[a-z0-9]+` runs joined by single spaces.
pub(crate) fn fold(text: &str) -> String {
    let lowered: String = text.chars().flat_map(casefold).collect();
    let plain: String = lowered.nfkd().filter(|ch| canonical_combining_class(*ch) == 0).collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for ch in plain.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            word.push(ch);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words.join(" ")
}

/// `str.casefold` for the characters that differ from lowercasing in Latin text.
fn casefold(ch: char) -> Vec<char> {
    match ch {
        'ß' | 'ẞ' => vec!['s', 's'],
        'ſ' => vec!['s'],
        _ => ch.to_lowercase().collect(),
    }
}

#[derive(Deserialize)]
struct SectorFile {
    sector: Vec<SectorEntry>,
}

#[derive(Deserialize)]
struct SectorEntry {
    id: String,
    display: SectorDisplay,
    aliases: Vec<String>,
    cnae_core: Vec<String>,
    cnae_extended: Vec<String>,
    scope: String,
    since: String,
}

#[derive(Deserialize)]
struct SectorDisplay {
    sing: String,
    plural: String,
    gender: String,
}

type Curated = (Vec<SectorEntry>, BTreeMap<String, BTreeSet<String>>, BTreeMap<String, BTreeSet<String>>);

fn sectors_and_aliases(path: &Path, valid: &BTreeSet<String>) -> Result<Curated> {
    let sectors = toml::from_str::<SectorFile>(&fs::read_to_string(path)?)?.sector;
    let mut ids = BTreeSet::new();
    let mut aliases: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut scopes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for sector in &sectors {
        let name = &sector.id;
        let valid_id = name.starts_with(|ch: char| ch.is_ascii_lowercase())
            && name.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
        ensure!(valid_id && ids.insert(name.clone()), "invalid or duplicate sector {name}");
        let sorted_unique = |codes: &[String]| codes.windows(2).all(|pair| pair[0] < pair[1]);
        ensure!(
            !sector.cnae_core.is_empty() && sorted_unique(&sector.cnae_core) && sorted_unique(&sector.cnae_extended),
            "invalid CNAE sets in {name}"
        );
        let core: BTreeSet<String> = sector.cnae_core.iter().cloned().collect();
        let extended: BTreeSet<String> = sector.cnae_extended.iter().cloned().collect();
        ensure!(core.is_disjoint(&extended), "core and extension overlap in {name}");
        let scope: BTreeSet<String> = core.union(&extended).cloned().collect();
        let invalid: Vec<&String> = scope.difference(valid).collect();
        ensure!(invalid.is_empty(), "invalid sector codes in {name}: {invalid:?}");
        scopes.insert(name.clone(), scope);
        ensure!(
            matches!(sector.display.gender.as_str(), "m" | "f") && sector.scope == "main" && sector.since == "v1",
            "invalid display or scope for {name}"
        );
        ensure!(!sector.aliases.is_empty(), "no aliases for {name}");
        for label in [&sector.display.sing, &sector.display.plural].into_iter().chain(&sector.aliases) {
            let alias = fold(label);
            ensure!(!alias.is_empty(), "empty normalized alias in {name}");
            aliases.entry(alias).or_default().insert(name.clone());
        }
    }
    ensure!(sectors.len() >= 200, "sector catalogue requires at least 200 curated entries");
    Ok((sectors, aliases, scopes))
}

fn automatic_aliases(subclasses: &BTreeMap<String, Subclass>) -> Vec<[String; 3]> {
    let mut candidates = BTreeSet::new();
    for (code, subclass) in subclasses {
        let texts = std::iter::once((subclass.descricao.as_str(), "description"))
            .chain(subclass.atividades.iter().map(|text| (text.as_str(), "activity")));
        for (text, source) in texts {
            let words = fold(text);
            if !words.is_empty() {
                candidates.insert([words.clone(), code.clone(), source.to_string()]);
            }
            if let Some((subject, action)) = text.rsplit_once(';')
                && !fold(action).is_empty()
                && !fold(subject).is_empty()
            {
                candidates.insert([fold(&format!("{action} {subject}")), code.clone(), "reordered_activity".to_string()]);
            }
            if let Some(head) = HEAD_PREFIXES.iter().find_map(|prefix| words.strip_prefix(prefix))
                && head.split_whitespace().count() >= 2
            {
                candidates.insert([head.to_string(), code.clone(), "head_phrase".to_string()]);
            }
        }
    }
    candidates.into_iter().collect()
}

fn digest(path: &Path) -> Result<Value> {
    let data = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(json!({"bytes": data.len(), "sha256": hex::encode(Sha256::digest(&data))}))
}

pub(super) fn build(raw: &Path, rfb: &Path, sector_path: &Path, out: &Path) -> Result<()> {
    let (nodes, subclasses, phrases) = read_hierarchy(&raw.join("ibge-subclasses.json"))?;
    let official: BTreeSet<String> = subclasses.keys().cloned().collect();
    let structure: BTreeSet<String> = ods_rows(&raw.join("concla-2.3.zip"), "Estrutura Detalhada", 6)?
        .iter()
        .flat_map(|row| row.iter().filter_map(|cell| code_from_cell(cell)))
        .collect();
    ensure!(
        official == structure,
        "IBGE and CONCLA subclass sets differ: {:?}",
        official.symmetric_difference(&structure).collect::<Vec<_>>()
    );
    let mut reference = BTreeMap::new();
    for row in tsv_rows(&rfb.join("rfb_cnaes.tsv"))? {
        let code = row["code"].clone();
        ensure!(is_exact_code(&code) && !reference.contains_key(&code), "invalid or duplicate RFB reference code {code}");
        reference.insert(code, row["label"].clone());
    }
    let mappings = legacy_map(raw, &reference, &official)?;
    let mut valid = official.clone();
    valid.extend(mappings.iter().filter(|(_, row)| row.0 != "unreported").map(|(code, _)| code.clone()));
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for row in tsv_rows(&rfb.join("used_codes.tsv"))? {
        let (code, field) = (&row["code"], row["field"].as_str());
        if matches!(field, "cnae" | "cnae_secundaria") {
            ensure!(official.contains(code) || mappings.contains_key(code), "unclassified used CNAE {code}");
        }
        if field == "cnae" {
            ensure!(!counts.contains_key(code), "duplicate main CNAE {code}");
            counts.insert(code.clone(), row["document_count"].parse()?);
        }
    }
    let (sectors, curated, scopes) = sectors_and_aliases(sector_path, &valid)?;
    let auto = automatic_aliases(&subclasses);
    let mut by_phrase: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for [alias, code, _] in &auto {
        by_phrase.entry(alias.clone()).or_default().insert(code.clone());
    }
    let mut ambiguous = Vec::new();
    let empty = BTreeSet::new();
    for alias in curated.keys().chain(by_phrase.keys()).collect::<BTreeSet<_>>() {
        let sector_ids = curated.get(alias).unwrap_or(&empty);
        let codes = by_phrase.get(alias).unwrap_or(&empty);
        let curated_codes: BTreeSet<&String> = sector_ids.iter().flat_map(|sector| &scopes[sector]).collect();
        if sector_ids.len() > 1 || codes.len() > 1 || (!sector_ids.is_empty() && codes.iter().any(|code| !curated_codes.contains(code))) {
            ambiguous.push([
                alias.clone(),
                sector_ids.iter().cloned().collect::<Vec<_>>().join(","),
                codes.iter().cloned().collect::<Vec<_>>().join(","),
            ]);
        }
    }
    let automatic_codes: BTreeSet<&String> = by_phrase.values().flatten().collect();
    let unique_automatic: BTreeSet<&String> = by_phrase
        .iter()
        .filter(|(alias, codes)| codes.len() == 1 && !curated.contains_key(*alias))
        .filter_map(|(_, codes)| codes.iter().next())
        .collect();
    let curated_codes: BTreeSet<&String> = scopes.values().flatten().collect();
    let unique_curated: BTreeSet<&String> =
        curated.values().filter(|ids| ids.len() == 1).flat_map(|ids| &scopes[ids.iter().next().expect("one sector")]).collect();

    let mut output_rows: Vec<[String; 8]> = nodes
        .iter()
        .map(|(code, (level, parent, name))| {
            [code.clone(), level.clone(), parent.clone(), name.clone(), String::new(), String::new(), String::new(), String::new()]
        })
        .collect();
    for (code, (relation, targets, source, note)) in &mappings {
        let parent = if nodes.contains_key(&code[..5]) { code[..5].to_string() } else { String::new() };
        output_rows.push([
            code.clone(),
            "legacy".into(),
            parent,
            reference[code].clone(),
            relation.clone(),
            targets.join(","),
            source.clone(),
            note.clone(),
        ]);
    }
    output_rows.sort();
    let core_documents = |sector: &SectorEntry| -> u64 { sector.cnae_core.iter().map(|code| counts.get(code).copied().unwrap_or(0)).sum() };
    let mut top: Vec<&SectorEntry> = sectors.iter().collect();
    top.sort_by(|a, b| core_documents(b).cmp(&core_documents(a)).then_with(|| a.id.cmp(&b.id)));
    top.truncate(50);

    ensure!(!out.exists(), "destination already exists: {}", out.display());
    let parent = out.parent().context("output has no parent directory")?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new().prefix(".cnae-").tempdir_in(parent)?;
    let target = staging.path();
    write_tsv(
        &target.join("cnae.tsv"),
        &["code", "level", "parent", "name", "legacy_relation", "legacy_targets", "legacy_source", "legacy_note"],
        &output_rows,
    )?;
    write_tsv(&target.join("sector_aliases_auto.tsv"), &["alias", "code", "source"], &auto)?;
    write_tsv(&target.join("ambiguity.tsv"), &["alias", "sectors", "auto_codes"], &ambiguous)?;
    let reachability: Vec<[String; 4]> = official
        .iter()
        .chain(mappings.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|code| {
            let flag = |set: bool| u8::from(set).to_string();
            [
                code.clone(),
                flag(automatic_codes.contains(code)),
                flag(curated_codes.contains(code)),
                flag(unique_automatic.contains(code) || unique_curated.contains(code)),
            ]
        })
        .collect();
    write_tsv(&target.join("reachability.tsv"), &["code", "automatic", "curated", "unambiguous"], &reachability)?;
    let top_rows: Vec<[String; 2]> = top.iter().map(|sector| [sector.id.clone(), core_documents(sector).to_string()]).collect();
    write_tsv(&target.join("top-50.tsv"), &["sector", "core_main_documents"], &top_rows)?;
    fs::copy(sector_path, target.join("sectors.toml"))?;
    let mut artifacts = BTreeMap::new();
    let mut names: Vec<PathBuf> = fs::read_dir(target)?.map(|entry| entry.map(|entry| entry.path())).collect::<Result<_, _>>()?;
    names.sort();
    for path in names {
        artifacts.insert(path.file_name().context("artifact name")?.to_string_lossy().into_owned(), digest(&path)?);
    }
    let mut sources = BTreeMap::new();
    for (name, url) in SOURCES {
        let mut entry = digest(&raw.join(name))?;
        entry["url"] = json!(url);
        sources.insert(name, entry);
    }
    let manifest = json!({
        "version": "v1",
        "cnae_version": "2.3",
        "sources": sources,
        "legacy_exception_sources": EXCEPTION_SOURCES.iter().copied().collect::<BTreeMap<_, _>>(),
        "legacy_exceptions": digest(&catalog_dir().join("cnae_legacy_exceptions.tsv"))?,
        "rfb_inputs": {"rfb_cnaes.tsv": digest(&rfb.join("rfb_cnaes.tsv"))?, "used_codes.tsv": digest(&rfb.join("used_codes.tsv"))?},
        "artifacts": artifacts,
    });
    fs::write(target.join("manifest.json"), serde_json::to_string_pretty(&manifest)? + "\n")?;
    let staged = staging.keep();
    fs::rename(&staged, out).with_context(|| format!("rename into {}", out.display()))?;
    println!("official subclasses: {}, activity phrases: {phrases}, hierarchy nodes: {}", official.len(), nodes.len());
    println!(
        "RFB codes: {}, covered by official hierarchy: {}, legacy dispositions: {}",
        reference.len(),
        reference.keys().filter(|code| official.contains(*code)).count(),
        mappings.len()
    );
    println!("curated sectors: {}, automatic alias rows: {}, ambiguous phrases: {}", sectors.len(), auto.len(), ambiguous.len());
    let without = official.iter().chain(mappings.keys()).collect::<BTreeSet<_>>();
    let without = without.iter().filter(|code| !unique_automatic.contains(**code) && !unique_curated.contains(**code)).count();
    println!("codes without an unambiguous alias: {without}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{code_from_cell, fold, parse_tsv};

    #[test]
    fn fold_matches_python() {
        assert_eq!(fold("Comércio Varejista de Pães; FABRICAÇÃO"), "comercio varejista de paes fabricacao");
        assert_eq!(fold("  ºª ²  "), "oa 2");
        assert_eq!(fold("Straße"), "strasse");
    }

    #[test]
    fn cnae_cells() {
        assert_eq!(code_from_cell("4721-1/02").as_deref(), Some("4721102"));
        assert_eq!(code_from_cell("4721-1/02 *").as_deref(), Some("4721102"));
        assert_eq!(code_from_cell("4721-1/02*").as_deref(), Some("4721102"));
        assert_eq!(code_from_cell("4721-1/023"), None);
        assert_eq!(code_from_cell("47.21-1"), None);
    }

    #[test]
    fn tsv_quotes() {
        assert_eq!(parse_tsv("a\tb\n\"x\"\"y\"\t\n"), vec![vec!["a".to_string(), "b".into()], vec!["x\"y".into(), String::new()]]);
    }
}
