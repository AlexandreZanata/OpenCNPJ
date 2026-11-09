//! Typed frame resolution from catalogue entities and declarative word-bounded cues.

use crate::{
    context::Conversation,
    lang::{LanguagePack, UFS},
    normalize,
    policy::{Decision, Interpretation},
    spotter::{Entity, Hit},
    time::{self, has},
};
use opencnpj_query::ir::*;
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Deserialize)]
pub(crate) struct Cues {
    pub list: Vec<String>,
    pub count: Vec<String>,
    pub compare: Vec<String>,
    pub rank: Vec<String>,
    pub trend: Vec<String>,
    pub breakdown: Vec<String>,
    pub registrations: Vec<String>,
    pub closures: Vec<String>,
    pub growth: Vec<String>,
}
#[derive(Deserialize)]
pub(crate) struct Policy {
    pub unavailable: Vec<String>,
    pub privacy: Vec<String>,
    pub ambiguous_activity: Vec<String>,
    pub ambiguous_quality: Vec<String>,
}

pub(crate) fn cue(text: &str, phrases: &[String]) -> bool {
    phrases.iter().any(|p| has(text, &[p]))
}
fn clarify(message: &str) -> Box<Interpretation> {
    Box::new(Interpretation::stop(Decision::Clarify, message))
}
fn reject(message: &str) -> Box<Interpretation> {
    Box::new(Interpretation::stop(Decision::Reject, message))
}

pub(crate) struct Frame {
    pub request: Request,
    pub act: String,
    pub notes: Vec<String>,
    /// Explicit row count of an exported list ("as 50 primeiras").
    pub rows: Option<u32>,
}

pub(crate) struct ResolveContext<'a> {
    pub pack: &'a LanguagePack,
    pub cues: &'a Cues,
    pub policy: &'a Policy,
    pub anchor: Day,
    pub current: Competence,
    pub competences: &'a [Competence],
    pub versions: CatalogueVersions,
    pub population: PopulationRef,
}

