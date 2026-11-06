use std::{collections::BTreeMap, fmt, ops::Deref};

use opencnpj_core::{Cnpj, cnpj::parse_basico};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smallvec::SmallVec;

macro_rules! ids {
    ($($name:ident : $repr:ty),+ $(,)?) => {$ (
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $repr);
    )+};
}

ids!(MunicipalityId: u16, UfId: u8, RegionId: u16, SectorId: u16, Competence: u16, Day: u32);

impl Competence {
    pub fn from_year_month(year: u16, month: u8) -> Option<Self> {
        let offset = year.checked_sub(2000)?.checked_mul(12)?;
        (1..=12).contains(&month).then_some(())?;
        Some(Self(offset.checked_add(u16::from(month - 1))?))
    }

    pub fn year(self) -> u16 {
        2000 + self.0 / 12
    }

    pub fn month(self) -> u8 {
        1 + (self.0 % 12) as u8
    }
}

impl fmt::Display for Competence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", 2000 + self.0 / 12, 1 + self.0 % 12)
    }
}

impl Day {
    /// The snapshot numeric columns use 1 for 1900-01-01 and 0 for missing.
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Option<Self> {
        let year = u32::from(year);
        if !(1900..=9999).contains(&year) || !(1..=12).contains(&month) || !(1..=month_days(year, month)).contains(&day) {
            return None;
        }
        let before_month: u32 = (1..month).map(|m| u32::from(month_days(year, m))).sum();
        Some(Self(year_start(year) + before_month + u32::from(day)))
    }
}

impl Day {
    /// Calendar components, or `None` for the missing-day sentinel and out-of-range values.
    pub fn to_ymd(self) -> Option<(u16, u8, u8)> {
        if self.0 == 0 || self.0 > year_start(10_000) {
            return None;
        }
        let mut year = 1900 + (self.0 - 1) / 365;
        while year_start(year) >= self.0 {
            year -= 1;
        }
        let mut day = self.0 - year_start(year);
        let mut month = 1;
        while day > u32::from(month_days(year, month)) {
            day -= u32::from(month_days(year, month));
            month += 1;
        }
        Some((year as u16, month, day as u8))
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_ymd() {
            Some((year, month, day)) => write!(f, "{year:04}-{month:02}-{day:02}"),
            None => write!(f, "day({})", self.0),
        }
    }
}

fn year_start(year: u32) -> u32 {
    let leaps = |n: u32| n / 4 - n / 100 + n / 400;
    (year - 1900) * 365 + leaps(year - 1) - leaps(1899)
}

fn month_days(year: u32, month: u8) -> u8 {
    match month {
        2 => 28 + u8::from(year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))),
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Sorted, unique set with space for eight inline members.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SmallSet<T>(SmallVec<[T; 8]>);

impl<T> Default for SmallSet<T> {
    fn default() -> Self {
        Self(SmallVec::new())
    }
}

impl<T: Ord> SmallSet<T> {
    pub fn new(values: impl IntoIterator<Item = T>) -> Self {
        values.into_iter().collect()
    }
}

impl<T: Ord> FromIterator<T> for SmallSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut values: SmallVec<[T; 8]> = iter.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        Self(values)
    }
}

impl<T> Deref for SmallSet<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Serialize> Serialize for SmallSet<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_slice().serialize(serializer)
    }
}

impl<'de, T: Ord + Deserialize<'de>> Deserialize<'de> for SmallSet<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::new(Vec::<T>::deserialize(deserializer)?))
    }
}

macro_rules! variants {
    ($($name:ident {$($variant:ident),+}),+ $(,)?) => {$ (
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    )+};
}

variants!(
    Unit { Establishments, HeadOffices },
    FlowMetric { Registrations, Closures, NetRegistrations, MeiOptIns, MeiExits },
    RegionLevel { Major, Intermediate, Immediate },
    CnaeLevel { Section, Division, Group, Class, Subclass },
    Dir { Asc, Desc },
    Grain { Month, Year },
    ListOrder { Cnpj, NewestFirst, OldestFirst, CapitalDesc },
    MetaTopic { DataDate, Sources, Methodology, Help, Cnaes },
    CnaeScope { Main, Any },
    Status { Null, Active, Suspended, Unfit, Closed },
    Porte { Unreported, Micro, Small, Other },
);

macro_rules! flags {
    ($name:ident, $flag:ident, [$($value:ident),+], $max:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(u8);

        impl $name {
            pub const ALL: Self = Self($max);

            pub const fn only(flag: $flag) -> Self {
                Self(1 << flag as u8)
            }

            pub fn new(values: impl IntoIterator<Item = $flag>) -> Self {
                values.into_iter().fold(Self(0), |set, flag| Self(set.0 | Self::only(flag).0))
            }

            pub const fn contains(self, flag: $flag) -> bool {
                self.0 & Self::only(flag).0 != 0
            }

            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }

            pub fn iter(self) -> impl Iterator<Item = $flag> {
                [$($flag::$value),+].into_iter().filter(move |&flag| self.contains(flag))
            }

            pub(crate) const fn bits(self) -> u8 {
                self.0
            }

            pub(crate) fn from_bits(bits: u8) -> Option<Self> {
                (bits <= $max).then_some(Self(bits))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_seq(self.iter())
            }
        }

        impl Default for $name {
            fn default() -> Self { Self::ALL }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self::new(Vec::<$flag>::deserialize(deserializer)?))
            }
        }
    };
}

