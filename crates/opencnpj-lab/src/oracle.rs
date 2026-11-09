//! Independent oracle helpers for `diff-exec`.
//!
//! Nothing here calls the executor's place resolution, population sums, CNAE section table
//! or ranking code. When both sides of a differential share a function, a bug in it shows
//! up on both sides and the comparison passes; these helpers re-derive the same answers
//! from the catalogue and the artifacts' raw sections instead.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use opencnpj_insights::{
    agg::{self, Aggregate, Cell, Section, Table, UF_MISSING},
    catalog::municipalities::{Geography, MunicipalityCatalogue, Place},
    execute::{AggregateSet, Row, SectorMap, Value},
};
use opencnpj_query::ir::*;

/// IBGE macro-regions, in the IR's id order (1 North … 5 Centre-West).
const MACRO_REGIONS: [(u16, &[&str]); 5] = [
    (1, &["AC", "AM", "AP", "PA", "RO", "RR", "TO"]),
    (2, &["AL", "BA", "CE", "MA", "PB", "PE", "PI", "RN", "SE"]),
    (3, &["ES", "MG", "RJ", "SP"]),
    (4, &["PR", "RS", "SC"]),
    (5, &["DF", "GO", "MS", "MT"]),
];

pub fn macro_region(uf: &str) -> Option<u16> {
    MACRO_REGIONS.iter().find(|(_, ufs)| ufs.contains(&uf)).map(|(id, _)| *id)
}

fn geography(catalogue: &MunicipalityCatalogue, id: u16) -> Option<&Geography> {
    match &catalogue.get(id)?.place {
        Place::Municipality(geo) => Some(geo),
        Place::Abroad => None,
    }
}

/// A place filter resolved to municipality ids and/or UF codes (`None` = unrestricted).
#[derive(Debug, Default)]
pub struct OraclePlace {
    pub municipalities: Option<BTreeSet<u16>>,
    pub ufs: Option<BTreeSet<String>>,
}

impl OraclePlace {
    pub fn admits(&self, id: u16, uf: &str) -> bool {
        self.municipalities.as_ref().is_none_or(|ids| ids.contains(&id)) && self.ufs.as_ref().is_none_or(|ufs| ufs.contains(uf))
    }

    /// The filter matches no municipality at all: the count is 0, never national.
    pub fn is_empty(&self) -> bool {
        self.municipalities.as_ref().is_some_and(BTreeSet::is_empty) || self.ufs.as_ref().is_some_and(BTreeSet::is_empty)
    }
}

pub fn place(catalogue: &MunicipalityCatalogue, filter: &PlaceFilter, within: &SmallSet<UfId>) -> Result<OraclePlace> {
    let code = |uf: UfId| agg::uf_code(uf).map(str::to_string).context("unknown UF id");
    let mut out = OraclePlace::default();
    match filter {
        PlaceFilter::Any => {}
        PlaceFilter::Municipalities(ids) => out.municipalities = Some(ids.iter().map(|id| id.0).collect()),
        PlaceFilter::Ufs(ids) => out.ufs = Some(ids.iter().map(|uf| code(*uf)).collect::<Result<_>>()?),
        PlaceFilter::Region(id) => match MACRO_REGIONS.iter().find(|(region, _)| *region == id.0) {
            Some((_, ufs)) => out.ufs = Some(ufs.iter().map(|uf| uf.to_string()).collect()),
            None => {
                out.municipalities = Some(
                    catalogue
                        .rows()
                        .iter()
                        .filter(|row| geography(catalogue, row.id).is_some_and(|geo| geo.intermediate_region == id.0))
                        .map(|row| row.id)
                        .collect(),
                )
            }
        },
        PlaceFilter::Near { center, rule } => out.municipalities = Some(near(catalogue, center.0, *rule)),
    }
    if !within.is_empty() {
        let allowed: BTreeSet<String> = within.iter().map(|uf| code(*uf)).collect::<Result<_>>()?;
        if let Some(ids) = &mut out.municipalities {
            ids.retain(|id| geography(catalogue, *id).is_some_and(|geo| allowed.contains(&geo.uf)));
        } else if let Some(ufs) = &mut out.ufs {
            ufs.retain(|uf| allowed.contains(uf));
        } else {
            out.ufs = Some(allowed);
        }
    }
    Ok(out)
}

