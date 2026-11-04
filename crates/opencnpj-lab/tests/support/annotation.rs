//! Review-time representation of the query IR in 04-QUERY-IR.md.
//!
//! Catalogue references use stable source keys until the dense catalogues and
//! production IR are available. This schema belongs to evaluation tooling.

use opencnpj_core::{Cnpj, cnpj::parse_basico};
use opencnpj_store::numeric::{date_string, date_value};
use serde::Deserialize;

pub const COMPETENCE: &str = "2026-10";
pub const ANCHOR: &str = "2026-10-11";

macro_rules! labels {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant),+];
        }
    };
}

labels!(Intent { Count, List, Lookup, Compare, Rank, Breakdown, Trend, Meta });
labels!(Act {
    NewQuery,
    ReplaceSlot,
    AddFilter,
    RemoveFilter,
    CompareWithFocus,
    ReferToResult,
    Correction,
    AnswerClarification,
    MetaQuestion,
    SmallTalk,
    OutOfDomain
});
labels!(Flow { Registrations, Closures, NetRegistrations, MeiOptIns, MeiExits });
labels!(MetricKind { Stock, Flow, ActiveChange, Per10k, InhabitantsPer, Share });

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    #[serde(rename = "SR-01")]
    Sr01,
    #[serde(rename = "SR-02")]
    Sr02,
    #[serde(rename = "SR-03")]
    Sr03,
    #[serde(rename = "SR-04")]
    Sr04,
    #[serde(rename = "SR-05")]
    Sr05,
    #[serde(rename = "SR-06")]
    Sr06,
    #[serde(rename = "SR-07")]
    Sr07,
    #[serde(rename = "SR-08")]
    Sr08,
    #[serde(rename = "SR-09")]
    Sr09,
    #[serde(rename = "SR-10")]
    Sr10,
    #[serde(rename = "SR-11")]
    Sr11,
    #[serde(rename = "SR-12")]
    Sr12,
    #[serde(rename = "SR-13")]
    Sr13,
    #[serde(rename = "SR-14")]
    Sr14,
}