flags!(StatusSet, Status, [Null, Active, Suspended, Unfit, Closed], 31);
flags!(PorteSet, Porte, [Unreported, Micro, Small, Other], 15);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tri {
    #[default]
    Any,
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactNeed {
    #[default]
    Any,
    Phone,
    Email,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CnaeCode {
    Section(u8),
    Division(u8),
    Group(u16),
    Class(u32),
    Subclass(u32),
}

impl fmt::Display for CnaeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Section(code) => write!(f, "{}", char::from(*code)),
            Self::Division(code) => write!(f, "{code:02}"),
            Self::Group(code) => write!(f, "{code:03}"),
            Self::Class(code) => write!(f, "{code:05}"),
            Self::Subclass(code) => write!(f, "{code:07}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CnpjRef(String);

impl CnpjRef {
    pub fn parse(value: &str) -> Result<Self, InvalidCnpj> {
        if let Ok(id) = Cnpj::parse(value) {
            Ok(Self(id.as_str().to_owned()))
        } else if let Some(id) = parse_basico(value) {
            Ok(Self(String::from_utf8(id.to_vec()).expect("ASCII root")))
        } else {
            Err(InvalidCnpj)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CnpjRef {
    type Error = InvalidCnpj;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid CNPJ or CNPJ root")]
pub struct InvalidCnpj;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopulationRef {
    IbgeCensus(u16),
    IbgeEstimate(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Stock,
    Flow(FlowMetric),
    ActiveChange { pct: bool },
    Per10k(PopulationRef),
    InhabitantsPer(PopulationRef),
    Share,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dim {
    Municipality,
    Uf,
    Region(RegionLevel),
    Sector,
    Cnae(CnaeLevel),
    Porte,
    Status,
    HeadOffice,
    Mei,
    Year,
    Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Member {
    Municipality(MunicipalityId),
    Uf(UfId),
    Region(RegionId),
    Sector(SectorId),
}

pub type Members = SmallSet<Member>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Op {
    Count { metric: Metric },
    List { order: ListOrder, limit: u16 },
    Lookup { id: CnpjRef },
    Compare { dim: Dim, members: Members, metric: Metric },
    Rank { dim: Dim, metric: Metric, dir: Dir, k: u8, min_base: u32 },
    Breakdown { dim: Dim, metric: Metric },
    Trend { metric: FlowMetric, grain: Grain },
    Meta { topic: MetaTopic },
}

impl Op {
    /// Metric used by this operation, with the implicit stock of `List`.
    pub fn metric(&self) -> Option<Metric> {
        match self {
            Self::Count { metric } | Self::Compare { metric, .. } | Self::Rank { metric, .. } | Self::Breakdown { metric, .. } => {
                Some(*metric)
            }
            Self::Trend { metric, .. } => Some(Metric::Flow(*metric)),
            Self::List { .. } => Some(Metric::Stock),
            Self::Lookup { .. } | Self::Meta { .. } => None,
        }
    }

    /// Dimension this operation aggregates over, when it has one.
    pub fn dimension(&self) -> Option<Dim> {
        match self {
            Self::Compare { dim, .. } | Self::Rank { dim, .. } | Self::Breakdown { dim, .. } => Some(*dim),
            _ => None,
        }
    }

    /// Status filter implied by the operation when the caller did not pin one (D-02).
    pub fn default_status(&self) -> StatusSet {
        match self.metric() {
            Some(Metric::Flow(FlowMetric::Closures)) => StatusSet::only(Status::Closed),
            Some(Metric::Flow(_)) => StatusSet::ALL,
            Some(_) => StatusSet::only(Status::Active),
            None => StatusSet::ALL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range<T> {
    pub from: Option<T>,
    pub to: Option<T>,
}

pub type DayRange = Range<Day>;
pub type PopRange = Range<u32>;
pub type MoneyRange = Range<u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NaturezaCode {
    Group(u8),
    Code(u16),
}

pub type NaturezaSet = SmallSet<NaturezaCode>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NearRule {
    Neighbours(u8),
    WithinKm(u16),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceFilter {
    #[default]
    Any,
    Municipalities(SmallSet<MunicipalityId>),
    Ufs(SmallSet<UfId>),
    Region(RegionId),
    Near {
        center: MunicipalityId,
        rule: NearRule,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityFilter {
    #[default]
    Any,
    Sectors {
        ids: SmallSet<SectorId>,
        cnaes: SmallSet<CnaeCode>,
        scope: CnaeScope,
    },
    Cnae {
        codes: SmallSet<CnaeCode>,
        scope: CnaeScope,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Exclusions {
    pub municipalities: SmallSet<MunicipalityId>,
    pub ufs: SmallSet<UfId>,
    pub sectors: SmallSet<SectorId>,
    /// Resolved main CNAEs for the excluded sectors.
    pub sector_cnaes: SmallSet<CnaeCode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Filters {
    pub place: PlaceFilter,
    pub within_ufs: SmallSet<UfId>,
    pub place_population: Option<PopRange>,
    pub population_ref: Option<PopulationRef>,
    pub activity: ActivityFilter,
    pub status: Option<StatusSet>,
    pub porte: PorteSet,
    pub mei: Tri,
    pub simples: Tri,
    pub head_office: Tri,
    pub natureza: NaturezaSet,
    pub opened: Option<DayRange>,
    pub status_date: Option<DayRange>,
    /// Capital bounds in integer cents.
    pub capital: Option<MoneyRange>,
    pub exclude: Exclusions,
    pub contact: ContactNeed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSpec {
    pub competence: Competence,
    pub period: Option<Period>,
    pub anchor: Day,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Days(DayRange),
    Competences { from: Competence, to: Competence },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Output {
    pub page: u16,
    pub with_methodology: bool,
    pub display_order: Vec<Member>,
}

impl Default for Output {
    fn default() -> Self {
        Self { page: 1, with_methodology: false, display_order: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CatalogueVersions {
    pub municipalities: u32,
    pub sectors: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub op: Op,
    pub unit: Unit,
    pub scope: Filters,
    pub time: TimeSpec,
    pub output: Output,
    pub versions: CatalogueVersions,
}

pub struct CanonicalContext<'a> {
    pub first_day: Day,
    pub last_day: Day,
    pub municipality_ufs: &'a BTreeMap<MunicipalityId, UfId>,
    pub sector_cnaes: &'a BTreeMap<SectorId, SmallSet<CnaeCode>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CanonicalError {
    #[error("unknown sector {0:?}")]
    UnknownSector(SectorId),
    #[error("unknown municipality {0:?}")]
    UnknownMunicipality(MunicipalityId),
    #[error("conflicting place and UF filters")]
    ConflictingPlace,
    #[error("date range has no overlap with the data")]
    DateRangeOutsideData,
}

impl Request {
    /// Canonicalize a copy of the request. The returned presentation order is
    /// kept separately from the canonical query and its cache key.
    pub fn canonicalize(&self, context: &CanonicalContext<'_>) -> Result<(Self, Vec<Member>), CanonicalError> {
        let mut request = self.clone();
        let scope = &mut request.scope;
        scope.status.get_or_insert_with(|| self.op.default_status());
        if let ActivityFilter::Sectors { ids, cnaes, .. } = &mut scope.activity {
            *cnaes = expand_sectors(ids, context)?;
        }
        scope.exclude.sector_cnaes = expand_sectors(&scope.exclude.sectors, context)?;
        canonical_place(scope, context)?;
        for range in [&mut scope.opened, &mut scope.status_date].into_iter().flatten() {
            *range = clip_days(*range, context)?;
        }
        if let Some(Period::Days(days)) = &mut request.time.period {
            *days = clip_days(*days, context)?;
        }
        let order = std::mem::take(&mut request.output.display_order);
        Ok((request, order))
    }
}

fn expand_sectors(ids: &SmallSet<SectorId>, context: &CanonicalContext<'_>) -> Result<SmallSet<CnaeCode>, CanonicalError> {
    let mut codes = Vec::new();
    for id in ids.iter() {
        codes.extend(context.sector_cnaes.get(id).ok_or(CanonicalError::UnknownSector(*id))?.iter().copied());
    }
    Ok(SmallSet::new(codes))
}

fn canonical_place(scope: &mut Filters, context: &CanonicalContext<'_>) -> Result<(), CanonicalError> {
    if scope.within_ufs.is_empty() {
        return Ok(());
    }
    match &mut scope.place {
        PlaceFilter::Any => scope.place = PlaceFilter::Ufs(scope.within_ufs.clone()),
        PlaceFilter::Ufs(ufs) => {
            let intersection = SmallSet::new(ufs.iter().copied().filter(|uf| scope.within_ufs.binary_search(uf).is_ok()));
            if intersection.is_empty() {
                return Err(CanonicalError::ConflictingPlace);
            }
            *ufs = intersection;
        }
        PlaceFilter::Municipalities(municipalities) => {
            for id in municipalities.iter() {
                let uf = context.municipality_ufs.get(id).ok_or(CanonicalError::UnknownMunicipality(*id))?;
                if scope.within_ufs.binary_search(uf).is_err() {
                    return Err(CanonicalError::ConflictingPlace);
                }
            }
        }
        PlaceFilter::Region(_) | PlaceFilter::Near { .. } => return Ok(()),
    }
    scope.within_ufs = SmallSet::default();
    Ok(())
}

fn clip_days(range: DayRange, context: &CanonicalContext<'_>) -> Result<DayRange, CanonicalError> {
    let from = range.from.unwrap_or(context.first_day).max(context.first_day);
    let to = range.to.unwrap_or(context.last_day).min(context.last_day);
    if from > to {
        return Err(CanonicalError::DateRangeOutsideData);
    }
    Ok(Range { from: Some(from), to: Some(to) })
}
