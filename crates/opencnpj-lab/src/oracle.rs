//! Independent oracle helpers for `diff-exec`.
//!
//! Nothing here calls the executor's place resolution, population sums, CNAE section table
//! or ranking code. When both sides of a differential share a function, a bug in it shows
//! up on both sides and the comparison passes; these helpers re-derive the same answers
//! from the catalogue and the artifacts' raw sections instead.
use std::collections::BTreeSet;

use anyhow::{Context, Result};
use opencnpj_insights::{
    agg::{Aggregate, Cell, Section, Table, UF_MISSING},
    execute::{AggregateSet, Row, SectorMap, Value},
};
use opencnpj_query::ir::*;

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
    use opencnpj::assistant::geo::macro_region;

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