impl Rule {
    pub const COUNT: usize = 14;
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub origin: Origin,
    pub gold: Gold,
    #[serde(default)]
    pub capabilities: Capabilities,
    pub context: Option<Context>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    AiDraft,
    HumanReviewed,
    Human,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capabilities {
    #[default]
    Current,
    History,
    Geography,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub previous: String,
    #[serde(default)]
    pub focus: Vec<String>,
    #[serde(default)]
    pub result: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gold {
    pub act: Act,
    pub intent: Option<Intent>,
    pub decision: Decision,
    pub ir: Option<Request>,
    #[serde(default)]
    pub options: Vec<Request>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Execute,
    ExecuteWithNote,
    Clarify,
    Reject,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub op: Op,
    unit: Unit,
    scope: Filters,
    time: TimeSpec,
    output: Output,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Count { metric: Metric },
    List { order: ListOrder, limit: u16 },
    Lookup { id: String },
    Compare { dim: Dimension, members: Vec<String>, metric: Metric },
    Rank { dim: Dimension, metric: Metric, dir: Direction, k: u8, min_base: u32 },
    Breakdown { dim: Dimension, metric: Metric },
    Trend { metric: Flow, grain: Grain },
    Meta { topic: Topic },
}

impl Op {
    pub fn intent(&self) -> Intent {
        match self {
            Self::Count { .. } => Intent::Count,
            Self::List { .. } => Intent::List,
            Self::Lookup { .. } => Intent::Lookup,
            Self::Compare { .. } => Intent::Compare,
            Self::Rank { .. } => Intent::Rank,
            Self::Breakdown { .. } => Intent::Breakdown,
            Self::Trend { .. } => Intent::Trend,
            Self::Meta { .. } => Intent::Meta,
        }
    }

    pub fn metric(&self) -> Option<(MetricKind, Option<&Flow>)> {
        match self {
            Self::Count { metric } | Self::Compare { metric, .. } | Self::Rank { metric, .. } | Self::Breakdown { metric, .. } => {
                Some(match metric {
                    Metric::Stock => (MetricKind::Stock, None),
                    Metric::Flow(flow) => (MetricKind::Flow, Some(flow)),
                    Metric::ActiveChange { .. } => (MetricKind::ActiveChange, None),
                    Metric::Per10k(_) => (MetricKind::Per10k, None),
                    Metric::InhabitantsPer(_) => (MetricKind::InhabitantsPer, None),
                    Metric::Share => (MetricKind::Share, None),
                })
            }
            Self::Trend { metric, .. } => Some((MetricKind::Flow, Some(metric))),
            _ => None,
        }
    }

    fn dimension(&self) -> Option<&Dimension> {
        match self {
            Self::Compare { dim, .. } | Self::Rank { dim, .. } | Self::Breakdown { dim, .. } => Some(dim),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Stock,
    Flow(Flow),
    ActiveChange { pct: bool },
    Per10k(Population),
    InhabitantsPer(Population),
    Share,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Population {
    IbgeCensus2022,
    IbgeEstimate2025,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Establishments,
    HeadOffices,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListOrder {
    Cnpj,
    NewestFirst,
    OldestFirst,
    CapitalDesc,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    Month,
    Year,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    DataDate,
    Sources,
    Methodology,
    Help,
    Cnaes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Municipality,
    Uf,
    Region,
    Sector,
    CnaeSection,
    CnaeDivision,
    CnaeGroup,
    CnaeClass,
    CnaeSubclass,
    Porte,
    Status,
    HeadOffice,
    Mei,
    Year,
    Month,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Filters {
    #[serde(default)]
    place: Place,
    place_population: Option<Bounds<u32>>,
    #[serde(default)]
    activity: Activity,
    status: Vec<String>,
    #[serde(default)]
    porte: Vec<String>,
    mei: Option<bool>,
    simples: Option<bool>,
    head_office: Option<bool>,
    #[serde(default)]
    natureza: Vec<String>,
    opened: Option<Bounds<String>>,
    status_date: Option<Bounds<String>>,
    capital: Option<Bounds<u64>>,
    #[serde(default)]
    exclude: Exclusions,
    contact: Option<Contact>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Place {
    #[default]
    Any,
    Municipality(Vec<String>),
    Uf(Vec<String>),
    Region(Region),
    Near(Near),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Region {
    North,
    Northeast,
    CentralWest,
    Southeast,
    South,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Near {
    center: String,
    rule: NearRule,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NearRule {
    Neighbours(u8),
    WithinKm(u16),
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Activity {
    #[default]
    Any,
    Sector(Vec<String>),
    Cnae(Cnae),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cnae {
    codes: Vec<String>,
    scope: CnaeScope,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CnaeScope {
    Main,
    Any,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclusions {
    #[serde(default)]
    municipality: Vec<String>,
    #[serde(default)]
    uf: Vec<String>,
    #[serde(default)]
    sector: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Contact {
    Phone,
    Email,
    Both,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bounds<T> {
    from: Option<T>,
    to: Option<T>,
}

impl<T: PartialOrd> Bounds<T> {
    fn valid(&self, member: impl Fn(&T) -> bool) -> bool {
        (self.from.is_some() || self.to.is_some())
            && self.from.iter().chain(&self.to).all(member)
            && self.from.as_ref().zip(self.to.as_ref()).is_none_or(|(from, to)| from <= to)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeSpec {
    competence: String,
    anchor: String,
    period: Option<Period>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Period {
    Days { from: String, to: String },
    Competences { from: String, to: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    page: u16,
    with_methodology: bool,
}

const STATUS: &[&str] = &["01", "02", "03", "04", "08"];
const PORTE: &[&str] = &["00", "01", "03", "05"];
const UFS: &[&str] = &[
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "EX", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO",
    "RR", "RS", "SC", "SE", "SP", "TO",
];
const SECTORS: &[&str] = &["bakery@v1", "pharmacy@v1", "transport@v1"];

fn unique_sorted<T: Ord>(members: &[T]) -> bool {
    members.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_set(values: &[String], allowed: impl Fn(&str) -> bool) -> bool {
    unique_sorted(values) && values.iter().all(|value| allowed(value))
}

fn digits(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn municipality(value: &str) -> bool {
    digits(value, 4) && value != "0000"
}

fn uf(value: &str) -> bool {
    UFS.contains(&value)
}

fn sector(value: &str) -> bool {
    SECTORS.contains(&value)
}

fn cnae(code: &str) -> bool {
    matches!(code.as_bytes(), [b'A'..=b'U']) || [2, 3, 5, 7].into_iter().any(|length| digits(code, length))
}

fn day(value: &str) -> bool {
    date_value(value).is_some_and(|parsed| date_string(parsed) == value) && value <= ANCHOR
}

fn supported_members(dim: &Dimension, members: &[String]) -> bool {
    match dim {
        Dimension::Municipality => valid_set(members, municipality),
        Dimension::Uf => valid_set(members, uf),
        Dimension::Sector => valid_set(members, sector),
        Dimension::Region => valid_set(members, |value| matches!(value, "north" | "northeast" | "central_west" | "southeast" | "south")),
        _ => false,
    }
}

impl Request {
    pub fn validate(&self, capabilities: &Capabilities) -> Result<(), &'static str> {
        if self.time.competence != COMPETENCE || self.time.anchor != ANCHOR {
            return Err("request must pin the H-dev snapshot and anchor");
        }
        self.scope.validate(capabilities)?;
        if matches!(self.unit, Unit::HeadOffices) && self.scope.head_office == Some(false) {
            return Err("head offices and branch-only filter contradict one another");
        }
        let is_list = matches!(self.op, Op::List { .. });
        if self.output.page == 0 || !is_list && (self.output.page != 1 || self.scope.contact.is_some()) {
            return Err("pagination and contact filters are for lists only");
        }
        let _ = self.output.with_methodology;
        match &self.op {
            Op::List { limit, order } => {
                let _ = order;
                if !(1..=100).contains(limit) {
                    return Err("list limit is outside the annotation caller plan");
                }
            }
            Op::Lookup { id } => {
                let root = parse_basico(id).is_some_and(|raw| raw.as_slice() == id.as_bytes());
                let full = Cnpj::parse(id).is_ok_and(|cnpj| cnpj.as_str() == id);
                if !root && !full {
                    return Err("lookup requires a canonical CNPJ or root");
                }
            }
            Op::Compare { dim, members, .. } if !(2..=4).contains(&members.len()) || !supported_members(dim, members) => {
                return Err("comparison requires two to four unique members of the stated dimension");
            }
            Op::Rank { k, min_base, dir, .. } => {
                let _ = (dir, min_base);
                if *k == 0 {
                    return Err("ranking limit must be positive");
                }
            }
            Op::Meta { topic } => {
                let _ = topic;
            }
            _ => {}
        }
        self.validate_metric(capabilities)
    }

    fn validate_metric(&self, capabilities: &Capabilities) -> Result<(), &'static str> {
        let metric = match &self.op {
            Op::Count { metric } | Op::Compare { metric, .. } | Op::Rank { metric, .. } | Op::Breakdown { metric, .. } => Some(metric),
            _ => None,
        };
        let population_dim = matches!(self.op.dimension(), Some(Dimension::Municipality | Dimension::Uf | Dimension::Region));
        let population_place = !matches!(self.scope.place, Place::Any);
        if let Some(Metric::Per10k(population) | Metric::InhabitantsPer(population)) = metric {
            let _ = population;
            if !population_dim && !population_place {
                return Err("population metric requires a geographic dimension or filter");
            }
        }
        if self.scope.place_population.is_some() && !population_dim && !population_place {
            return Err("population filter requires a geographic dimension or filter");
        }
        match (metric, &self.op, &self.time.period) {
            (Some(Metric::ActiveChange { pct }), _, Some(Period::Competences { from, to })) => {
                if !matches!(capabilities, Capabilities::History) || !valid_history(from, to) {
                    return Err("active change requires two available historical snapshots");
                }
                if *pct && !matches!(&self.op, Op::Rank { min_base, .. } if *min_base >= 30) {
                    return Err("percentage-change rankings require a minimum base of 30 in this fixture");
                }
            }
            (Some(Metric::Flow(flow)), _, Some(Period::Days { from, to })) => self.validate_flow(flow, from, to)?,
            (_, Op::Trend { metric, grain }, Some(Period::Days { from, to })) => {
                self.validate_flow(metric, from, to)?;
                let length = match grain {
                    Grain::Month => 7,
                    Grain::Year => 4,
                };
                if from[..length] == to[..length] {
                    return Err("a trend needs at least two grains");
                }
            }
            (Some(Metric::Flow(_) | Metric::ActiveChange { .. }), _, _) | (_, Op::Trend { .. }, _) => {
                return Err("metric requires an explicit period of the appropriate kind");
            }
            (_, _, Some(_)) => return Err("this operation has no flow or change period"),
            (_, _, None) => {}
        }
        Ok(())
    }

    fn validate_flow(&self, flow: &Flow, from: &str, to: &str) -> Result<(), &'static str> {
        if !day(from) || !day(to) || from > to {
            return Err("invalid flow period");
        }
        if matches!(flow, Flow::Closures) && self.scope.status.as_slice() != ["08"] {
            return Err("closures require baixa status 08");
        }
        Ok(())
    }
}

fn valid_history(from: &str, to: &str) -> bool {
    let months = ["2026-04", "2026-05", "2026-06", "2026-07", "2026-08", "2026-09", COMPETENCE];
    months.contains(&from) && months.contains(&to) && from < to
}

impl Filters {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), &'static str> {
        let place_ok = match &self.place {
            Place::Any => true,
            Place::Municipality(codes) => !codes.is_empty() && valid_set(codes, municipality),
            Place::Uf(codes) => !codes.is_empty() && valid_set(codes, uf),
            Place::Region(region) => {
                let _ = region;
                true
            }
            Place::Near(near) => {
                let positive = match near.rule {
                    NearRule::Neighbours(depth) => depth > 0,
                    NearRule::WithinKm(km) => km > 0,
                };
                matches!(capabilities, Capabilities::Geography) && municipality(&near.center) && positive
            }
        };
        let activity_ok = match &self.activity {
            Activity::Any => true,
            Activity::Sector(keys) => !keys.is_empty() && valid_set(keys, sector),
            Activity::Cnae(Cnae { codes, scope }) => {
                let _ = scope;
                !codes.is_empty() && valid_set(codes, cnae)
            }
        };
        if !place_ok || !activity_ok {
            return Err("invalid place or activity filter");
        }
        if self.status.is_empty()
            || !valid_set(&self.status, |status| STATUS.contains(&status))
            || !valid_set(&self.porte, |porte| PORTE.contains(&porte))
            || !valid_set(&self.natureza, |natureza| digits(natureza, 1) || digits(natureza, 4))
            || !valid_set(&self.exclude.municipality, municipality)
            || !valid_set(&self.exclude.uf, uf)
            || !valid_set(&self.exclude.sector, sector)
        {
            return Err("invalid categorical filter or exclusion set");
        }
        if self.mei == Some(true) && !self.porte.is_empty() && self.porte.iter().all(|porte| porte == "03" || porte == "05") {
            return Err("MEI contradicts the selected porte");
        }
        if self.opened.as_ref().is_some_and(|range| !range.valid(|value| day(value)))
            || self.status_date.as_ref().is_some_and(|range| !range.valid(|value| day(value)))
            || self.capital.as_ref().is_some_and(|range| !range.valid(|_| true))
            || self.place_population.as_ref().is_some_and(|range| !range.valid(|_| true))
        {
            return Err("invalid filter range");
        }
        let _ = self.simples;
        Ok(())
    }
}
