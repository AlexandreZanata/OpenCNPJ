//! Synthetic pt-BR dataset generator for the intelligence evaluation.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use opencnpj_insights::agg;
use opencnpj_query::{
    ir::*,
    validate::{self, Capabilities, ValidRequest},
};
use serde::{Deserialize, Serialize};

pub const VERSION: &str = "synth-0.1.0";
pub const SPEC: &str = include_str!("../synth/pt-BR/families.toml");
const LAST_DAY: Day = Day(46305);
const CURRENT: Competence = Competence(321);
const ANCHOR: Day = Day(46305);

#[derive(Debug, Clone, Deserialize)]
struct SpecFile {
    family: Vec<FamilySpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FamilySpec {
    pub id: String,
    pub op: String,
    pub metric: String,
    #[serde(default)]
    pub slots: Vec<String>,
    pub status: String,
    pub place: String,
    pub sector: bool,
    #[serde(default)]
    pub dim: Option<String>,
    #[serde(default)]
    pub grain: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub templates: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MunicipalityRow {
    pub id: u16,
    pub name: String,
    pub uf: String,
    pub prep: String,
    pub population: u32,
    pub test_only: bool,
}

#[derive(Debug, Clone)]
pub struct SectorRow {
    pub id: u16,
    pub singular: String,
    pub plural: String,
    pub feminine: bool,
    pub cnaes: Vec<u32>,
    pub aliases: Vec<String>,
    pub test_only: bool,
}

#[derive(Debug, Clone)]
pub struct UfRow {
    pub id: UfId,
    pub name: String,
    pub prep: String,
    pub test_only: bool,
}

pub struct Catalogues {
    pub municipalities: Vec<MunicipalityRow>,
    pub sectors: Vec<SectorRow>,
    pub ufs: Vec<UfRow>,
    pub caps: Capabilities,
    municipality_ufs: BTreeMap<MunicipalityId, UfId>,
    sector_cnaes: BTreeMap<SectorId, SmallSet<CnaeCode>>,
    pub spec_sha256: String,
    pub catalogue_sha256: String,
}

impl Catalogues {
    pub fn canonical(&self, request: Request) -> Result<ValidRequest> {
        let context = CanonicalContext {
            first_day: Day(1),
            last_day: LAST_DAY,
            municipality_ufs: &self.municipality_ufs,
            sector_cnaes: &self.sector_cnaes,
        };
        let canonical = request.canonicalize(&context)?.0;
        validate::validate(canonical, &self.caps)
            .map_err(|issues| anyhow::anyhow!(issues.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")))
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        let uf_rows = uf_table();
        let municipalities = vec![
            muni(0, "Sorriso", "MT", "em", 100_000, false),
            muni(1, "Sinop", "MT", "em", 150_000, false),
            muni(2, "Cuiabá", "MT", "em", 650_000, false),
            muni(3, "São Paulo", "SP", "em", 12_000_000, false),
            muni(4, "Rio de Janeiro", "RJ", "no", 6_700_000, false),
            muni(5, "Salvador", "BA", "em", 2_400_000, false),
            muni(6, "Bonito", "MS", "em", 22_000, false),
            muni(7, "Bom Jesus", "RS", "em", 12_000, false),
            muni(8, "Bom Jesus", "PI", "em", 25_000, false),
            muni(9, "Ji-Paraná", "RO", "em", 130_000, true),
        ];
        let sectors = vec![
            sector(0, "padaria", "padarias", true, vec![4721101, 4721102], &["padoca"], false),
            sector(1, "farmácia", "farmácias", true, vec![4771701], &[], false),
            sector(2, "supermercado", "supermercados", false, vec![4711301], &["mercado"], false),
            sector(3, "transporte", "transportes", false, vec![4930202], &[], false),
            sector(4, "restaurante", "restaurantes", false, vec![5611201], &["lanchonete"], false),
            sector(5, "construção", "construções", true, vec![4120400], &[], true),
        ];
        Self::assemble(municipalities, sectors, uf_rows, "fixture-spec", "fixture-catalogues")
    }

    fn assemble(
        municipalities: Vec<MunicipalityRow>,
        sectors: Vec<SectorRow>,
        ufs: Vec<UfRow>,
        spec_sha256: &str,
        catalogue_sha256: &str,
    ) -> Self {
        let municipality_ufs = municipalities.iter().filter_map(|row| agg::uf_id(&row.uf).map(|uf| (MunicipalityId(row.id), uf))).collect();
        let sector_cnaes =
            sectors.iter().map(|row| (SectorId(row.id), SmallSet::new(row.cnaes.iter().map(|code| CnaeCode::Subclass(*code))))).collect();
        let mut competence = Vec::new();
        let (mut year, mut month) = (2024, 11);
        for _ in 0..24 {
            competence.push(Competence::from_year_month(year, month).unwrap());
            month += 1;
            if month == 13 {
                month = 1;
                year += 1;
            }
        }
        let caps = Capabilities {
            current: CURRENT,
            competences: competence,
            flow_days: Day::from_ymd(2021, 1, 1).unwrap()..=LAST_DAY,
            populations: vec![PopulationRef::IbgeCensus(2022), PopulationRef::IbgeEstimate(2025)],
            catalogue_versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
            sector_count: u16::try_from(sectors.len()).unwrap(),
            geography: true,
            list_limit: 100,
        };
        Self {
            municipalities,
            sectors,
            ufs,
            caps,
            municipality_ufs,
            sector_cnaes,
            spec_sha256: spec_sha256.into(),
            catalogue_sha256: catalogue_sha256.into(),
        }
    }

    pub fn load(catalog: &Path, sectors_path: &Path) -> Result<Self> {
        let municipalities = load_municipalities(catalog)?;
        let sectors = load_sectors(sectors_path)?;
        let spec_sha256 = sha256(SPEC.as_bytes());
        let catalogue_sha256 = format!("{}-{}", sha256(&fs::read(catalog)?), sha256(&fs::read(sectors_path)?));
        Ok(Self::assemble(municipalities, sectors, uf_table(), &spec_sha256, &catalogue_sha256))
    }
}

#[cfg(test)]
fn muni(id: u16, name: &str, uf: &str, prep: &str, population: u32, test_only: bool) -> MunicipalityRow {
    MunicipalityRow { id, name: name.into(), uf: uf.into(), prep: prep.into(), population, test_only }
}

#[cfg(test)]
fn sector(id: u16, singular: &str, plural: &str, feminine: bool, cnaes: Vec<u32>, aliases: &[&str], test_only: bool) -> SectorRow {
    SectorRow {
        id,
        singular: singular.into(),
        plural: plural.into(),
        feminine,
        cnaes,
        aliases: aliases.iter().map(|s| (*s).into()).collect(),
        test_only,
    }
}

fn uf_table() -> Vec<UfRow> {
    const UFS: [(&str, &str, &str); 27] = [
        ("AC", "Acre", "no"),
        ("AL", "Alagoas", "em"),
        ("AM", "Amazonas", "no"),
        ("AP", "Amapá", "no"),
        ("BA", "Bahia", "na"),
        ("CE", "Ceará", "no"),
        ("DF", "Distrito Federal", "no"),
        ("ES", "Espírito Santo", "no"),
        ("GO", "Goiás", "em"),
        ("MA", "Maranhão", "no"),
        ("MG", "Minas Gerais", "em"),
        ("MS", "Mato Grosso do Sul", "no"),
        ("MT", "Mato Grosso", "em"),
        ("PA", "Pará", "no"),
        ("PB", "Paraíba", "na"),
        ("PE", "Pernambuco", "em"),
        ("PI", "Piauí", "no"),
        ("PR", "Paraná", "no"),
        ("RJ", "Rio de Janeiro", "no"),
        ("RN", "Rio Grande do Norte", "no"),
        ("RO", "Rondônia", "em"),
        ("RR", "Roraima", "em"),
        ("RS", "Rio Grande do Sul", "no"),
        ("SC", "Santa Catarina", "em"),
        ("SE", "Sergipe", "em"),
        ("SP", "São Paulo", "em"),
        ("TO", "Tocantins", "no"),
    ];
    UFS.iter()
        .filter_map(|(code, name, prep)| {
            agg::uf_id(code).map(|id| UfRow {
                id,
                name: (*name).into(),
                prep: (*prep).into(),
                test_only: xxhash_rust::xxh3::xxh3_64(name.as_bytes()).is_multiple_of(5),
            })
        })
        .collect()
}

fn load_municipalities(path: &Path) -> Result<Vec<MunicipalityRow>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut names = BTreeMap::new();
    for line in text.lines().skip(1) {
        let columns = line.split('\t').collect::<Vec<_>>();
        if columns.len() > 4 {
            *names.entry(columns[4].to_string()).or_insert(0_usize) += 1;
        }
    }
    let preps = [
        ("Rio de Janeiro", "no"),
        ("Recife", "no"),
        ("Porto Alegre", "em"),
        ("São Paulo", "em"),
        ("Salvador", "em"),
        ("Belém", "em"),
        ("Manaus", "em"),
    ];
    Ok(text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let columns = line.split('\t').collect::<Vec<_>>();
            if columns.len() < 10 {
                return None;
            }
            let id = columns[0].parse().ok()?;
            let name = columns[4].to_string();
            let uf = columns[3].to_string();
            let prep = preps.iter().find(|(n, _)| *n == name).map_or("em", |(_, p)| p).to_string();
            Some(MunicipalityRow {
                id,
                name: name.clone(),
                uf,
                prep,
                population: columns[8].parse().unwrap_or(0),
                test_only: xxhash_rust::xxh3::xxh3_64(name.as_bytes()).is_multiple_of(5),
            })
        })
        .collect::<Vec<_>>())
}

fn load_sectors(path: &Path) -> Result<Vec<SectorRow>> {
    #[derive(Deserialize)]
    struct File {
        sector: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        display: Display,
        #[serde(default)]
        aliases: Vec<String>,
        #[serde(default)]
        cnae_core: Vec<String>,
        #[serde(default)]
        cnae_extended: Vec<String>,
    }
    #[derive(Deserialize)]
    struct Display {
        sing: String,
        plural: String,
        gender: String,
    }
    let file: File = toml::from_str(&fs::read_to_string(path)?)?;
    ensure!(!file.sector.is_empty(), "empty sector catalogue");
    Ok(file
        .sector
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut cnaes = Vec::new();
            for code in entry.cnae_core.iter().chain(&entry.cnae_extended) {
                if let Ok(code) = code.parse::<u32>() {
                    cnaes.push(code);
                }
            }
            let test_only = xxhash_rust::xxh3::xxh3_64(entry.display.sing.as_bytes()).is_multiple_of(5);
            SectorRow {
                id: u16::try_from(index).expect("sector count"),
                singular: entry.display.sing,
                plural: entry.display.plural,
                feminine: entry.display.gender == "f",
                cnaes,
                aliases: entry.aliases,
                test_only,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub seed: u64,
    pub per_family: usize,
    pub level: Level,
}

impl Config {
    fn templates(self, len: usize) -> usize {
        match self.level {
            Level::L1 => len.min(2),
            Level::L2 => len.min(5),
            Level::L3 => len,
        }
    }

    fn coverage(self) -> usize {
        match self.level {
            Level::L1 => 25,
            Level::L2 => 50,
            Level::L3 => 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Split {
    Train,
    Val,
    Test,
}

impl Split {
    fn name(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Val => "val",
            Self::Test => "test",
        }
    }
}

fn family_split(id: &str) -> Split {
    match xxhash_rust::xxh3::xxh3_64(id.as_bytes()) % 100 {
        0..=69 => Split::Train,
        70..=84 => Split::Val,
        _ => Split::Test,
    }
}

#[derive(Debug, Clone)]
struct Sampled {
    place: Option<usize>,
    place2: Option<usize>,
    uf: Option<usize>,
    sector: Option<usize>,
    sector2: Option<usize>,
    status: Option<StatusSet>,
    status_phrase: String,
    porte: Option<Porte>,
    mei: bool,
    contact: Option<ContactNeed>,
    capital: bool,
    period: PeriodChoice,
    k: u8,
    n: u16,
    cnpj: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum PeriodChoice {
    Year,
    Last12,
    CurrentYear,
    Last90,
    Month,
}

impl PeriodChoice {
    fn text(self) -> &'static str {
        match self {
            Self::Year => "em 2025",
            Self::Last12 => "nos últimos doze meses",
            Self::CurrentYear => "neste ano",
            Self::Last90 => "nos últimos noventa dias",
            Self::Month => "em abril de 2026",
        }
    }

    fn days(self) -> DayRange {
        let from = match self {
            Self::Year => Day::from_ymd(2025, 1, 1),
            Self::Last12 => Day::from_ymd(2025, 10, 12),
            Self::CurrentYear => Day::from_ymd(2026, 1, 1),
            Self::Last90 => Day::from_ymd(2026, 7, 14),
            Self::Month => Day::from_ymd(2026, 4, 1),
        };
        let to = match self {
            Self::Year => Day::from_ymd(2025, 12, 31),
            Self::Month => Day::from_ymd(2026, 4, 30),
            _ => Some(ANCHOR),
        };
        Range { from, to }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Gold {
    pub act: String,
    pub decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir: Option<Request>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub family: String,
    pub template: usize,
    pub register: String,
    pub noise: Vec<String>,
    pub split: String,
    #[serde(rename = "gen")]
    pub generator: &'static str,
    pub seed: u64,
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub gold: Gold,
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize)]
pub struct Turn {
    pub text: String,
    pub gold: Gold,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dialogue {
    pub id: String,
    pub turns: Vec<Turn>,
    pub meta: Meta,
}

pub struct Entry {
    pub item: Item,
    pub form: String,
    pub intent: String,
    place: Option<u16>,
    sector: Option<u16>,
}

pub struct Dataset {
    pub entries: Vec<Entry>,
    pub dialogues: Vec<Dialogue>,
    pub manifest: Manifest,
}

#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    pub dataset_id: String,
    pub generator: &'static str,
    pub git_commit: String,
    pub seed: u64,
    pub spec_sha256: String,
    pub catalogue_sha256: String,
    pub counts: Counts,
    pub diversity: Diversity,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Counts {
    pub items: usize,
    pub dialogues: usize,
    pub by_split: BTreeMap<String, usize>,
    pub by_intent: BTreeMap<String, usize>,
    pub by_family: BTreeMap<String, usize>,
    pub by_noise: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Diversity {
    pub distinct_forms: BTreeMap<String, usize>,
    pub templates_per_family: BTreeMap<String, usize>,
    pub slot_coverage: BTreeMap<String, f64>,
    pub token_entropy: f64,
    pub char_entropy: f64,
    pub mean_jaccard: BTreeMap<String, f64>,
    pub noise_share: BTreeMap<String, f64>,
}

pub fn load_families(text: &str) -> Result<Vec<FamilySpec>> {
    let file: SpecFile = toml::from_str(text)?;
    ensure!(file.family.len() >= 60, "fewer than 60 families");
    for family in &file.family {
        ensure!(family.templates.len() >= 8, "{} has fewer than 8 templates", family.id);
        ensure!(family.authors.len() >= 2, "{} has fewer than two authors", family.id);
        let mut seen = BTreeSet::new();
        for template in &family.templates {
            ensure!(seen.insert(template), "{} repeats a template", family.id);
        }
    }
    Ok(file.family)
}

pub fn generate(families: &[FamilySpec], catalogues: &Catalogues, config: Config, git_commit: &str) -> Result<Dataset> {
    let mut entries = Vec::new();
    let mut dialogues = Vec::new();
    let mut counter = 0_u64;
    let mut dialogue_counter = 0_u64;
    for family in families {
        let split = family_split(&family.id);
        let job = Job { family, catalogues, config, split };
        let templates = config.templates(family.templates.len());
        if family.op == "dialogue" {
            for index in 0..config.per_family {
                dialogue_counter += 1;
                let id = format!("d-{dialogue_counter:05}");
                let mut rng = Rng::new(config.seed ^ xxhash_rust::xxh3::xxh3_64(id.as_bytes()));
                let dialogue = dialogue(&job, templates, index, &id, &mut rng)?;
                dialogues.push(dialogue);
            }
            continue;
        }
        for index in 0..config.per_family {
            counter += 1;
            let id = format!("s-{counter:06}");
            let mut rng = Rng::new(config.seed ^ xxhash_rust::xxh3::xxh3_64(id.as_bytes()));
            let mut sampled = sample(family, catalogues, config, split, &mut rng)?;
            let template_index = index % templates;
            let template = &family.templates[template_index];
            let item = item(&job, &mut sampled, template, template_index, &id, &mut rng)?;
            let form = delex(&item.text, &sampled, catalogues);
            entries.push(Entry {
                item,
                form,
                intent: family.op.clone(),
                place: sampled.place.map(|index| catalogues.municipalities[index].id),
                sector: sampled.sector.map(|index| catalogues.sectors[index].id),
            });
        }
    }
    entries = dedupe(entries);
    let manifest = manifest(families, catalogues, config, git_commit, &entries, &dialogues);
    Ok(Dataset { entries, dialogues, manifest })
}

struct Job<'a> {
    family: &'a FamilySpec,
    catalogues: &'a Catalogues,
    config: Config,
    split: Split,
}

fn item(job: &Job<'_>, sampled: &mut Sampled, template: &str, template_index: usize, id: &str, rng: &mut Rng) -> Result<Item> {
    let family = job.family;
    let catalogues = job.catalogues;
    let split = job.split;
    if family.id == "list.contact" {
        sampled.contact = Some(if template.contains("e-mail") && template.contains("telefone") {
            ContactNeed::Both
        } else if template.contains("e-mail") {
            ContactNeed::Email
        } else if template.contains("telefone") {
            ContactNeed::Phone
        } else {
            ContactNeed::Both
        });
    }
    if family.id == "list.capital" {
        sampled.capital = template.contains("acima de");
    }
    let text = realize(template, sampled, family, catalogues);
    let (text, noise, register) = noisify(text, rng, split);
    let gold = if family.op == "reject" {
        Gold {
            act: "new_query".into(),
            decision: "reject".into(),
            ir: None,
            reason: Some(sampled.reason.clone().unwrap_or_else(|| "Consulta fora do escopo dos dados abertos.".into())),
        }
    } else {
        let request = frame(family, catalogues, sampled)?;
        let canonical = catalogues.canonical(request)?;
        Gold { act: "new_query".into(), decision: "execute".into(), ir: Some(canonical.as_request().clone()), reason: None }
    };
    Ok(Item {
        id: id.into(),
        text,
        gold,
        meta: Meta {
            family: family.id.clone(),
            template: template_index,
            register,
            noise,
            split: split.name().into(),
            generator: VERSION,
            seed: job.config.seed,
            authors: family.authors.clone(),
        },
    })
}

fn dialogue(job: &Job<'_>, templates: usize, index: usize, id: &str, rng: &mut Rng) -> Result<Dialogue> {
    let family = job.family;
    let catalogues = job.catalogues;
    let split = job.split;
    let mut base = sample(family, catalogues, job.config, split, rng)?;
    base.period = PeriodChoice::Year;
    let base_template = "quantas {sector.plural} existem em {place.prep}";
    let base_text = realize(base_template, &base, family, catalogues);
    let base_request = catalogues.canonical(Request {
        op: Op::Count { metric: Metric::Stock },
        unit: Unit::Establishments,
        scope: Filters {
            place: base.place.map_or(PlaceFilter::Any, |index| {
                PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(catalogues.municipalities[index].id)]))
            }),
            activity: base.sector.map_or(ActivityFilter::Any, |index| {
                let sector = &catalogues.sectors[index];
                ActivityFilter::Sectors {
                    ids: SmallSet::new([SectorId(sector.id)]),
                    cnaes: SmallSet::new(sector.cnaes.iter().map(|code| CnaeCode::Subclass(*code))),
                    scope: CnaeScope::Main,
                }
            }),
            status: Some(StatusSet::only(Status::Active)),
            ..Filters::default()
        },
        time: TimeSpec { competence: CURRENT, period: None, anchor: ANCHOR },
        output: Output::default(),
        versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
    })?;
    let follow = sample(family, catalogues, job.config, split, rng)?;
    let template_index = index % templates;
    let template = &family.templates[template_index];
    let text = realize(template, &follow, family, catalogues);
    let (text, noise, register) = noisify(text, rng, split);
    let follow_request = catalogues.canonical(frame(family, catalogues, &follow)?)?;
    let meta = Meta {
        family: family.id.clone(),
        template: template_index,
        register,
        noise,
        split: split.name().into(),
        generator: VERSION,
        seed: job.config.seed,
        authors: family.authors.clone(),
    };
    Ok(Dialogue {
        id: id.into(),
        turns: vec![
            Turn {
                text: base_text,
                gold: Gold {
                    act: "new_query".into(),
                    decision: "execute".into(),
                    ir: Some(base_request.as_request().clone()),
                    reason: None,
                },
            },
            Turn {
                text,
                gold: Gold {
                    act: "replace_slot".into(),
                    decision: "execute".into(),
                    ir: Some(follow_request.as_request().clone()),
                    reason: None,
                },
            },
        ],
        meta,
    })
}

fn sample(family: &FamilySpec, catalogues: &Catalogues, config: Config, split: Split, rng: &mut Rng) -> Result<Sampled> {
    let municipality_pool = pick_pool(&catalogues.municipalities, split, |row| row.test_only);
    let sector_pool = pick_pool(&catalogues.sectors, split, |row| row.test_only);
    let uf_pool = pick_pool(&catalogues.ufs, split, |row| row.test_only);
    let coverage = config.coverage();
    let sample_index = |pool: &[usize], table_len: usize, rng: &mut Rng| -> usize {
        let limit = (pool.len() * coverage / 100).max(1);
        pool[rng.below(limit.min(pool.len()))].min(table_len - 1)
    };
    let place = if family.place == "municipality" || family.slots.iter().any(|slot| slot == "place") {
        Some(sample_index(&municipality_pool, catalogues.municipalities.len(), rng))
    } else {
        None
    };
    let uf = if family.place == "uf" || family.slots.iter().any(|slot| slot == "uf") {
        Some(sample_index(&uf_pool, catalogues.ufs.len(), rng))
    } else {
        None
    };
    let place2 = if family.slots.iter().any(|slot| slot == "place2") {
        let mut candidate = sample_index(&municipality_pool, catalogues.municipalities.len(), rng);
        if place == Some(candidate) {
            candidate = municipality_pool
                [(municipality_pool.iter().position(|&x| x == candidate).unwrap_or(0) + 1) % municipality_pool.len()]
            .min(catalogues.municipalities.len() - 1);
        }
        Some(candidate)
    } else {
        None
    };
    let sector = if family.sector { Some(sample_index(&sector_pool, catalogues.sectors.len(), rng)) } else { None };
    let sector2 = if family.slots.iter().any(|slot| slot == "sector2") {
        let mut candidate = sample_index(&sector_pool, catalogues.sectors.len(), rng);
        if sector == Some(candidate) {
            candidate = sector_pool[(sector_pool.iter().position(|&x| x == candidate).unwrap_or(0) + 1) % sector_pool.len()]
                .min(catalogues.sectors.len() - 1);
        }
        Some(candidate)
    } else {
        None
    };
    let status = match family.status.as_str() {
        "closed" => Some(StatusSet::only(Status::Closed)),
        "any" => Some(StatusSet::ALL),
        _ => Some(StatusSet::only(Status::Active)),
    };
    let (status_phrase, _) = status_phrase(family, catalogues, sector, rng);
    let porte = if family.slots.iter().any(|slot| slot == "porte") {
        Some(match rng.below(3) {
            0 => Porte::Micro,
            1 => Porte::Small,
            _ => Porte::Other,
        })
    } else {
        None
    };
    let mei = family.slots.iter().any(|slot| slot == "mei");
    let contact = family.slots.iter().any(|slot| slot == "contact").then_some(ContactNeed::Phone);
    let capital = family.slots.iter().any(|slot| slot == "capital");
    let period = if family.slots.iter().any(|slot| slot == "period" || slot == "n") {
        let mut choice = match rng.below(5) {
            0 => PeriodChoice::Year,
            1 => PeriodChoice::Last12,
            2 => PeriodChoice::CurrentYear,
            3 => PeriodChoice::Last90,
            _ => PeriodChoice::Month,
        };
        if family.op == "trend" && (matches!(choice, PeriodChoice::Month) || family.grain.as_deref() == Some("year")) {
            choice = PeriodChoice::Last12;
        }
        choice
    } else {
        PeriodChoice::Year
    };
    let k = if family.slots.iter().any(|slot| slot == "k") { 5 } else { 0 };
    let n = if family.slots.iter().any(|slot| slot == "n") { [6, 12, 90][rng.below(3)] } else { 0 };
    let cnpj = if family.slots.iter().any(|slot| slot == "cnpj") {
        ["12.ABC.345/01DE-35", "33.000.167/0001-01"][rng.below(2)].to_string()
    } else {
        String::new()
    };
    let reason = match family.id.as_str() {
        "reject.people" => Some("Consulta a pessoas físicas e sócios não é permitida; use o CNPJ da empresa.".into()),
        "reject.ood" => Some("A pergunta está fora do domínio dos dados abertos do CNPJ.".into()),
        "reject.revenue" => Some("Faturamento não está nos dados abertos; capital social é outra medida.".into()),
        "reject.employees" => Some("Número de funcionários não está nos dados abertos.".into()),
        "reject.opportunity" => Some("Recomendação de oportunidade não pode ser inferida apenas de contagens de concorrentes.".into()),
        _ => None,
    };
    Ok(Sampled {
        place,
        place2,
        uf,
        sector,
        sector2,
        status: if matches!(family.metric.as_str(), "flow:closures") { Some(StatusSet::only(Status::Closed)) } else { status },
        status_phrase,
        porte,
        mei,
        contact,
        capital,
        period,
        k,
        n,
        cnpj,
        reason,
    })
}

fn pick_pool<T>(rows: &[T], split: Split, test_only: impl Fn(&T) -> bool) -> Vec<usize> {
    let prefer_test = split == Split::Test;
    let mut preferred = Vec::new();
    let mut fallback = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if test_only(row) == prefer_test {
            preferred.push(index);
        } else {
            fallback.push(index);
        }
    }
    if preferred.is_empty() { fallback } else { preferred }
}

fn status_phrase(family: &FamilySpec, catalogues: &Catalogues, sector: Option<usize>, rng: &mut Rng) -> (String, bool) {
    let feminine = sector.is_some_and(|index| catalogues.sectors[index].feminine);
    let singular = match family.status.as_str() {
        "closed" => {
            if feminine {
                "baixada"
            } else {
                "baixado"
            }
        }
        _ => {
            if feminine {
                "ativa"
            } else {
                "ativo"
            }
        }
    };
    let plural = match family.status.as_str() {
        "closed" => {
            if feminine {
                "baixadas"
            } else {
                "baixados"
            }
        }
        _ => {
            if feminine {
                "ativas"
            } else {
                "ativos"
            }
        }
    };
    let include = family.status == "closed" || rng.below(100) < 55;
    (if include { plural.into() } else { String::new() }, include || family.status == "closed" || singular.is_empty())
}

fn frame(family: &FamilySpec, catalogues: &Catalogues, sampled: &Sampled) -> Result<Request> {
    let metric = metric(family)?;
    let flow = matches!(metric, Metric::Flow(_));
    let active_change = matches!(metric, Metric::ActiveChange { .. });
    let status = sampled.status;
    let place = match family.place.as_str() {
        "uf" => PlaceFilter::Ufs(SmallSet::new([catalogues.ufs[sampled.uf.context("missing UF")?].id])),
        "any" => PlaceFilter::Any,
        _ if matches!(family.op.as_str(), "compare") && family.dim.as_deref() == Some("municipality") => PlaceFilter::Any,
        _ => match sampled.place {
            Some(index) => PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(catalogues.municipalities[index].id)])),
            None => PlaceFilter::Any,
        },
    };
    let activity = if family.sector && family.dim.as_deref() != Some("sector") {
        let sector = &catalogues.sectors[sampled.sector.context("missing sector")?];
        ActivityFilter::Sectors {
            ids: SmallSet::new([SectorId(sector.id)]),
            cnaes: SmallSet::new(sector.cnaes.iter().map(|code| CnaeCode::Subclass(*code))),
            scope: CnaeScope::Main,
        }
    } else {
        ActivityFilter::Any
    };
    let period = if flow || active_change {
        if active_change {
            Some(Period::Competences { from: Competence(315), to: CURRENT })
        } else {
            Some(Period::Days(sampled.period.days()))
        }
    } else {
        None
    };
    let op = match family.op.as_str() {
        "count" => Op::Count { metric },
        "compare" => {
            let dim = match family.dim.as_deref() {
                Some("sector") => Dim::Sector,
                _ => Dim::Municipality,
            };
            let members = match dim {
                Dim::Sector => SmallSet::new([
                    Member::Sector(SectorId(catalogues.sectors[sampled.sector.context("member")?].id)),
                    Member::Sector(SectorId(catalogues.sectors[sampled.sector2.context("member2")?].id)),
                ]),
                _ => SmallSet::new([
                    Member::Municipality(MunicipalityId(catalogues.municipalities[sampled.place.context("member")?].id)),
                    Member::Municipality(MunicipalityId(catalogues.municipalities[sampled.place2.context("member2")?].id)),
                ]),
            };
            Op::Compare { dim, members, metric }
        }
        "rank" => {
            let dim = match family.dim.as_deref() {
                Some("uf") => Dim::Uf,
                Some("sector") => Dim::Sector,
                _ => Dim::Municipality,
            };
            Op::Rank { dim, metric, dir: Dir::Desc, k: sampled.k, min_base: 30 }
        }
        "breakdown" => {
            let dim = match family.dim.as_deref() {
                Some("status") => Dim::Status,
                Some("mei") => Dim::Mei,
                Some("head_office") => Dim::HeadOffice,
                Some("uf") => Dim::Uf,
                _ => Dim::Porte,
            };
            Op::Breakdown { dim, metric }
        }
        "trend" => {
            let flow_metric = match metric {
                Metric::Flow(flow) => flow,
                _ => bail!("trend needs a flow metric"),
            };
            let grain = if family.grain.as_deref() == Some("year") { Grain::Year } else { Grain::Month };
            Op::Trend { metric: flow_metric, grain }
        }
        "list" => Op::List { order: if sampled.capital { ListOrder::CapitalDesc } else { ListOrder::NewestFirst }, limit: 10 },
        "lookup" => Op::Lookup { id: CnpjRef::parse(&sampled.cnpj).context("fixture CNPJ")? },
        "meta" => Op::Meta { topic: if family.id.ends_with("method") { MetaTopic::Methodology } else { MetaTopic::Sources } },
        "dialogue" => {
            if family.id.contains("compare") || family.id.contains("followup_place") {
                Op::Compare {
                    dim: Dim::Municipality,
                    members: SmallSet::new([
                        Member::Municipality(MunicipalityId(catalogues.municipalities[sampled.place.context("dialogue place")?].id)),
                        Member::Municipality(MunicipalityId(catalogues.municipalities[sampled.place2.context("dialogue place2")?].id)),
                    ]),
                    metric,
                }
            } else if family.id.contains("followup_sector") {
                Op::Compare {
                    dim: Dim::Sector,
                    members: SmallSet::new([
                        Member::Sector(SectorId(catalogues.sectors[sampled.sector.context("dialogue sector")?].id)),
                        Member::Sector(SectorId(catalogues.sectors[sampled.sector2.context("dialogue sector2")?].id)),
                    ]),
                    metric,
                }
            } else {
                Op::Count { metric }
            }
        }
        _ => bail!("unknown op {}", family.op),
    };
    let scope = Filters {
        place,
        activity,
        status,
        porte: sampled.porte.map_or(PorteSet::ALL, PorteSet::only),
        mei: if sampled.mei { Tri::Yes } else { Tri::Any },
        contact: sampled.contact.unwrap_or(ContactNeed::Any),
        capital: sampled.capital.then_some(Range { from: Some(500_000_000), to: None }),
        ..Filters::default()
    };
    let unit = if family.id == "list.head" { Unit::HeadOffices } else { Unit::Establishments };
    Ok(Request {
        op,
        unit,
        scope,
        time: TimeSpec { competence: CURRENT, period, anchor: ANCHOR },
        output: Output::default(),
        versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
    })
}

