//! Search parameters shared by the HTTP API and the CLI.
//!
//! * Text: `q` (aliases `nome`, `razao_social`, `nome_fantasia`), `field`, `fuzzy=auto|true|false`.
//! * Term filters: every `Field::FILTERS` name, comma-separated values OR-ed, repeated
//!   parameters AND-ed. Codes accept prefixes (`cnae=47`, `cep=01310*`) and CNAE section
//!   letters (`cnae=G`).
//! * Identity: `cnpj` (prefix of 1–14 characters), `cpf` (owner CPF in MEI/EI names or a
//!   masked partner CPF), `cpf_titular`, `socio_cnpj`, `socio_cpf`.
//! * Ranges on numeric columns: `<column>_de`, `<column>_ate` (dates `YYYY-MM-DD` or
//!   `YYYYMMDD`, capital in reais), `<column>_vazio=true` for documents without a value.
//! * Composites: `mei_ativo`, `simples_ativo`, `tem_email`, `tem_telefone`.
//! * `ordenar=<column>[:asc|:desc]`, `limit`, `offset`, `cursor`.

use opencnpj_store::numeric::{self, NumField, Range};
use opencnpj_store::postings::Field;
use opencnpj_store::search::{Query, RangeFilter, TextField};

/// Caps applied while parsing (`usize::MAX` = none).
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub max_limit: usize,
    pub max_offset: usize,
}

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "sim" | "s" | "yes")
}

fn alias(k: &str) -> &str {
    match k {
        "cnae_principal" | "cnae_fiscal_principal" => "cnae",
        "cnae_fiscal_secundaria" => "cnae_secundaria",
        "cnae_any" | "cnae_todos" => "cnae_qualquer",
        "situacao_cadastral" => "situacao",
        "porte_empresa" => "porte",
        "natureza_juridica" => "natureza",
        "opcao_simples" | "opcao_pelo_simples" => "simples",
        "opcao_mei" => "mei",
        "matriz" | "identificador_matriz_filial" => "matriz_filial",
        "motivo_situacao_cadastral" => "motivo",
        "socio_cnpj" | "socio_cpf" | "socio" => "socio_documento",
        other => other,
    }
}

/// `YYYY-MM-DD`, `YYYYMMDD` or a bare year (`2020`, expanded to Jan 1 / Dec 31).
fn date_bound(v: &str, upper: bool) -> Result<u64, String> {
    let t = v.trim();
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        let d = if upper { format!("{t}1231") } else { format!("{t}0101") };
        return numeric::date_value(&d).ok_or_else(|| format!("bad year {v}"));
    }
    numeric::date_value(t).ok_or_else(|| format!("bad date {v} (use YYYY-MM-DD)"))
}

fn bound(field: NumField, v: &str, upper: bool) -> Result<u64, String> {
    if field.is_date() { date_bound(v, upper) } else { numeric::money_value(v).ok_or_else(|| format!("bad amount {v}")) }
}