fn near(catalogue: &MunicipalityCatalogue, center: u16, rule: NearRule) -> BTreeSet<u16> {
    let mut chosen = BTreeSet::from([center]);
    match rule {
        NearRule::Neighbours(depth) => {
            let mut ring = vec![center];
            for _ in 0..depth {
                let mut next = Vec::new();
                for id in ring {
                    for &other in geography(catalogue, id).map(|geo| geo.neighbours.as_slice()).unwrap_or_default() {
                        if chosen.insert(other) {
                            next.push(other);
                        }
                    }
                }
                ring = next;
            }
        }
        NearRule::WithinKm(km) => {
            let Some(origin) = geography(catalogue, center) else { return chosen };
            for row in catalogue.rows() {
                if let Some(geo) = geography(catalogue, row.id)
                    && great_circle_km(origin, geo) <= f64::from(km)
                {
                    chosen.insert(row.id);
                }
            }
        }
    }
    chosen
}

fn great_circle_km(a: &Geography, b: &Geography) -> f64 {
    let (phi1, phi2) = (a.latitude.to_radians(), b.latitude.to_radians());
    let half_dphi = (phi2 - phi1) / 2.0;
    let half_dlambda = (b.longitude - a.longitude).to_radians() / 2.0;
    let chord = half_dphi.sin().powi(2) + phi1.cos() * phi2.cos() * half_dlambda.sin().powi(2);
    2.0 * 6_371.0 * chord.sqrt().asin()
}

pub fn population_of(geo: &Geography, reference: PopulationRef) -> Option<u32> {
    match reference {
        PopulationRef::IbgeCensus(2022) => geo.census_2022,
        PopulationRef::IbgeEstimate(year) if year == geo.estimate_year => Some(geo.population_estimate),
        _ => None,
    }
}

/// Population of the municipalities a member covers, restricted by the request's place,
/// exclusions and population range exactly like the counts are.
pub fn population(
    catalogue: &MunicipalityCatalogue,
    request: &Request,
    member: Option<(Dim, u64)>,
    reference: PopulationRef,
) -> Result<u64> {
    let place = place(catalogue, &request.scope.place, &request.scope.within_ufs)?;
    let excluded_ids: BTreeSet<u16> = request.scope.exclude.municipalities.iter().map(|id| id.0).collect();
    let excluded_ufs: BTreeSet<&str> = request.scope.exclude.ufs.iter().filter_map(|uf| agg::uf_code(*uf)).collect();
    let range = request.scope.place_population.map(|range| (range.from.unwrap_or(0), range.to.unwrap_or(u32::MAX)));
    let mut total = 0_u64;
    for row in catalogue.rows() {
        let Place::Municipality(geo) = &row.place else { continue };
        if !place.admits(row.id, &geo.uf) || excluded_ids.contains(&row.id) || excluded_ufs.contains(geo.uf.as_str()) {
            continue;
        }
        let Some(people) = population_of(geo, reference) else { continue };
        if let Some((low, high)) = range
            && !(low..=high).contains(&people)
        {
            continue;
        }
        let in_member = match member {
            None => true,
            Some((Dim::Municipality, key)) => u64::from(row.id) == key,
            Some((Dim::Uf, key)) => agg::uf_id(&geo.uf).is_some_and(|uf| u64::from(uf.0) == key),
            Some((Dim::Region(RegionLevel::Major), key)) => macro_region(&geo.uf).is_some_and(|region| u64::from(region) == key),
            Some((Dim::Region(RegionLevel::Intermediate), key)) => u64::from(geo.intermediate_region) == key,
            Some((Dim::Region(RegionLevel::Immediate), key)) => u64::from(geo.immediate_region) == key,
            Some(_) => true,
        };
        if in_member {
            total += u64::from(people);
        }
    }
    Ok(total)
}