fn metric(family: &FamilySpec) -> Result<Metric> {
    Ok(match family.metric.as_str() {
        "stock" => Metric::Stock,
        "per10k" => Metric::Per10k(PopulationRef::IbgeEstimate(2025)),
        "inhabitants_per" => Metric::InhabitantsPer(PopulationRef::IbgeEstimate(2025)),
        "share" => Metric::Share,
        "active_change" => Metric::ActiveChange { pct: false },
        value if value.starts_with("flow:") => Metric::Flow(match &value[5..] {
            "registrations" => FlowMetric::Registrations,
            "closures" => FlowMetric::Closures,
            "net_registrations" => FlowMetric::NetRegistrations,
            "mei_opt_ins" => FlowMetric::MeiOptIns,
            "mei_exits" => FlowMetric::MeiExits,
            other => bail!("unknown flow metric {other}"),
        }),
        other => bail!("unknown metric {other}"),
    })
}

fn de_prep(prep: &str) -> &'static str {
    match prep {
        "no" => "do",
        "na" => "da",
        _ => "de",
    }
}

fn com_prep(prep: &str) -> &'static str {
    match prep {
        "no" => "com o",
        "na" => "com a",
        _ => "com",
    }
}

fn realize(template: &str, sampled: &Sampled, family: &FamilySpec, catalogues: &Catalogues) -> String {
    let mut values: BTreeMap<&str, String> = BTreeMap::new();
    values.insert("quant", "quantos".into());
    values.insert("sector.um", "um".into());
    if let Some(index) = sampled.place {
        let place = &catalogues.municipalities[index];
        let name = if catalogues.municipalities.iter().filter(|other| other.name == place.name).count() > 1 {
            format!("{} ({})", place.name, place.uf)
        } else {
            place.name.clone()
        };
        values.insert("place", name.clone());
        values.insert("place.prep", format!("{} {}", place.prep, name));
        values.insert("place.de", format!("{} {}", de_prep(&place.prep), name));
        values.insert("place.com", format!("{} {}", com_prep(&place.prep), name));
    }
    if let Some(index) = sampled.place2 {
        let place = &catalogues.municipalities[index];
        let name = if catalogues.municipalities.iter().filter(|other| other.name == place.name).count() > 1 {
            format!("{} ({})", place.name, place.uf)
        } else {
            place.name.clone()
        };
        values.insert("place2", name.clone());
        values.insert("place2.prep", format!("{} {}", place.prep, name));
        values.insert("place2.de", format!("{} {}", de_prep(&place.prep), name));
        values.insert("place2.com", format!("{} {}", com_prep(&place.prep), name));
    }
    if let Some(index) = sampled.uf {
        let uf = &catalogues.ufs[index];
        values.insert("uf", uf.name.clone());
        values.insert("uf.prep", format!("{} {}", uf.prep, uf.name));
        values.insert("uf.de", format!("{} {}", de_prep(&uf.prep), uf.name));
    }
    if let Some(index) = sampled.sector {
        let sector = &catalogues.sectors[index];
        values.insert("sector", sector.singular.clone());
        values.insert("sector.plural", sector.plural.clone());
        values.insert("sector.alias", sector.aliases.first().cloned().unwrap_or_else(|| sector.singular.clone()));
        if let Some(place) = sampled.place {
            values.insert("population", catalogues.municipalities[place].population.to_string());
        }
        values.insert("sector.art", if sector.feminine { "a".into() } else { "o".into() });
        values.insert("sector.art_plural", if sector.feminine { "as".into() } else { "os".into() });
        values.insert("sector.um", if sector.feminine { "uma".into() } else { "um".into() });
        values.insert("quant", if sector.feminine { "quantas".into() } else { "quantos".into() });
    }
    if let Some(index) = sampled.sector2 {
        let sector = &catalogues.sectors[index];
        values.insert("sector2", sector.singular.clone());
        values.insert("sector2.plural", sector.plural.clone());
    }
    values.insert("status?", sampled.status_phrase.clone());
    values.insert("status", sampled.status_phrase.clone());
    values.insert(
        "porte",
        sampled.porte.map_or_else(String::new, |porte| {
            match porte {
                Porte::Micro => "micro",
                Porte::Small => "pequeno porte",
                Porte::Other => "demais",
                Porte::Unreported => "não informado",
            }
            .into()
        }),
    );
    values.insert("mei", if sampled.mei { "MEI".into() } else { String::new() });
    values.insert(
        "contact",
        match sampled.contact {
            Some(ContactNeed::Phone) => "com telefone",
            Some(ContactNeed::Email) => "com e-mail",
            Some(ContactNeed::Both) => "com telefone ou e-mail",
            Some(ContactNeed::Any) | None => "",
        }
        .into(),
    );
    values.insert("capital", if sampled.capital { "capital acima de R$ 5 milhões".into() } else { String::new() });
    values.insert("money", "R$ 5 milhões".into());
    values.insert("period", sampled.period.text().into());
    values.insert("year", "2025".into());
    values.insert("month", "abril".into());
    values.insert("n", sampled.n.to_string());
    values.insert("k", sampled.k.to_string());
    values.insert("min_base", "30".into());
    values.insert("cnpj", sampled.cnpj.clone());
    values.insert(
        "dim",
        match family.dim.as_deref() {
            Some("status") => "situação",
            Some("mei") => "MEI",
            Some("head_office") => "matriz ou filial",
            Some("uf") => "UF",
            _ => "porte",
        }
        .into(),
    );
    values.insert("topic", if family.id.ends_with("method") { "metodologia".into() } else { "fontes".into() });
    values.insert("reason", sampled.reason.clone().unwrap_or_default());
    values.insert("exist", ["existem", "há", "tem"][(sampled.place.unwrap_or(0) + sampled.sector.unwrap_or(0)) % 3].into());
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let name = &after[..close];
        out.push_str(values.get(name).map(String::as_str).unwrap_or(""));
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn delex(text: &str, sampled: &Sampled, catalogues: &Catalogues) -> String {
    let mut form = text.to_lowercase();
    if let Some(index) = sampled.place {
        form = form.replace(&catalogues.municipalities[index].name.to_lowercase(), "[place]");
    }
    if let Some(index) = sampled.place2 {
        form = form.replace(&catalogues.municipalities[index].name.to_lowercase(), "[place2]");
    }
    if let Some(index) = sampled.sector {
        form = form.replace(&catalogues.sectors[index].plural.to_lowercase(), "[sector]");
        form = form.replace(&catalogues.sectors[index].singular.to_lowercase(), "[sector]");
    }
    if let Some(index) = sampled.sector2 {
        form = form.replace(&catalogues.sectors[index].plural.to_lowercase(), "[sector2]");
        form = form.replace(&catalogues.sectors[index].singular.to_lowercase(), "[sector2]");
    }
    form.split_whitespace().map(|word| if word.chars().any(|ch| ch.is_ascii_digit()) { "[n]" } else { word }).collect::<Vec<_>>().join(" ")
}

fn noisify(text: String, rng: &mut Rng, split: Split) -> (String, Vec<String>, String) {
    let question = text.split_whitespace().next().is_some_and(|word| {
        matches!(
            word.to_lowercase().trim_matches(|ch: char| !ch.is_alphabetic()),
            "quantas" | "quantos" | "qual" | "quais" | "como" | "onde" | "quanto" | "quem"
        )
    });
    let register = if question {
        match rng.below(10) {
            0..=1 => "formal",
            2..=3 => "colloquial",
            _ => "neutral",
        }
    } else {
        "neutral"
    };
    let mut text = match register {
        "formal" => format!("Gostaria de saber {}", text),
        "colloquial" => format!("me fala aí: {}", text),
        _ => text,
    };
    let mut applied = Vec::new();
    let severity = rng.below(10);
    if severity >= 4 {
        let mut candidates = vec!["abbrev", "no_accent", "typo", "case", "punct", "filler", "fragment"];
        if split == Split::Test {
            candidates.push("phonetic");
            candidates.push("merged");
        }
        let count = if severity >= 7 { 2 } else { 1 };
        for _ in 0..count {
            let kind = candidates[rng.below(candidates.len())];
            text = match kind {
                "abbrev" => abbrev(&text),
                "no_accent" => no_accent(&text),
                "typo" => typo(&text, rng),
                "case" => {
                    if rng.below(2) == 0 {
                        text.to_uppercase()
                    } else {
                        capitalize(&text)
                    }
                }
                "punct" => {
                    if rng.below(2) == 0 {
                        text.replace('?', "")
                    } else {
                        format!("{text}??")
                    }
                }
                "filler" => {
                    if rng.below(2) == 0 {
                        format!("bom dia, {text}")
                    } else {
                        format!("{text} por favor")
                    }
                }
                "fragment" => fragment(&text),
                "phonetic" => phonetic(&text),
                "merged" => merged(&text, rng),
                _ => text,
            };
            applied.push(kind.to_string());
        }
    }
    (text.split_whitespace().collect::<Vec<_>>().join(" "), applied, register.into())
}

fn abbrev(text: &str) -> String {
    let mut out = text.to_string();
    for (from, to) in [
        ("quantas", "qtas"),
        ("número", "numero"),
        ("você", "vc"),
        ("por favor", "pfv"),
        ("também", "tb"),
        ("mesmo", "msm"),
        ("para", "p/"),
        ("com", "c/"),
    ] {
        out = out.replace(from, to);
    }
    out
}

fn no_accent(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn typo(text: &str, rng: &mut Rng) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    if chars.len() < 4 {
        return text.into();
    }
    let index = rng.below(chars.len() - 1);
    match rng.below(3) {
        0 => {
            chars.insert(index, chars[index]);
        }
        1 => {
            chars.remove(index);
        }
        _ => {
            chars.swap(index, index + 1);
        }
    }
    chars.into_iter().collect()
}

fn phonetic(text: &str) -> String {
    text.replace('s', "ss").replace("ch", "x").replace('z', "s")
}

fn merged(text: &str, rng: &mut Rng) -> String {
    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.len() < 2 {
        return text.into();
    }
    let index = rng.below(words.len() - 1);
    let merged = format!("{}{}", words[index], words[index + 1]);
    let mut result: Vec<&str> = words[..index].to_vec();
    result.push(merged.as_str());
    result.extend(&words[index + 2..]);
    result.join(" ")
}

fn fragment(text: &str) -> String {
    let stop = ["quantas", "quantos", "qual", "quais", "me", "diz", "gostaria", "de", "saber"];
    let words = text.split_whitespace().filter(|word| !stop.contains(&word.to_lowercase().as_str())).collect::<Vec<_>>();
    if words.is_empty() { text.into() } else { words.join(" ") }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn dedupe(entries: Vec<Entry>) -> Vec<Entry> {
    let mut forms: BTreeMap<String, BTreeMap<Split, usize>> = BTreeMap::new();
    for entry in &entries {
        let split = split_from_name(&entry.item.meta.split);
        *forms.entry(entry.form.clone()).or_default().entry(split).or_default() += 1;
    }
    let mut kept = Vec::new();
    for entry in entries {
        let split = split_from_name(&entry.item.meta.split);
        let counts = &forms[&entry.form];
        if split == Split::Train && (counts.contains_key(&Split::Val) || counts.contains_key(&Split::Test)) {
            continue;
        }
        if counts[&split] > 3
            && kept.iter().filter(|other: &&Entry| other.form == entry.form && other.item.meta.split == entry.item.meta.split).count() >= 3
        {
            continue;
        }
        kept.push(entry);
    }
    kept
}

fn split_from_name(name: &str) -> Split {
    match name {
        "val" => Split::Val,
        "test" => Split::Test,
        _ => Split::Train,
    }
}

fn manifest(
    families: &[FamilySpec],
    catalogues: &Catalogues,
    config: Config,
    git_commit: &str,
    entries: &[Entry],
    dialogues: &[Dialogue],
) -> Manifest {
    let mut counts = Counts { items: entries.len(), dialogues: dialogues.len(), ..Counts::default() };
    let mut forms: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut used_places = BTreeSet::new();
    let mut used_sectors = BTreeSet::new();
    let mut tokens: BTreeMap<String, usize> = BTreeMap::new();
    let mut chars: BTreeMap<Vec<char>, usize> = BTreeMap::new();
    let mut noise_total = 0_usize;
    for entry in entries {
        *counts.by_split.entry(entry.item.meta.split.clone()).or_default() += 1;
        *counts.by_intent.entry(entry.intent.clone()).or_default() += 1;
        *counts.by_family.entry(entry.item.meta.family.clone()).or_default() += 1;
        forms.entry(entry.intent.clone()).or_default().insert(entry.form.clone());
        if let Some(place) = entry.place {
            used_places.insert(place);
        }
        if let Some(sector) = entry.sector {
            used_sectors.insert(sector);
        }
        for token in entry.item.text.split_whitespace() {
            *tokens.entry(token.to_lowercase()).or_default() += 1;
        }
        let chars_vec: Vec<char> = entry.item.text.chars().collect();
        for window in chars_vec.windows(4) {
            *chars.entry(window.to_vec()).or_default() += 1;
        }
        for noise in &entry.item.meta.noise {
            *counts.by_noise.entry(noise.clone()).or_default() += 1;
            noise_total += 1;
        }
    }
    let token_entropy = entropy(tokens.values().copied());
    let char_entropy = entropy(chars.values().copied());
    let mut mean_jaccard = BTreeMap::new();
    for (intent, set) in &forms {
        let forms = set.iter().collect::<Vec<_>>();
        if forms.len() < 2 {
            mean_jaccard.insert(intent.clone(), 0.0);
            continue;
        }
        let mut sum = 0.0;
        let mut pairs = 0_usize;
        for i in 0..forms.len().min(200) {
            for j in (i + 1)..forms.len().min(200) {
                sum += jaccard(forms[i], forms[j]);
                pairs += 1;
            }
        }
        mean_jaccard.insert(intent.clone(), if pairs == 0 { 0.0 } else { sum / pairs as f64 });
    }
    let mut noise_share = BTreeMap::new();
    for (noise, count) in &counts.by_noise {
        noise_share.insert(noise.clone(), if noise_total == 0 { 0.0 } else { *count as f64 / noise_total as f64 });
    }
    let dataset_id = format!("{VERSION}-{}-{}", &catalogues.spec_sha256[..8.min(catalogues.spec_sha256.len())], config.seed);
    Manifest {
        dataset_id,
        generator: VERSION,
        git_commit: git_commit.into(),
        seed: config.seed,
        spec_sha256: catalogues.spec_sha256.clone(),
        catalogue_sha256: catalogues.catalogue_sha256.clone(),
        counts,
        diversity: Diversity {
            distinct_forms: forms.iter().map(|(intent, set)| (intent.clone(), set.len())).collect(),
            templates_per_family: families.iter().map(|family| (family.id.clone(), family.templates.len())).collect(),
            slot_coverage: BTreeMap::from([
                ("municipalities".into(), used_places.len() as f64 * 100.0 / catalogues.municipalities.len() as f64),
                ("sectors".into(), used_sectors.len() as f64 * 100.0 / catalogues.sectors.len() as f64),
            ]),
            token_entropy,
            char_entropy,
            mean_jaccard,
            noise_share,
        },
        files: BTreeMap::new(),
    }
}

fn entropy(counts: impl Iterator<Item = usize>) -> f64 {
    let values = counts.collect::<Vec<_>>();
    let total = values.iter().sum::<usize>() as f64;
    if total == 0.0 {
        return 0.0;
    }
    -values
        .iter()
        .map(|&count| {
            let p = count as f64 / total;
            p * p.log2()
        })
        .sum::<f64>()
}

fn jaccard(a: &str, b: &str) -> f64 {
    let left: BTreeSet<&str> = a.split_whitespace().collect();
    let right: BTreeSet<&str> = b.split_whitespace().collect();
    let intersection = left.intersection(&right).count() as f64;
    let union = left.union(&right).count() as f64;
    if union == 0.0 { 0.0 } else { intersection / union }
}

pub fn write_dataset(dataset: &Dataset, out: &Path) -> Result<PathBuf> {
    let directory = out.join(&dataset.manifest.dataset_id);
    fs::create_dir_all(&directory)?;
    let mut files = BTreeMap::new();
    let mut write = |name: &str, lines: Vec<String>| -> Result<()> {
        let text = lines.join("\n") + "\n";
        fs::write(directory.join(name), &text)?;
        files.insert(name.into(), sha256(text.as_bytes()));
        Ok(())
    };
    let mut train = Vec::new();
    let mut val = Vec::new();
    let mut test = Vec::new();
    for entry in &dataset.entries {
        let line = serde_json::to_string(&entry.item)?;
        match split_from_name(&entry.item.meta.split) {
            Split::Train => train.push(line),
            Split::Val => val.push(line),
            Split::Test => test.push(line),
        }
    }
    write("train.jsonl", train)?;
    write("val.jsonl", val)?;
    write("test.jsonl", test)?;
    write("dialogues.jsonl", dataset.dialogues.iter().map(serde_json::to_string).collect::<Result<Vec<_>, _>>()?)?;
    let mut manifest = dataset.manifest.clone();
    manifest.files = files;
    let text = serde_json::to_string_pretty(&manifest)?;
    fs::write(directory.join("manifest.json"), &text)?;
    Ok(directory)
}

pub fn write_review_sample(dataset: &Dataset, directory: &Path, size: usize, seed: u64) -> Result<()> {
    let mut rng = Rng::new(seed);
    let mut indices = (0..dataset.entries.len()).collect::<Vec<_>>();
    for index in 0..indices.len() {
        let swap = rng.below(indices.len());
        indices.swap(index, swap);
    }
    let selected = indices.into_iter().take(size).collect::<Vec<_>>();
    let jsonl = selected.iter().map(|&index| serde_json::to_string(&dataset.entries[index].item)).collect::<Result<Vec<_>, _>>()?;
    fs::write(directory.join("review-sample.jsonl"), jsonl.join("\n") + "\n")?;
    let mut markdown = String::from("# Revisão de naturalidade\n\nMarque cada item como natural ou não.\n\n");
    for (position, &index) in selected.iter().enumerate() {
        let item = &dataset.entries[index].item;
        markdown.push_str(&format!(
            "{}. **{}** — _{}_ ({}, {})\n   - [ ] natural  [ ] não natural\n\n",
            position + 1,
            item.text,
            item.meta.family,
            item.meta.split,
            item.id
        ));
    }
    fs::write(directory.join("review-sample.md"), markdown)?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(0x9e3779b97f4a7c15))
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { usize::try_from(self.next() % n as u64).unwrap_or(0) }
    }
}

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, default_value = "crates/opencnpj-lab/synth/pt-BR/families.toml")]
    pub specs: PathBuf,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv")]
    pub catalog: PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: PathBuf,
    #[arg(long, default_value = "/data/opencnpj-data/intelligence/datasets")]
    pub out: PathBuf,
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    #[arg(long, default_value_t = 50)]
    pub per_family: usize,
    #[arg(long, default_value = "l3")]
    pub level: String,
    #[arg(long, default_value_t = 200)]
    pub review: usize,
}