/// Parses `(key, value)` pairs (repeats allowed) into a query.
pub fn parse(pairs: &[(String, String)], caps: Caps) -> Result<Query, String> {
    let mut q = Query { limit: 20, fuzzy_auto: true, ..Default::default() };
    let mut lo: Vec<(NumField, u64)> = Vec::new();
    let mut hi: Vec<(NumField, u64)> = Vec::new();
    for (k, v) in pairs {
        let k = k.as_str();
        if v.len() > 2000 {
            return Err(format!("parameter {k} too long"));
        }
        match k {
            "q" | "nome" | "razao_social" | "nome_fantasia" => {
                q.text = Some(v.clone()).filter(|s| !s.trim().is_empty());
                q.text_field = match k {
                    "razao_social" => TextField::Razao,
                    "nome_fantasia" => TextField::Fantasia,
                    _ => q.text_field,
                };
            }
            "field" | "campo" => {
                q.text_field = match v.as_str() {
                    "any" | "todos" => TextField::Any,
                    "razao" | "razao_social" => TextField::Razao,
                    "fantasia" | "nome_fantasia" => TextField::Fantasia,
                    _ => return Err("field must be any, razao_social or nome_fantasia".into()),
                }
            }
            "fuzzy" => match v.to_ascii_lowercase().as_str() {
                "auto" => (q.fuzzy, q.fuzzy_auto) = (false, true),
                "true" | "1" | "sim" => (q.fuzzy, q.fuzzy_auto) = (true, false),
                "false" | "0" | "nao" | "não" => (q.fuzzy, q.fuzzy_auto) = (false, false),
                _ => return Err("fuzzy must be auto, true or false".into()),
            },
            "limit" => q.limit = v.parse().map_err(|_| "bad limit")?,
            "offset" => q.offset = v.parse().map_err(|_| "bad offset")?,
            "cursor" => q.after = Some(v.parse().map_err(|_| "bad cursor")?),
            "cnpj" | "cnpj_prefixo" | "cnpj_basico" => {
                q.cnpj_prefixes.extend(v.split(',').filter(|s| !s.trim().is_empty()).map(str::to_string))
            }
            "ordenar" | "sort" => {
                let (f, dir) = v.split_once(':').unwrap_or((v, "asc"));
                let f = NumField::from_name(f).ok_or_else(|| format!("cannot sort by {f}"))?;
                let desc = match dir {
                    "asc" | "crescente" => false,
                    "desc" | "decrescente" => true,
                    d => return Err(format!("sort direction must be asc or desc, not {d}")),
                };
                q.sort = Some((f, desc));
            }
            "mei_ativo" | "simples_ativo" => {
                if truthy(v) {
                    let (flag, excl) =
                        if k == "mei_ativo" { (Field::Mei, NumField::ExclusaoMei) } else { (Field::Simples, NumField::ExclusaoSimples) };
                    q.filters.push((flag, vec!["S".into()]));
                    q.ranges.push(RangeFilter { field: excl, range: None });
                }
            }
            "tem_email" | "tem_telefone" => {
                if truthy(v) {
                    q.filters.push((Field::Contato, vec![if k == "tem_email" { "email" } else { "telefone" }.into()]));
                }
            }
            "format" | "group_by" | "agrupar" | "estab_offset" | "estab_limit" | "max" => {}
            _ => {
                if let Some((f, suffix)) = NumField::ALL.iter().find_map(|f| {
                    ["_de", "_min", "_ate", "_max", "_vazio"].iter().find_map(|s| (k.strip_suffix(s) == Some(f.name())).then_some((*f, *s)))
                }) {
                    match suffix {
                        "_de" | "_min" => lo.push((f, bound(f, v, false)?)),
                        "_ate" | "_max" => hi.push((f, bound(f, v, true)?)),
                        _ => {
                            if truthy(v) {
                                q.ranges.push(RangeFilter { field: f, range: None });
                            }
                        }
                    }
                    continue;
                }
                let a = alias(k);
                let f = Field::from_name(a).filter(|f| Field::FILTERS.contains(f)).ok_or_else(|| format!("unknown parameter {k}"))?;
                let values: Vec<String> = v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
                if values.is_empty() {
                    return Err(format!("empty value for {k}"));
                }
                q.filters.push((f, values));
            }
        }
    }
    let mut fields: Vec<NumField> = lo.iter().chain(&hi).map(|x| x.0).collect();
    fields.dedup();
    for f in NumField::ALL.into_iter().filter(|f| fields.contains(f)) {
        let l = lo.iter().filter(|x| x.0 == f).map(|x| x.1).max().unwrap_or(1);
        let h = hi.iter().filter(|x| x.0 == f).map(|x| x.1).min().unwrap_or(u64::MAX);
        q.ranges.push(RangeFilter { field: f, range: Some(Range { lo: l, hi: h }) });
    }
    if q.limit == 0 || q.limit > caps.max_limit {
        return Err(format!("limit must be between 1 and {}", caps.max_limit));
    }
    if q.offset > caps.max_offset {
        return Err(format!("offset above {}", caps.max_offset));
    }
    Ok(q)
}

pub fn has_criteria(q: &Query) -> bool {
    q.text.is_some() || !q.filters.is_empty() || !q.cnpj_prefixes.is_empty() || !q.ranges.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pairs: &[(&str, &str)]) -> Result<Query, String> {
        let v: Vec<(String, String)> = pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        parse(&v, Caps { max_limit: 100, max_offset: usize::MAX })
    }

    #[test]
    fn filters_ranges_and_composites() {
        let q = p(&[
            ("q", "padaria"),
            ("cnae", "47,G"),
            ("uf", "SP"),
            ("data_inicio_atividade_de", "2020-01-01"),
            ("data_inicio_atividade_ate", "2020"),
            ("capital_social_min", "1000,00"),
            ("mei_ativo", "true"),
            ("socio_cpf", "12345678901"),
            ("ordenar", "data_inicio_atividade:desc"),
            ("fuzzy", "true"),
        ])
        .unwrap();
        assert_eq!(q.text.as_deref(), Some("padaria"));
        assert!(q.fuzzy && !q.fuzzy_auto);
        assert!(q.filters.contains(&(Field::Cnae, vec!["47".into(), "G".into()])));
        assert!(q.filters.contains(&(Field::Mei, vec!["S".into()])));
        assert!(q.filters.contains(&(Field::SocioDoc, vec!["12345678901".into()])));
        assert_eq!(q.sort, Some((NumField::InicioAtividade, true)));
        let inicio = q.ranges.iter().find(|r| r.field == NumField::InicioAtividade).unwrap();
        let r = inicio.range.unwrap();
        assert_eq!((numeric::date_string(r.lo), numeric::date_string(r.hi)), ("2020-01-01".into(), "2020-12-31".into()));
        assert!(q.ranges.contains(&RangeFilter { field: NumField::ExclusaoMei, range: None }));
        assert_eq!(q.ranges.iter().find(|r| r.field == NumField::CapitalSocial).unwrap().range.unwrap().lo, 100_001);
    }

    #[test]
    fn errors() {
        assert!(p(&[("nope", "1")]).is_err());
        assert!(p(&[("data_inicio_atividade_de", "2020-13-01")]).is_err());
        assert!(p(&[("limit", "101")]).is_err());
        assert!(p(&[("ordenar", "uf")]).is_err());
    }
}