/// UF ids and main-CNAE subclasses of a request that has no other filter, i.e. one that
/// the artifact's marginal sections answer directly. `None` for any other request.
/// UF ids and main-CNAE subclasses (`None` = unrestricted) that marginals can answer.
type MarginalScope = (Option<BTreeSet<u8>>, Option<BTreeSet<u32>>);

fn marginal_scope(request: &Request) -> Option<MarginalScope> {
    let scope = &request.scope;
    let plain = request.unit == Unit::Establishments
        && scope.head_office == Tri::Any
        && scope.status == Some(StatusSet::only(Status::Active))
        && scope.porte == PorteSet::ALL
        && scope.mei == Tri::Any
        && scope.simples == Tri::Any
        && scope.natureza.is_empty()
        && scope.opened.is_none()
        && scope.status_date.is_none()
        && scope.capital.is_none()
        && scope.place_population.is_none()
        && scope.contact == ContactNeed::Any
        && scope.within_ufs.is_empty()
        && scope.exclude.municipalities.is_empty()
        && scope.exclude.ufs.is_empty()
        && scope.exclude.sectors.is_empty();
    if !plain {
        return None;
    }
    let ufs = match &scope.place {
        PlaceFilter::Any => None,
        PlaceFilter::Ufs(ids) => Some(ids.iter().map(|uf| uf.0).collect()),
        _ => return None,
    };
    let subclasses = |codes: &SmallSet<CnaeCode>| -> Option<BTreeSet<u32>> {
        codes
            .iter()
            .map(|code| match code {
                CnaeCode::Subclass(value) => Some(*value),
                _ => None,
            })
            .collect()
    };
    let cnaes = match &scope.activity {
        ActivityFilter::Any => None,
        ActivityFilter::Sectors { cnaes, scope: CnaeScope::Main, .. } | ActivityFilter::Cnae { codes: cnaes, scope: CnaeScope::Main } => {
            Some(subclasses(cnaes)?)
        }
        _ => return None,
    };
    Some((ufs, cnaes))
}

/// Active stock from the T1 marginal sections (no cell iteration, no executor filters).
fn marginal(aggregate: &Aggregate, ufs: Option<&BTreeSet<u8>>, cnaes: Option<&BTreeSet<u32>>) -> Result<u64> {
    let (section, keep): (Section, Box<dyn Fn(u64) -> bool>) = match (ufs, cnaes) {
        (None, None) => (Section::Uf(Table::T1), Box::new(|_| true)),
        (Some(ufs), None) => (Section::Uf(Table::T1), Box::new(move |key| u8::try_from(key).is_ok_and(|uf| ufs.contains(&uf)))),
        (None, Some(cnaes)) => (Section::Cnae(Table::T1), Box::new(move |key| u32::try_from(key).is_ok_and(|cnae| cnaes.contains(&cnae)))),
        (Some(ufs), Some(cnaes)) => (
            Section::UfCnae(Table::T1),
            Box::new(move |key| u8::try_from(key >> 24).is_ok_and(|uf| ufs.contains(&uf)) && cnaes.contains(&((key & 0xFF_FFFF) as u32))),
        ),
    };
    let mut total = 0;
    for cell in aggregate.cells(section) {
        let Cell { key, count } = cell?;
        if keep(key) {
            total += count;
        }
    }
    Ok(total)
}

fn intersect(a: Option<&BTreeSet<u32>>, b: &BTreeSet<u32>) -> BTreeSet<u32> {
    a.map_or_else(|| b.clone(), |a| a.intersection(b).copied().collect())
}