pub fn run(args: Args) -> Result<()> {
    let text = fs::read_to_string(&args.specs).with_context(|| format!("read {}", args.specs.display()))?;
    let families = load_families(&text)?;
    let catalogues = Catalogues::load(&args.catalog, &args.sectors)?;
    let level = match args.level.as_str() {
        "l1" => Level::L1,
        "l2" => Level::L2,
        _ => Level::L3,
    };
    let config = Config { seed: args.seed, per_family: args.per_family, level };
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(|| "unknown".to_string(), |value| value.trim().to_string());
    let dataset = generate(&families, &catalogues, config, &commit)?;
    let directory = write_dataset(&dataset, &args.out)?;
    write_review_sample(&dataset, &directory, args.review, args.seed)?;
    println!(
        "dataset {}: {} items, {} dialogues; train {}, val {}, test {}",
        dataset.manifest.dataset_id,
        dataset.entries.len(),
        dataset.dialogues.len(),
        dataset.manifest.counts.by_split.get("train").copied().unwrap_or(0),
        dataset.manifest.counts.by_split.get("val").copied().unwrap_or(0),
        dataset.manifest.counts.by_split.get("test").copied().unwrap_or(0)
    );
    println!("written to {}", directory.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_small() -> Dataset {
        let families = load_families(SPEC).unwrap();
        let catalogues = Catalogues::fixture();
        generate(&families, &catalogues, Config { seed: 7, per_family: 2, level: Level::L3 }, "test").unwrap()
    }

    #[test]
    fn families_are_complete() {
        let families = load_families(SPEC).unwrap();
        assert_eq!(families.len(), 62);
        for family in families {
            assert!(family.templates.len() >= 8, "{}", family.id);
            assert!(family.authors.len() >= 2, "{}", family.id);
        }
    }

    #[test]
    fn determinism() {
        let first = generate_small();
        let second = generate_small();
        let directory = tempfile::tempdir().unwrap();
        let a = write_dataset(&first, directory.path()).unwrap();
        let b = write_dataset(&second, &directory.path().join("second")).unwrap();
        for name in ["train.jsonl", "val.jsonl", "test.jsonl", "dialogues.jsonl", "manifest.json"] {
            assert_eq!(fs::read(a.join(name)).unwrap(), fs::read(b.join(name)).unwrap(), "{name}");
        }
    }

    #[test]
    fn splits_do_not_leak() {
        let dataset = generate_small();
        let catalogues = Catalogues::fixture();
        let test_only: Vec<String> = catalogues
            .municipalities
            .iter()
            .filter(|row| row.test_only)
            .map(|row| row.name.to_lowercase())
            .chain(catalogues.sectors.iter().filter(|row| row.test_only).map(|row| row.singular.to_lowercase()))
            .collect();
        let mut forms: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for entry in &dataset.entries {
            let split = entry.item.meta.split.as_str();
            forms.entry(entry.form.clone()).or_default().insert(split.into());
            if split != "test" {
                let text = entry.item.text.to_lowercase();
                for name in &test_only {
                    assert!(!text.contains(name), "{} leaks test-only {name}: {}", entry.item.id, entry.item.text);
                }
                for noise in &entry.item.meta.noise {
                    assert_ne!(noise, "phonetic");
                    assert_ne!(noise, "merged");
                }
            }
        }
        for (form, splits) in forms {
            assert!(!(splits.contains("train") && (splits.contains("val") || splits.contains("test"))), "cross-split form {form}");
        }
    }

    #[test]
    fn every_gold_ir_validates() {
        let dataset = generate_small();
        let catalogues = Catalogues::fixture();
        let mut count = 0;
        for entry in &dataset.entries {
            if let Some(ir) = &entry.item.gold.ir {
                catalogues.canonical(ir.clone()).unwrap();
                count += 1;
            }
        }
        for dialogue in &dataset.dialogues {
            for turn in &dialogue.turns {
                if let Some(ir) = &turn.gold.ir {
                    catalogues.canonical(ir.clone()).unwrap();
                    count += 1;
                }
            }
        }
        assert!(count > 0);
    }

    #[test]
    fn manifest_reports_diversity() {
        let dataset = generate_small();
        assert!(!dataset.manifest.diversity.distinct_forms.is_empty());
        assert!(!dataset.manifest.diversity.templates_per_family.is_empty());
        assert!(dataset.manifest.diversity.slot_coverage["municipalities"] > 0.0);
        assert!(dataset.manifest.diversity.token_entropy > 0.0);
        assert!(dataset.manifest.diversity.char_entropy > 0.0);
        assert!(!dataset.manifest.diversity.noise_share.is_empty());
    }
}