/// `export` marks a question whose answer is delivered as a file: a count of companies
/// becomes the list of those companies.
pub(crate) fn resolve(env: &ResolveContext<'_>, text: &str, state: &Conversation, export: bool) -> Result<Frame, Box<Interpretation>> {
    let ResolveContext { pack, cues, policy, anchor, current, competences, versions, population } = *env;
    let raw = normalize::fold(text);
    let words = normalize::words(text);
    let normalized = words.join(" ");
    let n = normalized.as_str();
    if cue(n, &policy.privacy) {
        return Err(reject(
            "A pesquisa de pessoas, CPFs e sócios não está disponível no chat. Posso pesquisar estabelecimentos por município e atividade.",
        ));
    }
    if cue(n, &policy.unavailable) {
        return Err(reject(
            "Essa informação não está nos dados cadastrais disponíveis. Posso contar estabelecimentos, listar CNPJs ou mostrar registros e baixas.",
        ));
    }
    let preliminary = pack.lexicon.scan(&words);
    let masked = masked_text(&words, &preliminary);
    let m = masked.as_str();
    let uncovered = |phrases: &[String]| {
        phrases.iter().any(|phrase| {
            let needle = normalize::words(phrase);
            !needle.is_empty()
                && words
                    .windows(needle.len())
                    .enumerate()
                    .any(|(at, w)| w == needle && !preliminary.iter().any(|h| h.start < at + needle.len() && at < h.end))
        })
    };
    if uncovered(&policy.ambiguous_quality) {
        return Err(clarify("Qual critério cadastral você quer usar? Informe porte, capital, período ou local de referência."));
    }
    if uncovered(&policy.ambiguous_activity) {
        return Err(clarify("Qual atividade específica você quer pesquisar? O termo precisa de uma definição de CNAE."));
    }
    let domain_evidence = preliminary.iter().any(|h| h.values.iter().any(|e| matches!(e, Entity::Sector(_))))
        || has(
            n,
            &[
                "empresa",
                "empresas",
                "estabelecimento",
                "estabelecimentos",
                "cnpj",
                "cnpjs",
                "cnae",
                "cnaes",
                "microempresas",
                "microempresa",
                "matriz",
                "matrizes",
                "filial",
                "filiais",
                "porte",
                "optantes",
                "mei",
                "simples",
                "capital social",
                "estoque",
                "densidade",
                "habitantes por estabelecimento",
            ],
        );
    if has(n, &["lojas", "loja"]) && !preliminary.iter().any(|h| h.values.iter().any(|e| matches!(e, Entity::Sector(_)))) {
        return Err(clarify("Que tipo de loja você quer pesquisar? Informe a atividade para definir os CNAEs."));
    }
    if !domain_evidence && state.last.is_none() && meta_topic(m).is_none() && lookup(&raw).is_none() {
        return Err(reject("Esse pedido não é uma pesquisa cadastral de empresas. Posso contar, listar e comparar estabelecimentos."));
    }
    let mut hits = preliminary;
    let has_lowercase = text.chars().any(char::is_lowercase);
    // A UF acronym must be uppercase in a mixed-case original, next to a place, or geographically introduced.
    // The token "para" is the preposition "para" unless introduced as the state of Pará ("no Pará", "do Pará", "estado do Pará").
    hits.retain(|h| {
        if h.end - h.start == 1
            && words[h.start] == "logradouro"
            && !has(n, &["cidade de logradouro", "municipio de logradouro", "logradouro pb"])
        {
            return false;
        }
        if h.end - h.start == 1
            && words[h.start] == "passos"
            && h.start > 0
            && (time::number(&words[h.start - 1]).is_some() || words[h.start - 1] == "ate")
        {
            return false;
        }
        !h.values.iter().all(|e| matches!(e, Entity::Uf(_))) || h.end - h.start > 1 || {
            let token = &words[h.start];
            (token != "para"
                && ((has_lowercase && text.split(|c: char| !c.is_alphanumeric()).any(|t| t == token.to_uppercase()))
                    || hits_adjacent_place(pack, &words, h.start)))
                || h.start > 0
                    && matches!(words[h.start - 1].as_str(), "em" | "no" | "na" | "de" | "do" | "da" | "estado" | "exceto" | "e")
                    && (token != "para"
                        || matches!(words[h.start - 1].as_str(), "no" | "do" | "estado")
                        || h.start >= 2 && matches!(words[h.start - 2].as_str(), "estado" | "estados")
                        || text.contains('á')
                        || text.contains('Á'))
        }
    });
    // Mask entity spans, retaining date punctuation. Short UF hits must not
    // erase matching substrings elsewhere ("es" once turned "meses" into "m  ").
    let temporal_raw = temporal_text(text, &hits);
    if time::invalid_dates(&temporal_raw) {
        return Err(clarify("Informe datas válidas no formato DD/MM/AAAA ou AAAA-MM-DD."));
    }
    for year in time::years(&temporal_raw) {
        if year > anchor.to_ymd().expect("valid anchor").0 && has(n, &[&format!("em {year}"), &format!("ano de {year}")]) {
            return Err(reject(
                "Não faço previsões nem disponho de snapshots futuros. Posso mostrar os registros e o estoque observados na base.",
            ));
        }
    }
    let mut notes = Vec::new();
    let mut rows = None;
    for i in 1..words.len() {
        if matches!(words[i - 1].as_str(), "em" | "no" | "na")
            && !hits.iter().any(|h| h.start <= i && i < h.end)
            // Recover only a complete single-token place followed by a UF. A
            // token inside a longer name ("Águas Lindaas de Goiás") must fail
            // closed instead of decomposing into unrelated municipalities.
            && words.get(i + 1).is_some_and(|s| UFS.iter().any(|u| u.eq_ignore_ascii_case(s)))
            && let Some(values) = pack.lexicon.fuzzy_place(&words[i])
        {
            hits.push(Hit { start: i, end: i + 1, values, fuzzy: true });
            notes.push(format!("Local interpretado por aproximação: {}.", words[i]));
        }
    }
    hits.sort_by_key(|h| h.start);
    // Fail closed on unknown places and on unsupported restrictions. A familiar
    // city elsewhere in the question must never hide an unrecognized filter.
    for (i, word) in words.iter().enumerate() {
        // Words inside a recognized entity must not act as filters or negations:
        // "Não-Me-Toque" contains "nao" but is a municipality name.
        if hits.iter().any(|h| h.start <= i && i < h.end) {
            continue;
        }
        if matches!(word.as_str(), "em" | "no" | "na") {
            let mut at = i + 1;
            while words.get(at).is_some_and(|w| {
                matches!(
                    w.as_str(),
                    "cidade"
                            | "municipio"
                            | "municipios"
                            | "cidades"
                            | "estado"
                            | "de"
                            | "do"
                            | "da"
                            // "no setor de TI": the activity after it must be recognized.
                            | "setor"
                            | "setores"
                            | "ramo"
                            | "segmento"
                )
            }) {
                at += 1;
            }
            if let Some(next) = words.get(at) {
                let recognized = hits.iter().any(|h| h.start <= at && at < h.end);
                let grammatical = next == "com"
                    || next == "menos"
                    || next == "mais"
                    || matches!(
                        next.as_str(),
                        "brasil"
                            | "ordem"
                            | "todas"
                            | "qualquer"
                            | "segunda"
                            | "terceira"
                            | "primeira"
                            | "primeiro"
                            | "segundo"
                            | "terceiro"
                            | "quarto"
                            | "trimestre"
                            | "semestre"
                            | "ano"
                            | "mes"
                            | "meses"
                            | "dias"
                            | "ate"
                            | "toneladas"
                            | "muxirum"
                            | "muchirum"
                            | "cadeia"
                            | "corredor"
                            | "eixo"
                            | "cruzamento"
                            | "praca"
                            | "retrato"
                            | "levantamento"
                            | "fotografia"
                            | "cnae"
                            | "cnaes"
                            | "pagina"
                            | "situacao"
                            | "cadastro"
                            | "base"
                            | "pesquisa"
                            | "grupo"
                            | "regiao"
                            | "competencia"
                            | "funcionamento"
                            | "tanto"
                            | "estimativa"
                            | "consulta"
                            | "subclasse"
                            | "classe"
                            | "divisao"
                            | "secao"
                            | "estabelecimentos"
                            | "estabelecimento"
                            | "habitantes"
                            | "verdade"
                    );
                let numeric = time::number(next).is_some();
                let month = [
                    "janeiro",
                    "fevereiro",
                    "marco",
                    "abril",
                    "maio",
                    "junho",
                    "julho",
                    "agosto",
                    "setembro",
                    "outubro",
                    "novembro",
                    "dezembro",
                ]
                .contains(&next.as_str());
                if !recognized && !grammatical && !numeric && !month {
                    return Err(clarify("Não reconheci o local ou a restrição. Informe o município e a UF, ou reformule esse filtro."));
                }
                // A multi-word place must be fully recognized up to its UF.
                // Otherwise a typo can decompose into unrelated municipalities
                // ("Porto SSeguro/BA" -> Porto/PI).
                if !month
                    && !grammatical
                    && !next.chars().all(|c| c.is_ascii_digit())
                    && let Some(uf_at) = (at..words.len().min(at + 8)).find(|&j| UFS.iter().any(|u| u.eq_ignore_ascii_case(&words[j])))
                {
                    let fully_recognized = (at..uf_at).all(|j| {
                        hits.iter().any(|h| h.start <= j && j < h.end)
                            || matches!(words[j].as_str(), "de" | "do" | "da" | "dos" | "das" | "e" | "em" | "no" | "na")
                    });
                    if !fully_recognized {
                        return Err(clarify("Não reconheci o local ou a restrição. Informe o município e a UF, ou reformule esse filtro."));
                    }
                }
            }
        }
        if matches!(word.as_str(), "quantas" | "quantos" | "conte")
            && let Some(next) = words.get(i + 1)
            && !hits.iter().any(|h| h.start <= i + 1 && i + 1 < h.end)
            && !matches!(
                next.as_str(),
                "quantos"
                    | "quantas"
                    | "empresas"
                    | "empresa"
                    | "estabelecimentos"
                    | "estabelecimento"
                    | "cnpj"
                    | "cnpjs"
                    | "os"
                    | "as"
                    | "o"
                    | "a"
                    | "um"
                    | "uma"
                    | "matriz"
                    | "filial"
                    | "matrizes"
                    | "filiais"
                    | "foram"
                    | "estavam"
                    | "abriram"
                    | "fecharam"
                    | "baixa"
                    | "baixas"
                    | "novas"
                    | "novos"
                    | "todos"
                    | "todas"
                    | "sao"
                    | "de"
                    | "microempresas"
                    | "microempresa"
                    | "somente"
                    | "apenas"
                    | "habitantes"
                    | "habitante"
                    | "tem"
                    | "ha"
                    | "existem"
            )
        {
            return Err(clarify("Qual atividade você quer pesquisar? Não reconheci esse setor no catálogo."));
        }
        // "empresas agricolas": a word describing the companies that names no known activity,
        // status or filter must not be dropped, or every company would be answered. A singular
        // "a empresa Zaflas" names one company and is left to the company search.
        let unknown_at = |at: usize| {
            words.get(at).is_some_and(|next| {
                !hits.iter().any(|h| h.start <= at && at < h.end)
                    && next.len() > 3
                    && time::number(next).is_none()
                    && !known_descriptor(next)
            })
        };
        if matches!(word.as_str(), "empresas" | "estabelecimentos" | "negocios" | "firmas" | "companhias") && unknown_at(i + 1) {
            return Err(clarify("Qual atividade você quer pesquisar? Não reconheci esse tipo de empresa no catálogo de setores."));
        }
        if matches!(word.as_str(), "sem" | "nao" | "exceto" | "excluindo" | "fora") {
            let end = (i + 7).min(words.len());
            let supported = has(
                &words[i..end].join(" "),
                &[
                    "mei",
                    "simples",
                    "filiais",
                    "filial",
                    "matrizes",
                    "matriz",
                    "informado",
                    "informada",
                    "restricao de data",
                    "nao estao ativas",
                    "nao estao ativos",
                    "nao tao ativas",
                    "nao tao ativos",
                    "duplicar",
                    "mudar atividade",
                    "separar",
                    "quis dizer",
                ],
            ) || hits.iter().any(|h| {
                h.start > i
                    && h.start < end
                    && words[i + 1..h.start].iter().all(|w| {
                        matches!(
                            w.as_str(),
                            "o" | "a"
                                | "os"
                                | "as"
                                | "no"
                                | "na"
                                | "nos"
                                | "nas"
                                | "de"
                                | "do"
                                | "da"
                                | "dos"
                                | "das"
                                | "e"
                                | "cidades"
                                | "municipios"
                                | "estados"
                                | "setores"
                                | "incluir"
                                | "contar"
                                | "cidade"
                                | "municipio"
                                | "estado"
                                | "setor"
                        )
                    })
            });
            if !supported {
                return Err(clarify("Não consegui aplicar essa exclusão a um filtro cadastral. Qual condição você quer excluir?"));
            }
        }
        if matches!(word.as_str(), "frota" | "bairro" | "cep" | "logradouro" | "certificacao" | "licenciamento") {
            return Err(clarify(
                "Esse filtro ainda não é interpretado com segurança. Use município, setor, situação, porte, MEI, Simples, natureza, datas ou capital.",
            ));
        }
    }
    let explicit_ufs: BTreeSet<_> =
        hits.iter().flat_map(|h| &h.values).filter_map(|e| if let Entity::Uf(id) = e { Some(*id) } else { None }).collect();
    let mut municipalities = Vec::new();
    let mut ufs = Vec::new();
    let mut regions = Vec::new();
    let mut sectors = Vec::new();
    let mut excluded_muni = Vec::new();
    let mut excluded_ufs = Vec::new();
    let mut excluded_sectors = Vec::new();
    let bare_entities = words.iter().enumerate().all(|(i, w)| {
        hits.iter().any(|h| h.start <= i && i < h.end)
            || matches!(
                w.as_str(),
                "de" | "do"
                    | "da"
                    | "em"
                    | "no"
                    | "na"
                    | "quantas"
                    | "quantos"
                    | "tem"
                    | "ha"
                    | "existem"
                    | "vs"
                    | "versus"
                    | "compara"
                    | "compare"
                    | "e"
                    | "ou"
            )
    });
    for hit in &hits {
        let before = words[..hit.start].iter().rev().take(6).rev().cloned().collect::<Vec<_>>().join(" ");
        let prefix = &words[..hit.start];
        let neg = prefix
            .iter()
            .enumerate()
            .rev()
            .find(|(i, w)| {
                matches!(w.as_str(), "exceto" | "excluindo" | "tirando" | "sem" | "menos" | "fora")
                    && !hits.iter().any(|h| h.start <= *i && *i < h.end)
            })
            .map(|(i, _)| i)
            .is_some_and(|at| {
                // Explicit exclusion markers coordinate several targets ("exceto A e
                // B"): entity words between the marker and this hit do not break the
                // scope. Other markers keep the strict function-word scope so that a
                // comparator such as "menos" never leaks across entities.
                let explicit = matches!(prefix[at].as_str(), "exceto" | "excluindo" | "tirando");
                prefix[at + 1..].iter().enumerate().all(|(k, w)| {
                    matches!(
                        w.as_str(),
                        "incluir"
                            | "contar"
                            | "o"
                            | "a"
                            | "os"
                            | "as"
                            | "no"
                            | "na"
                            | "nos"
                            | "nas"
                            | "cidade"
                            | "cidades"
                            | "municipio"
                            | "municipios"
                            | "estado"
                            | "estados"
                            | "setor"
                            | "e"
                            | "de"
                            | "do"
                            | "da"
                            | "dos"
                            | "das"
                    ) || (explicit && hits.iter().any(|h| h.start <= at + 1 + k && at + 1 + k < h.end))
                })
            });
        let city = has(&before, &["cidade", "municipio", "capital"])
            || words.get(hit.end).is_some_and(|s| s == "capital")
            || words.get(hit.end).is_some_and(|s| UFS.iter().any(|u| u.eq_ignore_ascii_case(s)));
        let state_name = hit.values.iter().any(|e| matches!(e, Entity::Uf(_)));
        let has_sector = hit.values.iter().any(|e| matches!(e, Entity::Sector(_)));
        let municipality_mentions = hits.iter().filter(|h| h.values.iter().any(|e| matches!(e, Entity::Municipality { .. }))).count();
        // A sector name that is also a municipality name ("serraria") is an
        // activity in "estabelecimentos de ...", even when an earlier word such
        // as the trend metric "menos baixas cadastrais" looks geographic.
        let activity_context = before.ends_with(" de")
            && has(
                &before,
                &[
                    "estabelecimentos",
                    "estabelecimento",
                    "empresas",
                    "empresa",
                    "lojas",
                    "loja",
                    "servicos",
                    "servico",
                    "atividades",
                    "atividade",
                    "setores",
                    "setor",
                    "exceto",
                    "excluindo",
                    "tirando",
                    "sem",
                    "menos",
                    "fora",
                ],
            );
        let geographic = has(&before, &["cidade", "municipio", "em", "no", "na", "sem", "exceto", "menos", "vs", "quis dizer"])
            && !(has_sector && activity_context && !city);
        if !geographic && !city && cue(&words[hit.start..hit.end].join(" "), &cues.count) {
            continue;
        }
        // The state right after a city ("Belém/PA", "Belém PA", "Belém do Pará") qualifies it.
        let local_uf = hits
            .iter()
            .find(|h| {
                h.start == hit.end || h.start == hit.end + 1 && words.get(hit.end).is_some_and(|w| matches!(w.as_str(), "do" | "da" | "de"))
            })
            .and_then(|h| h.values.iter().find_map(|e| if let Entity::Uf(id) = e { Some(*id) } else { None }));
        if !has_sector
            && !state_name
            && !geographic
            && local_uf.is_none()
            && hit.start > 0
            && !before.ends_with(" de")
            && !before.ends_with(" com")
            && !bare_entities
            && hit.values.iter().any(|e| matches!(e, Entity::Municipality { .. }))
        {
            continue;
        }
        let candidates: Vec<_> = hit
            .values
            .iter()
            .filter_map(|e| match e {
                Entity::Municipality { id, uf }
                    if (city
                        || !state_name
                        || state.last.as_ref().is_some_and(|r| matches!(r.scope.place, PlaceFilter::Municipalities(_)))
                            && n.starts_with("e "))
                        && (!has_sector || geographic)
                        && local_uf
                            .map_or(municipality_mentions > 1 || explicit_ufs.is_empty() || explicit_ufs.contains(uf), |id| id == *uf) =>
                {
                    Some((*id, *uf))
                }
                _ => None,
            })
            .collect();
        if candidates.len() > 1
            || candidates.is_empty() && !has_sector && hit.values.iter().any(|e| matches!(e, Entity::Municipality { .. })) && !state_name
        {
            return Err(clarify("Qual é a UF do município? Há mais de uma cidade com esse nome."));
        }
        if let Some((id, _)) = candidates.first() {
            if neg {
                excluded_muni.push(*id)
            } else {
                municipalities.push(*id)
            };
            continue;
        }
        let activities: Vec<_> = hit.values.iter().filter_map(|e| if let Entity::Sector(id) = e { Some(*id) } else { None }).collect();
        if activities.len() > 1 {
            return Err(clarify("Qual atividade você quer incluir? O termo corresponde a mais de um setor CNAE."));
        }
        if let Some(id) = activities.first() {
            if neg {
                excluded_sectors.push(*id)
            } else {
                sectors.push(*id)
            };
            continue;
        }
        for entity in &hit.values {
            match entity {
                Entity::Uf(id) => {
                    if hits
                        .iter()
                        .any(|h| h.end == hit.start && h.values.iter().any(|e| matches!(e, Entity::Municipality { uf, .. } if uf == id)))
                    {
                        continue;
                    }
                    if neg { excluded_ufs.push(*id) } else { ufs.push(*id) }
                }
                Entity::Region(id) => regions.push(*id),
                _ => {}
            }
        }
    }
    // The broader word "mercado" may be part of an exact, unambiguous sector phrase.
    if cue(m, &policy.ambiguous_activity) && sectors.is_empty() {
        return Err(clarify("Qual atividade específica você procura? Escolha um setor para definir os CNAEs da consulta."));
    }
    let meta = meta_topic(m);
    let bare_topic = !sectors.is_empty()
        && !municipalities.is_empty()
        && words
            .iter()
            .enumerate()
            .all(|(i, w)| hits.iter().any(|h| h.start <= i && i < h.end) || matches!(w.as_str(), "em" | "no" | "na" | "de" | "do" | "da"));
    let full_query = bare_topic
        || cue(m, &cues.count)
        || cue(m, &cues.list)
        || cue(m, &cues.compare)
        || cue(m, &cues.rank)
        || cue(m, &cues.trend)
        || cue(m, &cues.breakdown)
        || meta.is_some()
        || cue(m, &cues.growth)
        || lookup(&raw).is_some()
        || has(n, &["densidade", "habitantes por estabelecimento"])
        || export && (!hits.is_empty() || has(n, &["empresas", "empresa", "estabelecimentos", "estabelecimento", "cnpjs", "negocios"]));
    let followup = state.last.is_some()
        && !has(n, &["nova pesquisa", "nova pergunta", "novo assunto"])
        && (n.starts_with("e ")
            || has(
                n,
                &[
                    "agora",
                    "dessas",
                    "desses",
                    "delas",
                    "as duas",
                    "os dois",
                    "qual das duas",
                    "entre as duas",
                    "entre elas",
                    "na verdade",
                    "quis dizer",
                    "tira",
                    "tire",
                    "retire",
                    "retira",
                    "remova",
                    "pode tirar",
                    "volte a incluir",
                    "inclua filiais",
                    "limpe",
                    "desfaca",
                    "mantenha",
                    "fica so",
                    "quero somente",
                    "filtra",
                    "considere so",
                    "conte matrizes",
                    "conte matriz",
                    "conte filial",
                    "conte filiais",
                ],
            )
            || n.starts_with("nao ")
            || export && has(n, &["essas", "esses", "isso", "dessas", "desses", "mesmas", "mesmos"])
            || !full_query
            || municipalities.is_empty() && sectors.is_empty() && has(n, &["apenas", "somente", "so"]));
    if bare_topic {
        notes.push("Considerei o estoque de estabelecimentos ativos.".into());
    }
    let mut request = if followup {
        state.last.clone().expect("checked previous request")
    } else {
        Request {
            op: Op::Count { metric: Metric::Stock },
            unit: Unit::Establishments,
            scope: Filters::default(),
            time: TimeSpec { competence: current, period: None, anchor },
            output: Output::default(),
            versions,
        }
    };
    let mut act = if followup && (has(n, &["na verdade", "quis dizer"]) || n.starts_with("nao ")) {
        "correction"
    } else if followup && municipalities.is_empty() && sectors.is_empty() {
        "add_filter"
    } else if followup {
        "replace_slot"
    } else {
        "new_query"
    }
    .to_owned();
    if !full_query && !followup && meta.is_none() {
        return Err(reject(
            "Não identifiquei uma pesquisa cadastral. Tente contar empresas de um setor, listar CNPJs ou comparar municípios.",
        ));
    }
    if has(n, &["desse tipo", "deste tipo", "nao sejam desse", "nao sejam essas"]) {
        return Err(clarify("A qual filtro a exclusão se refere?"));
    }
    if !municipalities.is_empty() {
        request.scope.place = PlaceFilter::Municipalities(SmallSet::new(municipalities.iter().copied()));
        request.scope.within_ufs = SmallSet::default();
    } else if !ufs.is_empty() {
        request.scope.place = PlaceFilter::Ufs(SmallSet::new(ufs.iter().copied()));
        request.scope.within_ufs = SmallSet::default();
    } else if let Some(id) = regions.first() {
        request.scope.place = PlaceFilter::Region(*id);
    }
    if has(n, &["dessas cidades", "desses municipios"]) {
        let ids: Vec<_> =
            state.result_members.iter().filter_map(|m| if let Member::Municipality(id) = m { Some(*id) } else { None }).collect();
        if ids.is_empty() {
            return Err(clarify("A quais municípios você se refere? Execute um ranking ou informe os nomes."));
        }
        request.scope.place = PlaceFilter::Municipalities(SmallSet::new(ids));
        act = "refer_to_result".into();
    }
    if has(n, &["desses setores"]) {
        let ids: Vec<_> = state.result_members.iter().filter_map(|m| if let Member::Sector(id) = m { Some(*id) } else { None }).collect();
        if ids.is_empty() {
            return Err(clarify("A quais setores você se refere? Execute um ranking ou informe as atividades."));
        }
        request.scope.activity = ActivityFilter::Sectors { ids: SmallSet::new(ids), cnaes: SmallSet::default(), scope: CnaeScope::Main };
        act = "refer_to_result".into();
    }
    if !followup || !excluded_muni.is_empty() {
        request.scope.exclude.municipalities = SmallSet::new(excluded_muni);
    }
    if !followup || !excluded_ufs.is_empty() {
        request.scope.exclude.ufs = SmallSet::new(excluded_ufs);
    }
    if !followup || !excluded_sectors.is_empty() {
        request.scope.exclude.sectors = SmallSet::new(excluded_sectors);
    }
    if !sectors.is_empty() {
        request.scope.activity = ActivityFilter::Sectors {
            ids: SmallSet::new(sectors.iter().copied()),
            cnaes: SmallSet::default(),
            scope: if has(n, &["secundaria", "secundarias", "secundario", "secundarios"]) { CnaeScope::Any } else { CnaeScope::Main },
        };
    }
    explicit_cnae(n, &raw, &mut request)?;
    let domain = !hits.is_empty()
        || lookup(&raw).is_some()
        || has(
            n,
            &[
                "empresas",
                "empresa",
                "estabelecimentos",
                "estabelecimento",
                "cnpj",
                "cnpjs",
                "negocios",
                "mei",
                "simples",
                "capital social",
                "setor",
                "setores",
                "atividade",
                "atividades",
            ],
        );
    if !domain && !followup && meta.is_none() {
        return Err(reject("Não identifiquei empresas, atividades ou locais de uma pesquisa comercial."));
    }
    // Unknown place/sector names must not silently turn into a national/all-sector query.
    if hits.is_empty()
        && !followup
        && meta.is_none()
        && lookup(&raw).is_none()
        && !has(n, &["empresas", "estabelecimentos", "cnpj", "cnpjs", "mei", "simples", "setor", "setores", "atividade", "atividades"])
    {
        return Err(clarify("Qual atividade e município você quer pesquisar?"));
    }
    filters(n, m, &raw, &mut request, &mut act, population)?;
    if has(n, &["nao estao ativas", "nao estao ativos", "nao tao ativas", "nao tao ativos", "inativas", "inativos", "inativa", "inativo"]) {
        notes.push("Considerei suspensas, inaptas e baixadas; situação nula não foi incluída.".into());
    }
    if matches!(request.scope.place, PlaceFilter::Near { .. }) && !ufs.is_empty() {
        request.scope.within_ufs = SmallSet::new(ufs.iter().copied());
    }
    if let Some(topic) = meta {
        request.op = Op::Meta { topic };
        request.time.period = None;
        request.scope.status = None;
        act = "meta_question".into();
    } else if let Some(id) = lookup(&raw) {
        request.op = Op::Lookup {
            id: id.map_err(|_| {
                clarify("O CNPJ informado é inválido. Confira os dígitos verificadores ou informe uma raiz de 8 caracteres.")
            })?,
        };
        request.scope = Filters::default();
        request.time.period = None;
    } else {
        let mut temporal = time::period(&temporal_raw, anchor);
        if has(n, &["ultimos", "ultimas", "desde", "a partir"]) && temporal.is_none() {
            return Err(clarify("Não consegui resolver o período. Informe datas válidas ou uma quantidade de meses ou dias."));
        }
        let closed = cue(m, &cues.closures);
        let registered = (cue(m, &cues.registrations)
            && (temporal.is_some()
                || has(n, &["abriram", "surgiram", "comecaram", "criadas", "criados", "foram registrados", "registros de inicio"])))
            || temporal.is_some() && has(n, &["abertas", "abertos", "novas", "novos"]);
        let growth = cue(m, &cues.growth);
        // An explicit census reference selects the census population; otherwise the
        // ambient default (usually the latest IBGE estimate) applies.
        let population = if has(n, &["censo", "censitario", "censitaria"]) { PopulationRef::IbgeCensus(2022) } else { population };
        let metric =
            if has(n, &["habitantes por estabelecimento", "habitantes por empresa", "habitantes por farmacia", "habitantes por padaria"]) {
                Metric::InhabitantsPer(population)
            } else if has(n, &["por habitante", "por habitantes", "por 10 mil habitantes", "per capita", "densidade"]) {
                Metric::Per10k(population)
            } else if growth {
                Metric::ActiveChange { pct: has(n, &["variacao percentual", "crescimento percentual", "por cento"]) || raw.contains('%') }
            } else if has(n, &["saldo", "registros menos baixas", "aberturas menos baixas"]) {
                Metric::Flow(FlowMetric::NetRegistrations)
            } else if has(n, &["opcoes pelo mei", "opcao pelo mei em", "entraram no mei", "aderiram ao mei"]) {
                Metric::Flow(FlowMetric::MeiOptIns)
            } else if has(n, &["exclusoes do mei", "sairam do mei", "excluidas do mei"]) {
                Metric::Flow(FlowMetric::MeiExits)
            } else if closed {
                Metric::Flow(FlowMetric::Closures)
            } else if registered {
                Metric::Flow(FlowMetric::Registrations)
            } else {
                Metric::Stock
            };
        let iscompare = (cue(m, &cues.compare) && !has(n, &["antes de", "quero comparar com"]))
            || followup && (n.starts_with("e com ") || has(n, &["qual tem mais", "qual tem menos", "qual das duas", "compare com"]));
        let isrank = cue(m, &cues.rank) && has(n, &["mais", "menos", "maior", "menor", "variacao", "top", "ranking"]);
        let istrend = cue(m, &cues.trend) || has(n, &["evolucao mensal", "evolucao anual"]);
        let isbreakdown = cue(m, &cues.breakdown);
        // A file of companies lists the records a count would measure. Rankings, series,
        // comparisons and distributions are exported as their own table instead.
        let inherited = followup && !full_query;
        let analytical =
            iscompare || isrank || istrend || isbreakdown || inherited && !matches!(request.op, Op::Count { .. } | Op::List { .. });
        let measured = if inherited { request.op.metric() } else { Some(metric) };
        let records = export
            && !analytical
            && !matches!(request.op, Op::List { .. } if inherited)
            && matches!(measured, Some(Metric::Stock | Metric::Flow(FlowMetric::Closures | FlowMetric::Registrations)));
        let islist = (cue(m, &cues.list) || has(n, &["quais"]) && has(n, &["maior capital", "mais capital"]))
            && !has(n, &["evolucao", "distribua", "distribuicao"])
            && !cue(m, &cues.trend)
            || records;
        if let Some((event_text, opened_text)) =
            ["com inicio de atividade", "com data de inicio de atividade"].iter().find_map(|phrase| temporal_raw.split_once(phrase))
        {
            let Some(Period::Days(opened)) = time::period(opened_text, anchor) else {
                return Err(clarify("Qual o período de início de atividade? Informe o ano ou as datas."));
            };
            if closed {
                // The registration date is a filter, never the date of a closure.
                temporal =
                    Some(time::period(event_text, anchor).ok_or_else(|| {
                        clarify("Qual o período da baixa cadastral? A data de início de atividade é um filtro separado.")
                    })?);
            }
            if closed || islist || !registered {
                request.scope.opened = Some(opened);
            }
        }
        let dim = dimension(n);
        if islist {
            let explicit = list_limit(n);
            // An exported list shows one page and writes the requested rows to the file.
            // Above 100 rows a chat list is refused with a clamped option, never truncated.
            let limit = match explicit {
                Some(rows) if export => rows.min(100) as u16,
                Some(rows) => u16::try_from(rows).unwrap_or(u16::MAX),
                None => 20,
            };
            rows = explicit.filter(|_| export);
            let order = if has(n, &["capital social", "capital"]) && has(n, &["maior", "mais", "ordenados"]) {
                ListOrder::CapitalDesc
            } else if has(n, &["antigos para os mais novos", "mais antigos", "mais antigas"])
                && !has(n, &["mais novos para os mais antigos"])
            {
                ListOrder::OldestFirst
            } else if has(n, &["mais novos", "mais recentes", "recentes primeiro", "mais novas"]) {
                ListOrder::NewestFirst
            } else {
                ListOrder::Cnpj
            };
            let flow = if closed {
                Some(FlowMetric::Closures)
            } else if registered {
                Some(FlowMetric::Registrations)
            } else if let (true, Some(Metric::Flow(flow))) = (inherited, request.op.metric()) {
                Some(flow)
            } else {
                None
            };
            let period = temporal.or(if inherited { request.time.period } else { None });
            // A period with no event ("padarias de 2025", "desde 2020") filters nothing a list
            // can apply: ask whether it is the opening or the closure, never drop it.
            if flow.is_none() && temporal.is_some() && request.scope.opened.is_none() && request.scope.status_date.is_none() {
                return Err(clarify(
                    "Esse período se refere à abertura ou à baixa das empresas? Ex.: “abertas em 2025” ou “que fecharam em 2025”.",
                ));
            }
            request.op = Op::List { order, limit };
            request.time.period = None;
            // The event a list names filters its records: a closure by the date of the
            // closed status, a registration by the start of activity. Neither is dropped.
            match (flow, period) {
                (Some(FlowMetric::Closures), period) => {
                    request.scope.status.get_or_insert(StatusSet::only(Status::Closed));
                    if let Some(Period::Days(days)) = period {
                        request.scope.status_date = Some(days);
                    }
                }
                (Some(FlowMetric::Registrations), Some(Period::Days(days))) => {
                    request.scope.opened.get_or_insert(days);
                    request.scope.status.get_or_insert(StatusSet::ALL);
                }
                _ => {}
            }
        } else if iscompare {
            if municipalities.len() == 1 && !ufs.is_empty() {
                return Err(clarify("Você quer comparar municípios ou estados? Qualifique os locais para usar a mesma dimensão."));
            }
            let (dim, members) = if municipalities.len() >= 2 {
                (Dim::Municipality, SmallSet::new(municipalities.iter().map(|id| Member::Municipality(*id))))
            } else if ufs.len() >= 2 {
                (Dim::Uf, SmallSet::new(ufs.iter().map(|id| Member::Uf(*id))))
            } else if sectors.len() >= 2 {
                (Dim::Sector, SmallSet::new(sectors.iter().map(|id| Member::Sector(*id))))
            } else {
                let explicit: Vec<_> =
                    municipalities.iter().map(|id| Member::Municipality(*id)).chain(ufs.iter().map(|id| Member::Uf(*id))).collect();
                let mut candidates = explicit.clone();
                for member in &state.focus {
                    if !candidates.contains(member) {
                        candidates.push(*member);
                    }
                    if !explicit.is_empty() && candidates.len() >= 2 {
                        break;
                    }
                }
                let members = SmallSet::new(candidates);
                if members.len() < 2 {
                    return Err(clarify("Quais dois a quatro locais ou setores você quer comparar?"));
                }
                let dim = match members.first() {
                    Some(Member::Municipality(_)) => Dim::Municipality,
                    Some(Member::Uf(_)) => Dim::Uf,
                    Some(Member::Sector(_)) => Dim::Sector,
                    _ => return Err(clarify("Quais locais ou setores você quer comparar?")),
                };
                (dim, members)
            };
            if matches!(dim, Dim::Municipality | Dim::Uf) {
                request.scope.place = PlaceFilter::Any;
            } else {
                request.scope.activity = ActivityFilter::Any;
            }
            request.op = Op::Compare { dim, members, metric };
        } else if isrank {
            request.op = Op::Rank {
                dim: dim.unwrap_or(Dim::Municipality),
                metric,
                dir: if has(n, &["menor", "menores", "menos empresas", "menos estabelecimentos", "menos farmacias", "menos padarias"]) {
                    Dir::Asc
                } else {
                    Dir::Desc
                },
                k: u8::try_from(list_limit(n).unwrap_or(10)).map_err(|_| clarify("Informe um limite de ranking de até 255."))?,
                min_base: if matches!(metric, Metric::ActiveChange { pct: true }) || has(n, &["base inicial", "base minima"]) {
                    30
                } else {
                    0
                },
            };
        } else if istrend {
            let flow = if let Metric::Flow(flow) = metric {
                flow
            } else {
                return Err(clarify("Qual série você quer: registros, baixas ou saldo cadastral?"));
            };
            request.op = Op::Trend { metric: flow, grain: if has(n, &["anual", "por ano"]) { Grain::Year } else { Grain::Month } };
        } else if isbreakdown {
            request.op = Op::Breakdown {
                dim: dim.ok_or_else(|| clarify("Por qual dimensão você quer dividir: porte, situação, atividade ou local?"))?,
                metric: if has(n, &["participacao", "percentual", "percentuais"]) { Metric::Share } else { metric },
            };
        } else if !followup || full_query {
            request.op = Op::Count { metric };
        }
        if matches!(request.op.metric(), Some(Metric::Flow(_))) {
            request.time.period =
                temporal.or(if followup { request.time.period } else { None }).or_else(|| time::period("este ano", anchor));
            request.scope.status = if matches!(request.op.metric(), Some(Metric::Flow(FlowMetric::Closures))) {
                if has(n, &["ainda ativas", "ainda ativos", "somente ativas", "somente ativos", "so ativas", "so ativos"]) {
                    Some(StatusSet::only(Status::Active))
                } else {
                    Some(StatusSet::only(Status::Closed))
                }
            } else if !has(n, &["continuam ativos", "continuam ativas", "ainda ativas", "ainda ativos", "ativas hoje", "ativos hoje"]) {
                Some(StatusSet::ALL)
            } else {
                Some(StatusSet::only(Status::Active))
            };
        }
        if matches!(metric, Metric::ActiveChange { .. }) {
            let cs = time::competences(&raw);
            let from = cs.first().copied().unwrap_or(Competence(current.0.saturating_sub(6)));
            let to = cs.get(1).copied().unwrap_or(current);
            request.time.period = Some(Period::Competences { from, to });
            request.scope.status = Some(StatusSet::only(Status::Active));
        }
        if matches!(metric, Metric::Stock)
            && (has(m, &["estavam ativas", "estavam ativos", "estoque em", "competencia"])
                || temporal.is_some() && has(m, &["tinham", "existiam", "havia"]))
        {
            let years = time::years(&temporal_raw);
            if !years.is_empty() && !competences.iter().any(|c| years.contains(&c.year())) {
                return Err(reject("Não há snapshots disponíveis para o ano solicitado. Informe uma competência disponível."));
            }
            request.time.competence =
                time::competences(&raw).first().copied().ok_or_else(|| clarify("Qual competência mensal você quer consultar?"))?;
        }
    }
    if hits.iter().any(|h| h.fuzzy) {
        notes.push("Confirme o município interpretado por aproximação.".into());
    }
    Ok(Frame { request, act, notes, rows })
}

