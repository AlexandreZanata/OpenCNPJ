use std::fmt::{self, Display, Formatter};

use crate::ir::*;

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let unit = match self.unit {
            Unit::Establishments => "establishments",
            Unit::HeadOffices => "head offices",
        };
        match &self.op {
            Op::Count { metric } => write!(f, "COUNT {unit} METRIC {metric}")?,
            Op::List { order, limit } => write!(f, "LIST {unit} ORDER {order:?} LIMIT {limit}")?,
            Op::Lookup { id } => write!(f, "LOOKUP {}", id.as_str())?,
            Op::Compare { dim, members, metric } => write!(f, "COMPARE {dim:?} {} METRIC {metric} UNIT {unit}", List(members))?,
            Op::Rank { dim, metric, dir, k, min_base } => {
                write!(f, "RANK {dim:?} METRIC {metric} DIR {dir:?} K {k} MIN_BASE {min_base} UNIT {unit}")?;
            }
            Op::Breakdown { dim, metric } => write!(f, "BREAKDOWN {dim:?} METRIC {metric} UNIT {unit}")?,
            Op::Trend { metric, grain } => write!(f, "TREND {metric:?} GRAIN {grain:?} UNIT {unit}")?,
            Op::Meta { topic } => write!(f, "META {topic:?}")?,
        }
        let scope = &self.scope;
        let mut filters = Vec::new();
        match &scope.place {
            PlaceFilter::Any => {}
            PlaceFilter::Municipalities(ids) => filters.push(format!("municipality={}", List(ids))),
            PlaceFilter::Ufs(ids) => filters.push(format!("uf={}", List(ids))),
            PlaceFilter::Region(id) => filters.push(format!("region={}", id.0)),
            PlaceFilter::Near { center, rule } => filters.push(format!("near={} {rule:?}", center.0)),
        }
        if !scope.within_ufs.is_empty() {
            filters.push(format!("within uf={}", List(&scope.within_ufs)));
        }
        if let Some(range) = scope.place_population {
            filters.push(format!("population={range}"));
        }
        if let Some(population) = scope.population_ref {
            filters.push(format!("population source={population}"));
        }
        match &scope.activity {
            ActivityFilter::Any => {}
            ActivityFilter::Cnae { codes, scope } => filters.push(format!("CNAE {scope:?}={}", List(codes))),
            ActivityFilter::Sectors { ids, cnaes, scope } => {
                filters.push(format!("sector={} CNAE {scope:?}={}", List(ids), List(cnaes)));
            }
        }
        if let Some(status) = scope.status {
            filters.push(format!("status={}", List(&status.iter().collect::<Vec<_>>())));
        }
        if scope.porte != PorteSet::ALL {
            filters.push(format!("porte={}", List(&scope.porte.iter().collect::<Vec<_>>())));
        }
        for (label, value) in [("MEI", scope.mei), ("Simples", scope.simples), ("head office", scope.head_office)] {
            if value != Tri::Any {
                filters.push(format!("{label}={value:?}"));
            }
        }
        if !scope.natureza.is_empty() {
            filters.push(format!("natureza={}", List(&scope.natureza)));
        }
        for (name, range) in [("opened", scope.opened), ("status date", scope.status_date)] {
            if let Some(range) = range {
                filters.push(format!("{name}={range}"));
            }
        }
        if let Some(capital) = scope.capital {
            filters.push(format!("capital cents={capital}"));
        }
        if let Some(name) = &scope.name_prefix {
            filters.push(format!("name prefix {:?}={}", name.field, name.prefix));
        }
        let exclude = &scope.exclude;
        if !exclude.municipalities.is_empty() {
            filters.push(format!("exclude municipality={}", List(&exclude.municipalities)));
        }
        if !exclude.ufs.is_empty() {
            filters.push(format!("exclude uf={}", List(&exclude.ufs)));
        }
        if !exclude.sectors.is_empty() {
            filters.push(format!("exclude sector={} CNAE={}", List(&exclude.sectors), List(&exclude.sector_cnaes)));
        }
        if scope.contact != ContactNeed::Any {
            filters.push(format!("contact={:?}", scope.contact));
        }
        if !filters.is_empty() {
            write!(f, "\n  WHERE {}", filters.join(" AND "))?;
        }
        write!(f, "\n  AT competence {} ANCHOR {}", self.time.competence, self.time.anchor)?;
        match self.time.period {
            Some(Period::Days(days)) => write!(f, " PERIOD {days}")?,
            Some(Period::Competences { from, to }) => write!(f, " PERIOD {from}..{to}")?,
            None => {}
        }
        if self.output.page != 1 {
            write!(f, " PAGE {}", self.output.page)?;
        }
        if self.output.with_methodology {
            write!(f, " WITH METHODOLOGY")?;
        }
        write!(f, "\n  CATALOGUES municipalities@{} sectors@{}", self.versions.municipalities, self.versions.sectors)
    }
}

impl Display for Metric {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stock => f.write_str("stock"),
            Self::Flow(flow) => write!(f, "{flow:?}"),
            Self::ActiveChange { pct: false } => f.write_str("active change"),
            Self::ActiveChange { pct: true } => f.write_str("active change (%)"),
            Self::Per10k(population) => write!(f, "stock per 10,000 inhabitants ({population})"),
            Self::InhabitantsPer(population) => write!(f, "inhabitants per establishment ({population})"),
            Self::Share => f.write_str("share of scope stock"),
        }
    }
}

impl Display for PopulationRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::IbgeCensus(year) => write!(f, "IBGE census {year}"),
            Self::IbgeEstimate(year) => write!(f, "IBGE estimate {year}"),
        }
    }
}

impl<T: Display> Display for Range<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(from) = &self.from {
            write!(f, "{from}")?;
        }
        f.write_str("..")?;
        if let Some(to) = &self.to {
            write!(f, "{to}")?;
        }
        Ok(())
    }
}

macro_rules! display_id {
    ($($id:ty),+) => {$ (
        impl Display for $id {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { Display::fmt(&self.0, f) }
        }
    )+};
}

display_id!(MunicipalityId, UfId, SectorId);

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Null => "01",
            Self::Active => "02",
            Self::Suspended => "03",
            Self::Unfit => "04",
            Self::Closed => "08",
        })
    }
}

impl Display for Porte {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unreported => "00",
            Self::Micro => "01",
            Self::Small => "03",
            Self::Other => "05",
        })
    }
}

impl Display for NaturezaCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Group(id) => write!(f, "{id}*"),
            Self::Code(id) => write!(f, "{id:04}"),
        }
    }
}

impl Display for Member {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Municipality(id) => write!(f, "municipality {}", id.0),
            Self::Uf(id) => write!(f, "uf {}", id.0),
            Self::Region(id) => write!(f, "region {}", id.0),
            Self::Sector(id) => write!(f, "sector {}", id.0),
        }
    }
}

struct List<'a, T>(&'a [T]);

impl<T: Display> Display for List<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (index, item) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            Display::fmt(item, f)?;
        }
        f.write_str("]")
    }
}
