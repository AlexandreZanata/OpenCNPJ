//! Places of a request resolved against the municipality catalogue: members, vicinity and
//! population. Independent of the aggregates, so the engine executor and the oracle share it.

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use opencnpj_insights::{
    agg,
    catalog::municipalities::{Geography, MunicipalityCatalogue, Place},
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

pub fn geography(catalogue: &MunicipalityCatalogue, id: u16) -> Option<&Geography> {
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

pub fn near(catalogue: &MunicipalityCatalogue, center: u16, rule: NearRule) -> BTreeSet<u16> {
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

pub fn great_circle_km(a: &Geography, b: &Geography) -> f64 {
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