/// Words that describe companies without naming an activity: status, size, legal form,
/// registration events and connectives ("empresas ativas", "padarias e matrizes").
fn known_descriptor(word: &str) -> bool {
    matches!(
        word,
        "ativas"
            | "ativos"
            | "ativa"
            | "ativo"
            | "abertas"
            | "abertos"
            | "aberta"
            | "baixadas"
            | "baixados"
            | "baixada"
            | "fechadas"
            | "fechados"
            | "encerradas"
            | "encerrados"
            | "extintas"
            | "extintos"
            | "suspensas"
            | "suspensos"
            | "inaptas"
            | "inaptos"
            | "nulas"
            | "inativas"
            | "inativos"
            | "novas"
            | "novos"
            | "registradas"
            | "registrados"
            | "cadastradas"
            | "cadastrados"
            | "criadas"
            | "criados"
            | "existentes"
            | "optantes"
            | "matrizes"
            | "matriz"
            | "filiais"
            | "filial"
            | "sediadas"
            | "sediados"
            | "brasileiras"
            | "locais"
            | "listadas"
            | "encontradas"
            | "mais"
            | "menos"
            | "maiores"
            | "menores"
            | "pequenas"
            | "grandes"
            | "micro"
            | "microempresas"
            | "individuais"
            | "limitadas"
            | "ltda"
            | "tiveram"
            | "fecharam"
            | "abriram"
            | "estao"
            | "foram"
            | "sao"
            | "possuem"
            | "existem"
            | "havia"
            | "tinham"
            | "estavam"
            | "com"
            | "sem"
            | "nao"
            | "para"
            | "pelo"
            | "pela"
            | "entre"
            | "desse"
            | "dessa"
            | "desses"
            | "dessas"
            | "neste"
            | "nesta"
            | "nesse"
            | "nessa"
            | "deste"
            | "desta"
            | "todas"
            | "todos"
            | "cujo"
            | "cuja"
            | "onde"
            | "aqui"
            | "agora"
            | "hoje"
            | "ainda"
            | "situacao"
            | "porte"
            | "capital"
            | "cadastro"
            | "listar"
            | "contar"
            | "funcionando"
            | "operando"
            | "atuantes"
            | "atuando"
            | "presentes"
            | "instaladas"
            | "localizadas"
            | "situadas"
            | "suas"
            | "seus"
            | "telefone"
            | "email"
            | "contatos"
            | "simples"
            | "demais"
    )
}

