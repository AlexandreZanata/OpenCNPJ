//! Deterministic Portuguese interpretation for admitted indicator operations.

use std::fmt;

use opencnpj_insights::{
    catalog::municipalities::{MunicipalityCatalogue, Place, fold_name},
    official::{
        CategoryFilter, Request,
        query::uf_code,
        registry::{Category, Registry, Variable},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskError {
    Unsupported(String),
    Clarify(String),
    Unavailable(String),
}

impl fmt::Display for AskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(message) | Self::Clarify(message) | Self::Unavailable(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AskError {}

/// Interpret a Portuguese question into an admitted typed request.
pub fn parse(text: &str, catalogue: &MunicipalityCatalogue, registry: &Registry) -> Result<Request, AskError> {
    let normalized = normalize(text);
    if normalized.is_empty() {
        return Err(AskError::Clarify("A pergunta está vazia.".into()));
    }
    let uf = detect_uf(&normalized);
    let geographies = detect_municipalities(&normalized, catalogue, uf);
    let category = detect_category(&normalized, registry);
    let variable = detect_variable(&normalized, registry, category);
    let periods = detect_periods(&normalized);

    if normalized.contains("pib per capita") || normalized.contains("pib por habitante") {
        let denominator = geographies_or_state(&geographies, uf);
        if denominator.is_empty() {
            return Err(AskError::Clarify("Informe um município ou MT para calcular o PIB per capita.".into()));
        }
        return Ok(Request::Ratio {
            numerator: "pib_municipal".into(),
            denominator: "populacao_estimada".into(),
            geographies: denominator,
            period: periods.first().cloned(),
            category: None,
            scale: 1.0,
        });
    }
    if normalized.contains("bairro") || normalized.contains("setor censitario") || normalized.contains("distrito") {
        return Err(AskError::Unavailable(
            "Indicadores por bairro ou setor censitário não foram admitidos nas fontes municipais desta fatia. Não substituo silenciosamente pelo município."
                .into(),
        ));
    }
    if normalized.contains("renda domiciliar") || normalized.contains("renda media") || normalized.contains("renda per capita") {
        return Err(AskError::Unavailable(
            "Renda domiciliar municipal do Censo 2022 não foi admitida nesta fonte; não substituo por PIB, salário ou média de outra unidade."
                .into(),
        ));
    }
    if normalized.contains("cnpj")
        && (normalized.contains("funcionario") || normalized.contains("empregado") || normalized.contains("pessoal"))
    {
        return Err(AskError::Unavailable(
            "Não há contagem de funcionários por CNPJ nas fontes admitidas. Estatísticas municipais de emprego não descrevem um CNPJ individual."
                .into(),
        ));
    }
    if normalized.contains("estabelecimento")
        && (normalized.contains("por habitante") || normalized.contains("por mil") || normalized.contains("10 mil"))
    {
        return Err(AskError::Unavailable(
            "A razão entre estabelecimentos CNPJ e população ainda não foi admitida como operação oficial. Não invento o numerador.".into(),
        ));
    }

    let variable = variable.ok_or_else(|| {
        let names = registry.variables().map(|variable| variable.label.as_str()).collect::<Vec<_>>().join("; ");
        AskError::Unsupported(format!("Não reconheci um indicador admitido. Indicadores disponíveis: {names}."))
    })?;

    if variable.id.starts_with("pam_") && category.is_none() {
        return Err(AskError::Clarify("Informe o produto agrícola (por exemplo, soja, milho, algodão ou café).".into()));
    }
    if variable.id.starts_with("ppm_") && category.is_none() {
        return Err(AskError::Clarify("Informe o tipo de rebanho (por exemplo, bovino, suíno ou ovino).".into()));
    }

    let operation = operation(&normalized);
    let category_filter = category.map(|category| CategoryFilter::new(&category.classification_id, &category.category_id));
    match operation {
        Operation::Lookup => {
            let geographies = geographies_or_state(&geographies, uf);
            Ok(Request::Lookup { variable: variable.id.clone(), geographies, periods, category: category_filter })
        }
        Operation::Compare => {
            if geographies.len() < 2 {
                return Err(AskError::Clarify("Informe ao menos dois municípios para comparar.".into()));
            }
            Ok(Request::Compare { variable: variable.id.clone(), geographies, period: periods.first().cloned(), category: category_filter })
        }
        Operation::Rank => {
            let level = "N6".to_string();
            Ok(Request::Rank {
                variable: variable.id.clone(),
                period: periods.first().cloned(),
                geography_level: level,
                uf_code: uf.map(|value| value.to_string()).or_else(|| Some("51".into())),
                category: category_filter,
                limit: limit(&normalized),
                descending: !(normalized.contains("menor") || normalized.contains("menores") || normalized.contains("crescente")),
            })
        }
        Operation::Trend => {
            if geographies.len() != 1 {
                return Err(AskError::Clarify("Informe exatamente um município para a série temporal.".into()));
            }
            Ok(Request::Trend { variable: variable.id.clone(), geography: geographies[0].clone(), periods, category: category_filter })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    Lookup,
    Compare,
    Rank,
    Trend,
}

fn operation(text: &str) -> Operation {
    if contains_any(text, &["ranking", "rank", "maiores", "menores", "top "]) {
        Operation::Rank
    } else if contains_any(text, &["evolucao", "tendencia", "serie", "historico", "ao longo"]) {
        Operation::Trend
    } else if contains_any(text, &["comparar", "comparacao", "compare", "versus", " vs ", "diferenca"]) {
        Operation::Compare
    } else {
        Operation::Lookup
    }
}

fn normalize(text: &str) -> String {
    fold_name(text)
        .chars()
        .map(|character| if character.is_alphanumeric() || character == '-' || character == '.' { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    let phrase = normalize(phrase);
    if phrase.is_empty() {
        return false;
    }
    format!(" {text} ").contains(&format!(" {phrase} "))
}

fn contains_any(text: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| contains_phrase(text, phrase))
}

fn detect_uf(text: &str) -> Option<&'static str> {
    let candidates = [
        ("mato grosso", "MT"),
        ("mato-grosso", "MT"),
        ("mt", "MT"),
        ("sao paulo", "SP"),
        ("rio de janeiro", "RJ"),
        ("minas gerais", "MG"),
        ("parana", "PR"),
        ("goias", "GO"),
        ("mato grosso do sul", "MS"),
    ];
    candidates.iter().find(|(phrase, _)| contains_phrase(text, phrase)).and_then(|(_, uf)| uf_code(uf))
}

fn detect_municipalities(text: &str, catalogue: &MunicipalityCatalogue, uf: Option<&str>) -> Vec<String> {
    let mut matches = Vec::new();
    for row in catalogue.rows() {
        let Place::Municipality(place) = &row.place else { continue };
        if let Some(uf) = uf
            && place.uf != uf_of_code(uf)
        {
            continue;
        }
        let mut aliases = vec![row.name.as_str()];
        aliases.extend(row.variants.iter().map(String::as_str));
        let mut best = None;
        for alias in aliases {
            if let Some(index) = phrase_index(text, alias) {
                best = Some(best.map_or(index, |current: usize| current.min(index)));
            }
        }
        if let Some(index) = best {
            matches.push((index, format!("{:07}", place.ibge)));
        }
    }
    matches.sort();
    matches.dedup_by(|left, right| left.1 == right.1);
    matches.into_iter().map(|(_, code)| code).collect()
}

fn uf_of_code(code: &str) -> &str {
    match code {
        "11" => "RO",
        "12" => "AC",
        "13" => "AM",
        "14" => "RR",
        "15" => "PA",
        "16" => "AP",
        "17" => "TO",
        "21" => "MA",
        "22" => "PI",
        "23" => "CE",
        "24" => "RN",
        "25" => "PB",
        "26" => "PE",
        "27" => "AL",
        "28" => "SE",
        "29" => "BA",
        "31" => "MG",
        "32" => "ES",
        "33" => "RJ",
        "35" => "SP",
        "41" => "PR",
        "42" => "SC",
        "43" => "RS",
        "50" => "MS",
        "51" => "MT",
        "52" => "GO",
        "53" => "DF",
        _ => "",
    }
}

fn phrase_index(text: &str, phrase: &str) -> Option<usize> {
    let phrase = normalize(phrase);
    if phrase.is_empty() {
        return None;
    }
    format!(" {text} ").find(&format!(" {phrase} ")).map(|index| index.saturating_sub(1))
}

fn detect_category<'a>(text: &str, registry: &'a Registry) -> Option<&'a Category> {
    let mut candidates = registry
        .categories()
        .filter_map(|category| {
            let mut phrases = vec![category.label.as_str()];
            phrases.extend(category.synonyms.iter().map(String::as_str));
            phrases
                .iter()
                .filter_map(|phrase| phrase_index(text, phrase).map(|index| (index, phrase.len(), category)))
                .max_by_key(|(_, length, _)| *length)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(index, _, _)| *index);
    candidates.first().map(|(_, _, category)| *category)
}

fn detect_variable<'a>(text: &str, registry: &'a Registry, category: Option<&Category>) -> Option<&'a Variable> {
    if let Some(category) = category {
        if category.dataset_id == "ibge-1612" || category.dataset_id == "ibge-1613" {
            let prefix = if category.dataset_id == "ibge-1613" { "cafe_" } else { "pam_" };
            let metric = if contains_any(text, &["area colhida"]) {
                "area_colhida"
            } else if contains_any(text, &["area plantada", "area destinada"]) {
                "area_plantada"
            } else if contains_any(text, &["rendimento", "produtividade"]) {
                "rendimento"
            } else if contains_any(text, &["valor da producao"]) {
                "valor_producao"
            } else {
                "producao"
            };
            let id = format!("{prefix}{metric}");
            if let Some(variable) = registry.variable(&id) {
                return Some(variable);
            }
        } else if category.dataset_id == "ibge-3939" {
            return registry.variable("ppm_efetivo");
        }
    }
    let mut candidates = Vec::new();
    for variable in registry.variables() {
        let mut phrases = vec![variable.label.as_str()];
        phrases.extend(variable.synonyms.iter().map(String::as_str));
        if let Some((index, length)) = phrases
            .iter()
            .filter_map(|phrase| phrase_index(text, phrase).map(|index| (index, normalize(phrase).len())))
            .max_by_key(|(_, length)| *length)
        {
            candidates.push((index, length, variable));
        }
    }
    candidates.sort_by_key(|(index, length, _)| (*index, *length));
    if let Some((_, _, variable)) = candidates.last() {
        return Some(*variable);
    }
    // A product or herd category alone selects the natural metric of its dataset.
    category.and_then(|category| {
        if category.dataset_id == "ibge-1612" || category.dataset_id == "ibge-1613" {
            registry.variable("pam_producao")
        } else if category.dataset_id == "ibge-3939" {
            registry.variable("ppm_efetivo")
        } else {
            None
        }
    })
}

fn detect_periods(text: &str) -> Vec<String> {
    let years = text
        .split(|character: char| !character.is_ascii_digit())
        .filter(|token| token.len() == 4)
        .filter_map(|token| token.parse::<u16>().ok())
        .filter(|year| (1900..=2100).contains(year))
        .collect::<Vec<_>>();
    if years.len() >= 2 && contains_any(text, &["entre", " de ", " a ", "ate"]) {
        let (from, to) = (years[0].min(years[1]), years[0].max(years[1]));
        return (from..=to).map(|year| year.to_string()).collect();
    }
    let mut unique = years.into_iter().map(|year| year.to_string()).collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    unique
}

fn limit(text: &str) -> usize {
    for token in text.split_whitespace() {
        if let Ok(value) = token.parse::<usize>()
            && (1..=100).contains(&value)
        {
            return value;
        }
    }
    10
}

fn geographies_or_state(geographies: &[String], uf: Option<&str>) -> Vec<String> {
    if !geographies.is_empty() { geographies.to_vec() } else { uf.map(str::to_string).into_iter().collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencnpj_insights::official::registry::Registry;
    use std::path::Path;

    fn setup() -> (MunicipalityCatalogue, Registry) {
        let catalogue = MunicipalityCatalogue::load(Path::new("../opencnpj-insights/catalog/municipalities.tsv")).unwrap();
        (catalogue, Registry::builtin().unwrap())
    }

    #[test]
    fn population_lookup_by_name_and_year() {
        let (catalogue, registry) = setup();
        let request = parse("Qual a população estimada de Sorriso em 2024?", &catalogue, &registry).unwrap();
        assert_eq!(
            request,
            Request::Lookup {
                variable: "populacao_estimada".into(),
                geographies: vec!["5107925".into()],
                periods: vec!["2024".into()],
                category: None,
            }
        );
    }

    #[test]
    fn compare_two_municipalities() {
        let (catalogue, registry) = setup();
        let request = parse("Compare Sorriso e Sinop na população de 2024", &catalogue, &registry).unwrap();
        assert!(matches!(request, Request::Compare { geographies, .. } if geographies.len() == 2));
    }

    #[test]
    fn ranking_in_mt_defaults_to_municipal_level() {
        let (catalogue, registry) = setup();
        let request = parse("Ranking dos municípios de MT por população em 2024", &catalogue, &registry).unwrap();
        assert!(matches!(request, Request::Rank { uf_code: Some(code), category: None, .. } if code == "51"));
    }

    #[test]
    fn trend_range_and_crop_category() {
        let (catalogue, registry) = setup();
        let request = parse("Evolução da produção de soja em Sorriso entre 2020 e 2023", &catalogue, &registry).unwrap();
        match request {
            Request::Trend { variable, periods, category, .. } => {
                assert_eq!(variable, "pam_producao");
                assert_eq!(periods, ["2020", "2021", "2022", "2023"]);
                assert_eq!(category.unwrap().category_id, "2713");
            }
            other => panic!("unexpected request {other:?}"),
        }
    }

    #[test]
    fn pib_per_capita_is_an_explicit_ratio() {
        let (catalogue, registry) = setup();
        let request = parse("PIB per capita de Sorriso em 2023", &catalogue, &registry).unwrap();
        assert!(
            matches!(request, Request::Ratio { ref numerator, ref denominator, .. } if numerator == "pib_municipal" && denominator == "populacao_estimada")
        );
    }

    #[test]
    fn neighbourhood_indicator_is_refused_not_municipally_substituted() {
        let (catalogue, registry) = setup();
        let error = parse("População do bairro Centro de Sorriso em 2024", &catalogue, &registry).unwrap_err();
        assert!(matches!(error, AskError::Unavailable(message) if message.contains("bairro")));
    }

    #[test]
    fn unavailable_company_headcount_is_disclosed() {
        let (catalogue, registry) = setup();
        let error = parse("Quantos funcionários tem este CNPJ em Sorriso?", &catalogue, &registry).unwrap_err();
        assert!(matches!(error, AskError::Unavailable(message) if message.contains("CNPJ")));
    }
}