/// Answers stock and active-change requests of past competences from the artifacts' raw
/// marginal sections. `None` when the request shape is outside what marginals can answer.
pub fn marginal_value(request: &Request, aggregates: &AggregateSet, sectors: &SectorMap) -> Result<Option<Value>> {
    let Some((ufs, cnaes)) = marginal_scope(request) else { return Ok(None) };
    let artifact = |competence: Competence| aggregates.get(competence).with_context(|| format!("no aggregates for {competence}"));
    let competences = match (request.op.metric(), request.time.period) {
        (Some(Metric::Stock), _) => (None, request.time.competence),
        (Some(Metric::ActiveChange { .. }), Some(Period::Competences { from, to })) => (Some(from), to),
        _ => return Ok(None),
    };
    // Value of one group (UF set, CNAE set) in the requested metric: (value, base).
    let measure = |ufs: Option<&BTreeSet<u8>>, cnaes: Option<&BTreeSet<u32>>| -> Result<(i64, u64)> {
        let after = marginal(artifact(competences.1)?, ufs, cnaes)?;
        Ok(match competences.0 {
            None => (i64::try_from(after)?, 0),
            Some(from) => {
                let before = marginal(artifact(from)?, ufs, cnaes)?;
                (i64::try_from(after)? - i64::try_from(before)?, before)
            }
        })
    };
    // A group is a row when it has stock at either end, as in the executor's group maps.
    let present = |value: i64, base: u64| -> Result<bool> { Ok(value + i64::try_from(base)? > 0 || base > 0) };
    match request.op {
        Op::Count { .. } => {
            let (value, _) = measure(ufs.as_ref(), cnaes.as_ref())?;
            Ok(Some(Value::Scalar { value }))
        }
        Op::Rank { dim: dim @ (Dim::Uf | Dim::Sector), metric, dir, k, min_base } => {
            let mut rows = Vec::new();
            match dim {
                Dim::Uf => {
                    let mut keys = BTreeSet::new();
                    for competence in [competences.0, Some(competences.1)].into_iter().flatten() {
                        for cell in artifact(competence)?.cells(Section::Uf(Table::T1)) {
                            let key = u8::try_from(cell?.key)?;
                            if key != UF_MISSING && ufs.as_ref().is_none_or(|ufs| ufs.contains(&key)) {
                                keys.insert(key);
                            }
                        }
                    }
                    for key in keys {
                        let (value, base) = measure(Some(&BTreeSet::from([key])), cnaes.as_ref())?;
                        if present(value, base)? {
                            rows.push(Row { key: u64::from(key), value, base });
                        }
                    }
                }
                _ => {
                    for (&id, codes) in sectors {
                        let codes = intersect(cnaes.as_ref(), codes);
                        let (value, base) = measure(ufs.as_ref(), Some(&codes))?;
                        if present(value, base)? {
                            rows.push(Row { key: u64::from(id), value, base });
                        }
                    }
                }
            }
            let percent = matches!(metric, Metric::ActiveChange { pct: true });
            if percent {
                rows.retain(|row| row.base >= u64::from(min_base));
            }
            rows.sort_by(|a, b| {
                let order = if percent {
                    (i128::from(a.value) * i128::from(b.base.max(1))).cmp(&(i128::from(b.value) * i128::from(a.base.max(1))))
                } else {
                    a.value.cmp(&b.value)
                };
                let order = if dir == Dir::Desc { order.reverse() } else { order };
                order.then_with(|| a.key.cmp(&b.key))
            });
            rows.truncate(usize::from(k));
            Ok(Some(Value::Rows(rows)))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::macro_region;

    #[test]
    fn macro_regions_cover_every_uf_once() {
        let ufs = [
            "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO",
            "RR", "RS", "SC", "SE", "SP", "TO",
        ];
        for uf in ufs {
            assert!(macro_region(uf).is_some(), "{uf}");
        }
        assert_eq!(macro_region("MT"), Some(5));
        assert_eq!(macro_region("BA"), Some(2));
        assert_eq!(macro_region("EX"), None);
    }
}