/// Mask entities while keeping the separators needed to parse literal dates.
fn temporal_text(text: &str, hits: &[Hit]) -> String {
    let mut at = 0;
    normalize::tokens(text)
        .into_iter()
        .map(|token| {
            let end = at + normalize::words(&token).len();
            let masked = hits.iter().any(|hit| hit.start < end && at < hit.end);
            at = end;
            if masked { "_entity_".to_owned() } else { token }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Replace entity words with a sentinel so command cues inside place or sector
/// names ("Itaporanga d'Ajuda", "Contagem") do not change the intent.
fn masked_text(words: &[String], hits: &[Hit]) -> String {
    let mut masked = words.to_vec();
    for hit in hits {
        let end = hit.end.min(masked.len());
        for word in &mut masked[hit.start.min(end)..end] {
            *word = "entidade".into();
        }
    }
    masked.join(" ")
}

fn hits_adjacent_place(pack: &LanguagePack, words: &[String], at: usize) -> bool {
    at > 0
        && pack
            .lexicon
            .scan(&words[..at])
            .last()
            .is_some_and(|h| h.end == at && h.values.iter().any(|e| matches!(e, Entity::Municipality { .. })))
}

fn meta_topic(n: &str) -> Option<MetaTopic> {
    if has(n, &["quais cnaes", "que codigos de atividade", "quais codigos"]) {
        Some(MetaTopic::Cnaes)
    } else if has(n, &["de que data", "de quando", "ate quando", "atualizada", "atualizados", "competencia dos dados"]) {
        Some(MetaTopic::DataDate)
    } else if has(n, &["quais fontes", "de onde vem", "fontes dos dados"]) {
        Some(MetaTopic::Sources)
    } else if has(n, &["metodologia", "como voce conta", "como voce calculou", "como calculou"]) {
        Some(MetaTopic::Methodology)
    } else if has(n, &["que pesquisas", "quais filtros", "o que voce faz", "ajuda", "como pesquisar"]) {
        Some(MetaTopic::Help)
    } else {
        None
    }
}

fn lookup(raw: &str) -> Option<Result<CnpjRef, opencnpj_query::ir::InvalidCnpj>> {
    let tokens: Vec<_> = raw.split_whitespace().collect();
    tokens.iter().enumerate().find_map(|(at, s)| {
        // Sentence punctuation or filler can be attached to the identifier
        // (".../0001-87,"); strip it before validating the CNPJ itself.
        let token = s.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if time::date_shaped(token) {
            return None;
        }
        let compact: String = token.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let before = tokens[at.saturating_sub(3)..at].join(" ");
        let before = normalize::words(&before).join(" ");
        let identifier_hint = has(&before, &["cnpj", "raiz", "registro"]);
        let numeric_filter = has(&before, &["capital", "social", "habitantes", "populacao"]);
        let full_format = compact.len() == 14 && token.contains('/') && token.contains('-');
        if numeric_filter || !(tokens.len() == 1 || identifier_hint || full_format) {
            return None;
        }
        ((token.contains('/') || token.matches('.').count() >= 2 || compact.len() == 14 || compact.len() == 8)
            && (compact.len() == 14 || compact.len() == 8)
            && compact.chars().any(|c| c.is_ascii_digit()))
        .then(|| CnpjRef::parse(token))
    })
}

fn dimension(n: &str) -> Option<Dim> {
    if has(n, &["grupos de uf", "por uf", "estados", "ufs"]) {
        Some(Dim::Uf)
    } else if has(n, &["municipios", "cidades", "por municipio"]) {
        Some(Dim::Municipality)
    } else if has(n, &["setores", "por setor", "grupos de setor"]) {
        Some(Dim::Sector)
    } else if has(n, &["secao cnae", "secoes", "por secao"]) {
        Some(Dim::Cnae(CnaeLevel::Section))
    } else if has(n, &["divisao cnae", "divisoes"]) {
        Some(Dim::Cnae(CnaeLevel::Division))
    } else if has(n, &["grupo cnae", "grupos cnae"]) {
        Some(Dim::Cnae(CnaeLevel::Group))
    } else if has(n, &["subclasse cnae", "subclasses"]) {
        Some(Dim::Cnae(CnaeLevel::Subclass))
    } else if has(n, &["por porte", "grupos de porte"]) {
        Some(Dim::Porte)
    } else if has(n, &["por situacao", "grupos de situacao"]) {
        Some(Dim::Status)
    } else if has(n, &["por matriz", "por matriz e filial"]) {
        Some(Dim::HeadOffice)
    } else if has(n, &["por opcao pelo mei", "por mei"]) {
        Some(Dim::Mei)
    } else {
        None
    }
}

/// An explicit row count ("liste 50", "as 5000 padarias"). Counts after "de" stay small
/// so a year ("de 2025 em …") is never read as a count.
fn list_limit(n: &str) -> Option<u32> {
    let t: Vec<String> = n.split_whitespace().map(str::to_owned).collect();
    if let Some((q, end)) = time::quantity(&t, 0)
        && !(1900..=2100).contains(&q)
        && t.get(end).is_some_and(|s| time::number(s).is_none())
    {
        return u32::try_from(q).ok();
    }
    for i in 0..t.len() {
        if matches!(t[i].as_str(), "liste" | "listar" | "traga" | "tragam" | "traz" | "trazem" | "top" | "os" | "as" | "de")
            && let Some((q, end)) = time::quantity(&t, i + 1)
            && (if t[i] == "de" { q <= 1000 && t.get(end).is_some_and(|s| s == "em" || s == "por") } else { q <= 1_000_000 })
        {
            return u32::try_from(q).ok();
        }
        if matches!(t[i].as_str(), "lista" | "listagem" | "relacao") {
            let mut j = i + 1;
            if t.get(j).is_some_and(|s| s == "de" || s == "com") {
                j += 1;
            }
            if let Some((q, _)) = time::quantity(&t, j)
                && q <= 1_000_000
            {
                return u32::try_from(q).ok();
            }
        }
    }
    None
}

fn explicit_cnae(n: &str, raw: &str, r: &mut Request) -> Result<(), Box<Interpretation>> {
    if !has(n, &["cnae", "cnaes"]) || has(n, &["quais cnaes", "que codigos"]) {
        return Ok(());
    }
    let t: Vec<String> = raw.split_whitespace().map(str::to_owned).collect();
    let mut codes = Vec::new();
    for (i, word) in t.iter().enumerate() {
        let digits: String = word.chars().filter(char::is_ascii_digit).collect();
        let nearby = t[i.saturating_sub(4)..i].join(" ");
        if !has(&nearby, &["cnae", "subclasse", "classe", "grupo", "divisao", "codigo", "secao"]) {
            continue;
        }
        if digits.len() == 7 {
            if let Ok(code) = digits.parse() {
                codes.push(CnaeCode::Subclass(code));
            }
        } else {
            // Trailing filler punctuation (", por favor?", "??") must not drop
            // an explicit division/group/class code.
            let stripped = word.trim_matches(|c: char| matches!(c, ',' | '?' | '.' | '!' | ';' | ':'));
            if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(code) = stripped.parse::<u32>() {
                    match digits.len() {
                        2 => codes.push(CnaeCode::Division(code as u8)),
                        3 => codes.push(CnaeCode::Group(code as u16)),
                        5 => codes.push(CnaeCode::Class(code)),
                        _ => {}
                    }
                }
            } else if nearby.ends_with("secao") && stripped.len() == 1 && stripped.as_bytes()[0].is_ascii_alphabetic() {
                codes.push(CnaeCode::Section(stripped.to_ascii_uppercase().as_bytes()[0]));
            }
        }
    }
    if !codes.is_empty() {
        r.scope.activity = ActivityFilter::Cnae {
            codes: SmallSet::new(codes),
            scope: if has(n, &["secundaria", "secundarias", "secundario", "secundarios"]) { CnaeScope::Any } else { CnaeScope::Main },
        };
    }
    Ok(())
}

fn filters(n: &str, m: &str, raw: &str, r: &mut Request, act: &mut String, population: PopulationRef) -> Result<(), Box<Interpretation>> {
    let clear = has(
        n,
        &[
            "retire",
            "retira",
            "tira",
            "tire",
            "remova",
            "remover",
            "limpe",
            "desfaca",
            "pode tirar",
            "volte a incluir",
            "inclua filiais",
            "conte matrizes e filiais",
            "conte matriz e filial",
            "conte matriz e filiais",
            "matriz quanto filial",
        ],
    );
    if clear {
        *act = if has(n, &["mantenha", "mantem", "mantém"]) { "compound_edit" } else { "remove_filter" }.into();
        let keep_head_office = has(
            n,
            &[
                "mantenha o filtro de matriz",
                "mantenha o filtro de matrizes",
                "mantenha matriz",
                "mantenha as matrizes",
                "mantem o filtro de matriz",
                "mantem matriz",
            ],
        );
        let keep_mei = has(n, &["mantenha o filtro de mei", "mantenha o mei", "mantem o filtro de mei", "mantem o mei"]);
        if !keep_head_office && has(m, &["matriz", "filiais", "filial", "matrizes"]) {
            r.scope.head_office = Tri::Any;
            r.unit = Unit::Establishments;
        }
        if !keep_mei && has(n, &["mei"]) {
            r.scope.mei = Tri::Any;
        }
        if has(n, &["capital", "limite de capital"]) && !has(n, &["mantenha o capital", "mantenha o limite de capital"]) {
            r.scope.capital = None;
        }
        if has(n, &["porte", "portes"]) {
            r.scope.porte = PorteSet::ALL;
        }
        if has(n, &["capital"]) {
            r.scope.capital = None;
        }
    } else {
        if has(m, &["matrizes", "matriz", "sediadas", "sediados", "sede"]) {
            r.scope.head_office = Tri::Yes;
        }
        if has(m, &["sediadas", "sediados", "quantas matrizes", "quantas sao matrizes"]) {
            r.unit = Unit::HeadOffices;
            r.scope.head_office = Tri::Any;
        }
        if has(m, &["filiais", "filial"]) && !has(m, &["matrizes e filiais", "matriz e filial"]) {
            r.scope.head_office = if has(
                m,
                &[
                    "sem filiais",
                    "sem filial",
                    "sem incluir filiais",
                    "exclui filiais",
                    "exclui as filiais",
                    "excluir filiais",
                    "excluindo filiais",
                    "nao sejam filiais",
                    "nao sao filiais",
                    "nao e filial",
                ],
            ) {
                Tri::Yes
            } else {
                Tri::No
            };
        }
        if r.scope.head_office == Tri::No {
            r.unit = Unit::Establishments;
        }
        if has(m, &["matrizes e filiais", "matriz e filial"]) {
            r.scope.head_office = Tri::Any;
        }
        if has(n, &["mei"]) {
            r.scope.mei = if has(
                n,
                &[
                    "nao sao mei",
                    "nao sejam mei",
                    "nao sao optantes pelo mei",
                    "nao sejam optantes pelo mei",
                    "excluindo mei",
                    "excluindo os mei",
                    "excluindo as mei",
                    "fora do mei",
                    "sem mei",
                    "nao mei",
                    "exclui mei",
                ],
            ) {
                Tri::No
            } else if has(n, &["por opcao pelo mei", "por mei", "opcoes pelo mei", "opcao pelo mei em", "exclusoes do mei"]) {
                Tri::Any
            } else {
                Tri::Yes
            };
        }
        if has(n, &["simples", "simples nacional"]) {
            r.scope.simples = if has(
                n,
                &[
                    "fora do simples",
                    "nao optantes pelo simples",
                    "nao sao optantes pelo simples",
                    "nao sejam optantes pelo simples",
                    "nao sao do simples",
                    "sem simples",
                ],
            ) {
                Tri::No
            } else {
                Tri::Yes
            };
        }
        if has(n, &["microempresa", "microempresas", "micro", "porte micro"]) {
            r.scope.porte = PorteSet::only(Porte::Micro);
        }
        if has(n, &["pequeno porte", "epp"]) {
            r.scope.porte = PorteSet::only(Porte::Small);
        }
        if has(n, &["porte cadastral demais", "porte demais", "porte 05"]) {
            r.scope.porte = PorteSet::only(Porte::Other);
        }
        if has(n, &["porte nao informado", "porte cadastral nao informado"]) {
            r.scope.porte = PorteSet::only(Porte::Unreported);
        }
    }
    if has(n, &["todas as situacoes", "em qualquer situacao"]) {
        r.scope.status = Some(StatusSet::ALL);
    } else {
        // "sociedade anônima fechada" is a legal form (natureza 2054), not a closed status.
        let status_text = n.replace("anonimas fechadas", "anonimas").replace("anonima fechada", "anonima");
        let mut statuses = Vec::new();
        for (phrases, status) in [
            (&["ativos", "ativas", "ativa", "ativo", "funcionando", "em funcionamento"][..], Status::Active),
            (&["suspensos", "suspensas", "suspensa"][..], Status::Suspended),
            (&["inaptos", "inaptas", "inapta"][..], Status::Unfit),
            (
                &["baixados", "baixadas", "baixada", "fechadas", "fechados", "encerradas", "encerrados", "extintas", "extintos"][..],
                Status::Closed,
            ),
            (&["situacao cadastral nula", "situacao nula"][..], Status::Null),
        ] {
            if has(&status_text, phrases) {
                statuses.push(status);
            }
        }
        if !statuses.is_empty() {
            r.scope.status = Some(StatusSet::new(statuses));
        }
    }
    if has(n, &["nao estao ativas", "nao estao ativos", "nao tao ativas", "nao tao ativos", "inativas", "inativos", "inativa", "inativo"]) {
        r.scope.status = Some(StatusSet::new([Status::Suspended, Status::Unfit, Status::Closed]));
    }
    let code_words: Vec<_> = n.split_whitespace().collect();
    for (i, word) in code_words.iter().enumerate() {
        if matches!(*word, "situacao" | "status" | "porte")
            && let Some(code) = code_words.iter().skip(i + 1).take(3).find(|v| v.len() == 2 && v.bytes().all(|b| b.is_ascii_digit()))
        {
            if *word == "porte" {
                r.scope.porte = PorteSet::only(match *code {
                    "00" => Porte::Unreported,
                    "01" => Porte::Micro,
                    "03" => Porte::Small,
                    "05" => Porte::Other,
                    _ => return Err(clarify("Qual código de porte RFB: 00, 01, 03 ou 05?")),
                });
            } else {
                r.scope.status = Some(StatusSet::only(match *code {
                    "01" => Status::Null,
                    "02" => Status::Active,
                    "03" => Status::Suspended,
                    "04" => Status::Unfit,
                    "08" => Status::Closed,
                    _ => return Err(clarify("Qual situação cadastral RFB: 01, 02, 03, 04 ou 08?")),
                }));
            }
        }
    }
    let t = normalize::tokens(raw);
    if has(n, &["telefone", "email", "e mail"]) {
        r.scope.contact = match (has(n, &["telefone"]), has(n, &["email", "e mail"])) {
            (true, true) => ContactNeed::Both,
            (true, false) => ContactNeed::Phone,
            _ => ContactNeed::Email,
        };
    }
    for i in 0..t.len() {
        if t[i] == "pagina" {
            let q = i.checked_sub(1).and_then(|at| time::number(&t[at])).or_else(|| t.get(i + 1).and_then(|s| time::number(s)));
            if let Some(q) = q {
                r.output.page = u16::try_from(q).map_err(|_| clarify("Qual página você quer?"))?;
            }
        }
        if (t[i] == "codigo" || t[i] == "natureza")
            && let Some((q, _)) = time::quantity(&t, i + 1)
            && (1000..=9999).contains(&q)
        {
            r.scope.natureza = SmallSet::new([NaturezaCode::Code(q as u16)]);
        }
    }
    if has(n, &["sociedade empresaria limitada", "sociedades empresarias limitadas", "ltda"]) {
        r.scope.natureza = SmallSet::new([NaturezaCode::Code(2062)]);
    }
    if has(n, &["empresario individual"]) {
        r.scope.natureza = SmallSet::new([NaturezaCode::Code(2135)]);
    }
    if has(n, &["sociedade anonima fechada"]) {
        r.scope.natureza = SmallSet::new([NaturezaCode::Code(2054)]);
    }
    if has(n, &["grupo de natureza juridica 2", "entidades empresariais"]) {
        r.scope.natureza = SmallSet::new([NaturezaCode::Group(2)]);
    }
    if has(n, &["capital social"])
        && let Some(at0) = (1..t.len()).find(|&i| {
            t[i] == "social" && t[i - 1] == "capital" && i.checked_sub(2).is_none_or(|prev| !matches!(t[prev].as_str(), "maior" | "mais"))
        })
    {
        let at = (at0..t.len())
            .find(|&i| {
                t[i] == "social" && t[i - 1] == "capital" && (i + 1..(i + 8).min(t.len())).any(|j| time::money_quantity(&t, j).is_some())
            })
            .unwrap_or(at0);
        let window_end = (at + 8).min(t.len());
        let nums: Vec<_> = (at + 1..window_end)
            .filter_map(|i| time::money_quantity(&t, i))
            .filter(|(_, end)| t.get(*end).is_none_or(|s| s != "habitantes"))
            .map(|(n, _)| n)
            .collect();
        // Scale words are not parsed as independent quantities.
        if let Some(&value) = nums.first() {
            let cents = value;
            let interval =
                (at0..t.len()).any(|j| t[j] == "entre" && ((j + 1..t.len()).take(2).any(|k| time::money_quantity(&t, k).is_some())));
            let (from, to) = if interval {
                (Some(cents), Some(*nums.get(1).ok_or_else(|| clarify("Informe os dois limites de capital social."))?))
            } else if has(n, &["ate", "no maximo"]) {
                (None, Some(cents))
            } else if has(n, &["igual a zero"]) {
                (Some(0), Some(0))
            } else if has(n, &["acima de", "mais de"]) {
                (Some(cents.saturating_add(1)), None)
            } else {
                (Some(cents), None)
            };
            r.scope.capital = Some(Range { from, to });
        }
        if r.scope.capital.is_none() && !has(n, &["maior capital", "mais capital", "por capital"]) {
            return Err(clarify("Qual o valor ou intervalo de capital social? Informe o valor em reais."));
        }
    }
    if has(n, &["habitantes"])
        && has(n, &["municipios com", "cidades com", "municipios de", "cidades de"])
        && !has(n, &["por estabelecimento", "por empresa"])
        // A density phrase ("por 10 mil habitantes") may precede the population band;
        // bind the band to the last "habitantes" whose own window has no "por".
        && let Some(at) = (0..t.len()).rev().find(|&at| {
            t[at] == "habitantes" && !t[at.saturating_sub(4)..at].iter().any(|s| s == "por")
        })
    {
        let nums: Vec<_> = (at.saturating_sub(4)..at).filter_map(|i| time::quantity(&t, i)).collect();
        if let Some(&(value, _)) = nums.first() {
            let val = u32::try_from(value).map_err(|_| clarify("Qual o limite de população?"))?;
            let (from, to) = if has(n, &["menos de", "abaixo de"]) {
                (None, Some(val.saturating_sub(1)))
            } else if has(n, &["ate"]) {
                (None, Some(val))
            } else if has(n, &["mais de", "acima de"]) {
                (Some(val.saturating_add(1)), None)
            } else {
                (Some(val), None)
            };
            r.scope.place_population = Some(Range { from, to });
            r.scope.population_ref = Some(population);
        }
    }
    if has(m, &["vizinhos", "vizinhas", "vizinhanca", "centroide", "proximas", "proximos", "regiao de"])
        && let PlaceFilter::Municipalities(ids) = &r.scope.place
        && let Some(&center) = ids.first()
    {
        let mut rule = NearRule::Neighbours(1);
        for i in 1..t.len() {
            if t[i] == "km"
                && let Some(n) = time::number(&t[i - 1])
            {
                rule = NearRule::WithinKm(u16::try_from(n).map_err(|_| clarify("Qual raio em quilômetros?"))?);
            }
            if t[i] == "passos"
                && let Some(n) = time::number(&t[i - 1])
            {
                rule = NearRule::Neighbours(u8::try_from(n).map_err(|_| clarify("Quantos passos de vizinhança?"))?);
            }
        }
        r.scope.place = PlaceFilter::Near { center, rule };
    }
    Ok(())
}
