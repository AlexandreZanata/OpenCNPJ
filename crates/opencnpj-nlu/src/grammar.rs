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
    pub dated_closures: Vec<String>,
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
    pub vocabulary: &'a crate::vocabulary::Known,
    pub delivery: &'a crate::skills::Vocabulary,
}

const PHONE_WORDS: &[&str] = &["telefone", "telefones", "fone", "celular", "whatsapp", "zap"];
const EMAIL_WORDS: &[&str] = &["email", "emails", "e mail", "e mails"];

pub(crate) const SECTOR_QUALIFIERS: &[&str] = &["area", "atividade", "setor", "ramo", "segmento"];
const PLACE_QUALIFIERS: &[&str] = &["cidade", "municipio", "cidades", "municipios", "estado"];

/// Use the same grammar for each sector qualifier, provided it points to a known activity.
fn normalize_sector_qualifier(words: &mut [String], hits: &mut Vec<Hit>) {
    let mut qualified = false;
    let mut generic_farms = BTreeSet::new();
    for hit in hits.iter().filter(|hit| hit.values.iter().any(|entity| matches!(entity, Entity::Sector(_)))) {
        let mut end = hit.start;
        loop {
            let genitive = end > 0 && matches!(words[end - 1].as_str(), "de" | "da" | "do" | "das" | "dos");
            let Some(start) = end.checked_sub(2 + usize::from(genitive)) else { break };
            if !(end < hit.start || genitive)
                || !matches!(words[start].as_str(), "na" | "no" | "da" | "do" | "em")
                || !SECTOR_QUALIFIERS.contains(&words[start + 1].as_str())
                || hits.iter().any(|other| other.start < end && start < other.end)
            {
                break;
            }
            words[start] = "no".into();
            words[start + 1] = "setor".into();
            if genitive {
                words[start + 2] = "de".into();
            }
            qualified = true;
            end = start;
        }
        if end < hit.start && end > 0 && matches!(words[end - 1].as_str(), "fazenda" | "fazendas") {
            generic_farms.insert(end - 1);
        }
    }
    hits.retain(|hit| {
        let generic = hit.end - hit.start == 1
            && generic_farms.contains(&hit.start)
            && hit.values.iter().all(|entity| matches!(entity, Entity::Sector(_)));
        if generic {
            words[hit.start] = "estabelecimentos".into();
        }
        !generic
    });
    if qualified {
        for (at, word) in words.iter_mut().enumerate() {
            if word == "comercios" && !hits.iter().any(|hit| hit.start <= at && at < hit.end) {
                *word = "estabelecimentos".into();
            }
        }
    }
}

fn contact_alias(word: &str) -> Option<&'static str> {
    match word {
        "tel" | "teleofne" => Some("telefone"),
        "wpp" | "whats" => Some("whatsapp"),
        "cel" => Some("celular"),
        "emial" => Some("email"),
        _ => None,
    }
}

struct ContactPhrase {
    end: usize,
    name: usize,
    descriptor: Option<usize>,
    alias: Option<&'static str>,
}

impl ContactPhrase {
    fn phone(&self, words: &[String]) -> bool {
        PHONE_WORDS.contains(&self.alias.unwrap_or(&words[self.end - 1]))
    }
}

/// Read a contact noun phrase. Keep its span so a conjunction, qualification or
/// restatement immediately after it can be interpreted before normalization.
fn contact_phrase(words: &[String], start: usize) -> Option<ContactPhrase> {
    let start = start
        + usize::from(
            words.get(start).is_some_and(|word| matches!(word.as_str(), "um" | "uma" | "o" | "a" | "nenhum" | "nenhuma" | "tanto")),
        );
    let name = match words.get(start..)? {
        [noun, link, ..]
            if matches!(noun.as_str(), "numero" | "numeros" | "endereco" | "enderecos" | "no")
                && matches!(link.as_str(), "de" | "do" | "da") =>
        {
            start + 2
        }
        _ => start,
    };
    let (length, alias) = match words.get(name..)? {
        [word, ..] if PHONE_WORDS.contains(&word.as_str()) || matches!(word.as_str(), "email" | "emails") => (1, None),
        [word, ..] if contact_alias(word).is_some() => (1, contact_alias(word)),
        [prefix, word, ..] if prefix == "e" && matches!(word.as_str(), "mail" | "mails") => (2, None),
        [noun, qualifier, ..] if noun == "contato" && qualifier == "telefonico" => (2, Some("telefone")),
        [noun, link, verb, ..] if noun == "numero" && link == "para" && verb == "ligar" => (3, Some("telefone")),
        [noun, qualifier, ..] if matches!(noun.as_str(), "correio" | "endereco") && qualifier == "eletronico" => (2, Some("email")),
        _ => return None,
    };
    Some(ContactPhrase { end: name + length, name, descriptor: (name != start).then_some(start), alias })
}

fn contact_qualifier_end(words: &[String], end: usize) -> usize {
    match &words[end..] {
        [qualifier, ..]
            if matches!(
                qualifier.as_str(),
                "cadastrado" | "cadastrada" | "informado" | "informada" | "publicado" | "publicada" | "disponivel"
            ) =>
        {
            end + 1
        }
        [link, qualifier, ..]
            if matches!(link.as_str(), "de" | "para") && qualifier == "contato" || link == "no" && qualifier == "cadastro" =>
        {
            end + 2
        }
        _ => end,
    }
}

fn contact_predicate_end(words: &[String], verb: usize) -> Option<usize> {
    match words.get(verb)?.as_str() {
        "com" | "sem" | "tem" | "tenha" | "tenham" | "possui" | "possuem" | "informam" | "informaram" | "deixam" => Some(verb + 1),
        "dispoe" | "dispoem" if words.get(verb + 1).is_some_and(|word| word == "de") => Some(verb + 2),
        "conta" | "contam" if words.get(verb + 1).is_some_and(|word| word == "com") => Some(verb + 2),
        _ => None,
    }
}

/// Read contact clauses independently of the request operation. Keep word positions
/// aligned with the original text and refuse conditions the contact filter cannot express.
fn normalize_contact_filters(words: &mut [String], hits: &[Hit]) -> Result<(), Box<Interpretation>> {
    let covered = |start, end| {
        hits.iter().any(|hit| hit.start < end && start < hit.end && hit.values.iter().any(|entity| !matches!(entity, Entity::Uf(_))))
    };
    let choice = || clarify("Você quer filtrar por telefone, por e-mail ou exigir os dois?");
    for at in 0..words.len() {
        if let Some(contact) = contact_phrase(words, at)
            && !covered(at, contact.end)
        {
            let end = contact_qualifier_end(words, contact.end);
            if words.get(end).is_some_and(|word| word == "ou") {
                let next = end + 1 + usize::from(words.get(end + 1).is_some_and(|word| word == "que"));
                let name = contact_predicate_end(words, next).unwrap_or(next);
                if let Some(other) = contact_phrase(words, name)
                    && !covered(next, other.end)
                {
                    return Err(choice());
                }
            }
        }
    }
    for at in 0..words.len() {
        let (verb, negated) = if words[at] == "nao" { (at + 1, true) } else { (at, words[at] == "sem") };
        let Some(name) = contact_predicate_end(words, verb) else { continue };
        let Some(contact) = contact_phrase(words, name) else { continue };
        if covered(at, contact.end) {
            continue;
        }
        if negated {
            return Err(clarify(
                "O filtro de contato busca dados informados no cadastro. Você quer estabelecimentos com telefone, e-mail ou ambos?",
            ));
        }
        if matches!(words[verb].as_str(), "tem" | "tenha" | "tenham" | "possui" | "possuem")
            && !(at > 0 && (words[at - 1] == "que" || hits.iter().any(|hit| hit.end <= at)))
        {
            continue;
        }
        let after_contact = contact_qualifier_end(words, contact.end);
        let restatement = match &words[after_contact..] {
            [first, ..] if first == "digo" => 1,
            [first, second, ..] if matches!((first.as_str(), second.as_str()), ("isto", "e") | ("ou", "seja")) => 2,
            [first, second, third, ..] if first == "em" && second == "outras" && third == "palavras" => 3,
            _ => 0,
        };
        let both = words[name] == "tanto" && words.get(after_contact).is_some_and(|word| word == "quanto");
        if restatement > 0 || both {
            let next = after_contact + restatement.max(1);
            let other_name = contact_predicate_end(words, next).unwrap_or(next);
            if let Some(other) = contact_phrase(words, other_name)
                && !covered(after_contact, other.end)
            {
                if restatement > 0 && contact.phone(words) != other.phone(words) {
                    return Err(choice());
                }
                if both {
                    words[name] = "o".into();
                }
                words[after_contact..next].fill("e".into());
                if other_name == next {
                    words[next - 1] = "com".into();
                }
            }
        }
        if words[verb] != "com" {
            words[verb] = "possui".into();
        }
        if let Some(descriptor) = contact.descriptor {
            words[descriptor] = "contato".into();
        }
    }
    for at in 0..words.len() {
        if let Some(contact) = contact_phrase(words, at)
            && !covered(at, contact.end)
        {
            if let Some(alias) = contact.alias {
                words[contact.name..contact.end].fill("contato".into());
                words[contact.name] = alias.into();
            }
            if words.get(contact.end).is_some_and(|word| word == "disponivel") {
                words[contact.end] = "informado".into();
            }
        }
    }
    Ok(())
}

fn uf_id(word: &str) -> Option<UfId> {
    UFS.iter().position(|uf| uf.eq_ignore_ascii_case(word)).map(|index| UfId(index as u8))
}

/// A multiword typo is only tried at an activity boundary, then matched as a
/// complete alias. A partial sector cannot remain alongside the recovery.
fn recover_activities(words: &[String], hits: &mut Vec<Hit>, env: &ResolveContext<'_>, notes: &mut Vec<String>) {
    let first_name =
        hits.iter().find(|hit| hit.values.iter().any(|entity| !matches!(entity, Entity::Uf(_)))).map_or(words.len(), |hit| hit.start);
    let opening = env.delivery.record_opening(&words[..first_name]).0;
    let head = (opening..words.len())
        .find(|&at| !matches!(words[at].as_str(), "as" | "os" | "a" | "o" | "todas" | "todos" | "so" | "somente" | "apenas"));
    let editable = |word: &str| !env.vocabulary.knows(word);
    for start in 0..words.len() {
        let qualified = start >= 2
            && matches!(words[start - 1].as_str(), "de" | "do" | "da" | "dos" | "das")
            && (SECTOR_QUALIFIERS.contains(&words[start - 2].as_str())
                || matches!(words[start - 2].as_str(), "empresas" | "estabelecimentos" | "matrizes" | "filiais"));
        if head != Some(start) && !qualified {
            continue;
        }
        for end in (start + 2..=(start + 6).min(words.len())).rev() {
            let name = &words[start..end];
            if !name.iter().any(|word| editable(word))
                || hits.iter().any(|hit| {
                    hit.start < end
                        && start < hit.end
                        && (hit.start < start
                            || hit.end > end
                            || hit.start == start && hit.end == end
                            || hit.values.iter().any(|entity| !matches!(entity, Entity::Sector(_))))
                })
            {
                continue;
            }
            if let Some(values) = env.pack.lexicon.fuzzy_sector(name, editable) {
                hits.retain(|hit| hit.start >= end || hit.end <= start);
                hits.push(Hit { start, end, values, fuzzy: false });
                notes.push(format!("Atividade interpretada por aproximação: {}.", name.join(" ")));
                break;
            }
        }
    }
    for (start, word) in words.iter().enumerate() {
        if word.len() >= 7
            && editable(word)
            && !hits.iter().any(|hit| hit.start <= start && start < hit.end)
            && let Some(values) = env.pack.lexicon.fuzzy_sector(&words[start..start + 1], editable)
        {
            hits.push(Hit { start, end: start + 1, values, fuzzy: false });
            notes.push(format!("Atividade interpretada por aproximação: {word}."));
        }
    }
}

/// Require a geographic introducer and a full place name. Multiword recovery also
/// requires an adjacent UF, so an incomplete name cannot fall back to a short prefix.
fn recover_places(words: &[String], hits: &mut Vec<Hit>, env: &ResolveContext<'_>, notes: &mut Vec<String>) {
    for at in 1..words.len() {
        let previous = words[at - 1].as_str();
        let locative = matches!(previous, "em" | "no" | "na");
        let coordinated = previous == "e"
            && hits.iter().any(|hit| {
                hit.values.iter().any(|entity| matches!(entity, Entity::Municipality { .. }))
                    && (hit.end + 1 == at || hit.end + 2 == at && uf_id(&words[hit.end]).is_some())
            });
        if !locative && !coordinated && !matches!(previous, "de" | "do" | "da") {
            continue;
        }
        let start = if locative
            && PLACE_QUALIFIERS.contains(&words[at].as_str())
            && words.get(at + 1).is_some_and(|word| matches!(word.as_str(), "de" | "do" | "da"))
        {
            at + 2
        } else {
            at
        };
        let explicit = (start + 1..(start + 7).min(words.len())).find_map(|end| uf_id(&words[end]).map(|uf| (end, Some(uf))));
        let Some((end, uf)) = explicit.or_else(|| {
            (locative && start + 1 == words.len() && words[start].len() >= 7 && !env.vocabulary.knows(&words[start]))
                .then_some((start + 1, None))
        }) else {
            continue;
        };
        if hits.iter().any(|hit| {
            hit.start < end
                && start < hit.end
                && (hit.start < start
                    || hit.end > end
                    || hit.start == start && hit.end == end
                    || hit.values.iter().any(|entity| matches!(entity, Entity::Sector(_) | Entity::Region(_))))
        }) {
            continue;
        }
        let name = &words[start..end];
        if let Some(values) = env.pack.lexicon.fuzzy_place(name, uf) {
            hits.retain(|hit| hit.start >= end || hit.end <= start);
            hits.push(Hit { start, end, values, fuzzy: true });
            notes.push(format!("Local interpretado por aproximação: {}.", name.join(" ")));
        }
    }
}

/// `export` marks a question whose answer is delivered as a file: a count of companies
/// becomes the list of those companies.
pub(crate) fn resolve(env: &ResolveContext<'_>, text: &str, state: &Conversation, export: bool) -> Result<Frame, Box<Interpretation>> {
    let ResolveContext { pack, cues, policy, anchor, current, competences, versions, population, vocabulary, delivery } = *env;
    let raw = normalize::fold(text);
    let mut words = normalize::words(text);
    let mut normalized = words.join(" ");
    if cue(&normalized, &policy.privacy) {
        return Err(reject(
            "A pesquisa de pessoas, CPFs e sócios não está disponível no chat. Posso pesquisar estabelecimentos por município e atividade.",
        ));
    }
    let mut preliminary = pack.lexicon.scan(&words);
    // A complete sector alias can contain the name of an unavailable metric.
    // "Empresas de produção de soja" names an activity. "Qual a produção de soja" asks for output.
    let unavailable = policy.unavailable.iter().filter(|phrase| has(&normalized, &[phrase])).any(|phrase| {
        let needle = normalize::words(phrase);
        !needle.is_empty()
            && words.windows(needle.len()).enumerate().any(|(at, window)| {
                window == needle
                    && !preliminary.iter().any(|hit| {
                        hit.start <= at
                            && at + needle.len() <= hit.end
                            && hit.values.iter().any(|entity| matches!(entity, Entity::Sector(_)))
                    })
            })
    });
    if unavailable {
        return Err(reject(
            "Essa informação não está nos dados cadastrais disponíveis. Posso contar estabelecimentos, listar CNPJs ou mostrar registros e baixas.",
        ));
    }
    let mut approximated = Vec::new();
    recover_places(&words, &mut preliminary, env, &mut approximated);
    recover_activities(&words, &mut preliminary, env, &mut approximated);
    preliminary.sort_by_key(|h| h.start);
    // A normalized opener such as "dá pra" can contain the word "para". Ignore
    // UF readings entirely inside a recognized opener, never a named entity.
    let first_name = preliminary
        .iter()
        .find(|hit| hit.values.iter().any(|entity| !matches!(entity, Entity::Uf(_))))
        .map_or(words.len(), |hit| hit.start);
    let (mut opening, mut list_command) = delivery.record_opening(&words[..first_name]);
    preliminary.retain(|hit| hit.end > opening);
    let first_comma = text.split_once(',').map(|(prefix, _)| normalize::words(prefix).len());
    if let Some(end) = first_comma {
        let places = preliminary.iter().take_while(|hit| hit.end <= end);
        if end > 0
            && places.clone().all(|hit| hit.values.iter().all(|entity| matches!(entity, Entity::Municipality { .. } | Entity::Uf(_))))
            && places.map(|hit| hit.end - hit.start).sum::<usize>() == end
        {
            let next_name = preliminary
                .iter()
                .find(|hit| hit.start >= end && hit.values.iter().any(|entity| !matches!(entity, Entity::Uf(_))))
                .map_or(words.len(), |hit| hit.start);
            let (after_place, command) = delivery.record_opening(&words[end..next_name]);
            if after_place > 0 {
                opening = end + after_place;
                list_command = command;
            }
        }
    }
    // A place name before "em" followed immediately by another municipality is not
    // a coordinated list. "Flores em Fortaleza CE" can also mean flowers in Fortaleza.
    if let [first, second, ..] = preliminary.as_slice()
        && first.start == 0
        && first.end + 1 == second.start
        && words[first.end] == "em"
        && [first, second].iter().all(|hit| hit.values.iter().any(|entity| matches!(entity, Entity::Municipality { .. })))
        && first_comma.is_none_or(|end| end >= second.end)
    {
        return Err(clarify("Qual município você quer pesquisar? Separe locais diferentes por “e” ou por vírgula."));
    }
    let closing = delivery.record_closing(&words, words.len()).max(preliminary.last().map_or(opening, |hit| hit.end));
    if words.iter().any(|word| {
        matches!(word.as_str(), "area" | "atividade" | "ramo" | "setor" | "segmento" | "comercios")
            || PHONE_WORDS.contains(&word.as_str())
            || contact_alias(word).is_some()
            || matches!(word.as_str(), "email" | "emails" | "mail" | "mails" | "eletronico" | "telefonico" | "ligar")
    }) {
        normalize_sector_qualifier(&mut words, &mut preliminary);
        normalize_contact_filters(&mut words, &preliminary)?;
    }
    for hit in &preliminary {
        if hit.start >= 2
            && hit.values.iter().any(|entity| matches!(entity, Entity::Municipality { .. } | Entity::Uf(_)))
            && words[hit.start - 2] == "la"
            && words[hit.start - 1] == "em"
        {
            words[hit.start - 2] = "e".into();
        }
    }
    normalized = words.join(" ");
    let n = normalized.as_str();
    let masked = masked_words(&words, &preliminary);
    let body = masked[opening..].join(" ");
    let m = body.as_str();
    if has(m, &["matrizes ou filiais", "filiais ou matrizes", "matriz ou filial", "filial ou matriz"])
        || has(m, &["capital social", "capital acima"])
            && has(m, &["ou epp", "ou pequeno porte", "ou microempresa", "ou mei", "ou simples"])
    {
        return Err(clarify("Esses filtros são alternativas? Informe qual condição quer aplicar ou use “e” para combinar filtros."));
    }
    // An opener can be the first word of a list cue, as in "mostre as matrizes".
    let list_cue = cue(m, &cues.list) || opening > 0 && cue(&masked.join(" "), &cues.list);
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
    let size_class = has(n, &["grande porte", "medio porte", "medio e grande porte", "grande e medio porte"]);
    let quality: Vec<String> = policy
        .ambiguous_quality
        .iter()
        .filter(|phrase| !(size_class && matches!(phrase.as_str(), "grande" | "grandes")))
        .cloned()
        .collect();
    if uncovered(&quality) {
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
                "comercios",
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
                "cooperativa",
                "cooperativas",
                "associacoes",
                "fundacoes",
                "sociedades anonimas",
                "sociedade anonima",
                "eireli",
                "ltda",
            ],
        );
    // "preciso prospectar clientes para minha distribuidora": a prospecting request that
    // names no kind of company to look for.
    let prospecting = has(n, &["prospectar", "prospeccao", "leads", "prospects"])
        || has(n, &["clientes"]) && has(n, &["encontrar", "achar", "buscar", "procurar", "preciso", "quero", "novos", "potenciais"]);
    // The activity after "para minha/meu" is the asker's own business, not the companies sought.
    let own_business = has(n, &["para minha", "para meu", "para a minha", "para o meu", "para nossa", "para nosso"]);
    if state.last.is_none() && prospecting && (!domain_evidence || own_business) && lookup(&raw).is_none() {
        return Err(clarify(
            "Que tipo de empresa você quer encontrar? Diga a atividade e o local, por exemplo: “restaurantes em Cuiabá com telefone”.",
        ));
    }
    if has(n, &["lojas", "loja"]) && !preliminary.iter().any(|h| h.values.iter().any(|e| matches!(e, Entity::Sector(_)))) {
        return Err(clarify("Que tipo de loja você quer pesquisar? Informe a atividade para definir os CNAEs."));
    }
    if !domain_evidence && state.last.is_none() && meta_topic(m).is_none() && lookup(&raw).is_none() {
        // "liste os fornecedores de embalagens em Cuiabá": a list of something in a place,
        // named by words no catalogue holds. Ask about them.
        let strange: Vec<&str> = words
            .iter()
            .enumerate()
            .filter(|(at, w)| *at >= opening && !preliminary.iter().any(|h| h.start <= *at && *at < h.end) && !vocabulary.knows(w))
            .map(|(_, w)| w.as_str())
            .collect();
        let placed = preliminary.iter().any(|h| h.values.iter().any(|e| matches!(e, Entity::Municipality { .. } | Entity::Uf(_))));
        if (list_command || list_cue) && placed && !strange.is_empty() {
            return Err(clarify(&unrecognized(&strange)));
        }
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
    let mut notes = approximated;
    let mut rows = None;
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
            if matches!(&words[at..], [todo, article, state, ..] if todo == "todo" && article == "o" && state == "estado") {
                at += 2;
            }
            let first = words.get(at).map(String::as_str).unwrap_or("");
            let sector = SECTOR_QUALIFIERS.contains(&first) || first == "setores";
            let labels = if sector { SECTOR_QUALIFIERS } else { PLACE_QUALIFIERS };
            let repeated = |at: usize| {
                at > i + 1
                    && labels.contains(&first)
                    && matches!(words.get(at..), Some([prep, noun, ..])
                        if matches!(prep.as_str(), "em" | "no" | "na") && labels.contains(&noun.as_str()))
            };
            let mut restated = false;
            while let Some(w) = words.get(at) {
                if repeated(at) {
                    restated = true;
                } else if !(PLACE_QUALIFIERS.contains(&w.as_str())
                    || SECTOR_QUALIFIERS.contains(&w.as_str())
                    || matches!(w.as_str(), "setores" | "de" | "do" | "da"))
                {
                    break;
                }
                at += 1;
            }
            if let Some(next) = words.get(at) {
                let recognized = hits.iter().any(|h| {
                    h.start <= at
                        && at < h.end
                        && (!restated
                            || h.values.iter().any(|entity| {
                                if sector {
                                    matches!(entity, Entity::Sector(_))
                                } else {
                                    matches!(entity, Entity::Municipality { .. } | Entity::Uf(_))
                                }
                            }))
                });
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
                if !recognized && (restated || !grammatical && !numeric && !month) {
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
        if matches!(
            word.as_str(),
            "frota" | "bairro" | "bairros" | "cep" | "ddd" | "rua" | "avenida" | "logradouro" | "certificacao" | "licenciamento"
        ) {
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
    // Starts of the hits already read as places, for coordination ("Sinop e Sorriso").
    let mut placed: Vec<usize> = Vec::new();
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
        // A place coordinated with the place before it shares its role.
        let coordinated = hits.iter().any(|h| {
            placed.contains(&h.start) && (h.end == hit.start || h.end + 1 == hit.start && matches!(words[h.end].as_str(), "e" | "ou"))
        });
        let geographic = (coordinated
            || has(&before, &["cidade", "municipio", "em", "no", "na", "sem", "exceto", "menos", "vs", "quis dizer"]))
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
            placed.push(hit.start);
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
                    if neg {
                        excluded_ufs.push(*id)
                    } else {
                        ufs.push(*id)
                    }
                    placed.push(hit.start);
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
    // "dentistas em Cuiabá com telefone", "empresas abertas em 2025 em Sinop": a noun phrase
    // naming an activity or companies, as typed into a search box. It starts a new query when
    // it also names the place, or when there is no conversation to refine. Every other word
    // of it must be known (see the closed-vocabulary check at the end).
    let company_noun = has(
        n,
        &[
            "empresas",
            "empresa",
            "estabelecimentos",
            "estabelecimento",
            "cnpjs",
            "negocios",
            "mei",
            "microempresas",
            "cooperativas",
            "associacoes",
            "fundacoes",
            "sociedades anonimas",
        ],
    );
    let named_place = !municipalities.is_empty() || !ufs.is_empty() || !regions.is_empty();
    let covered = |at: usize| hits.iter().any(|h| h.start <= at && at < h.end);
    let plural_noun = |w: &str| {
        matches!(
            w,
            "empresas"
                | "estabelecimentos"
                | "negocios"
                | "cnpjs"
                | "firmas"
                | "companhias"
                | "microempresas"
                | "cooperativas"
                | "associacoes"
                | "fundacoes"
                | "sociedades"
        )
    };
    // The phrase is headed by what is searched for, after articles and a plain request.
    let head = words[opening..]
        .iter()
        .position(|w| {
            !matches!(
                w.as_str(),
                "a" | "o"
                    | "as"
                    | "os"
                    | "um"
                    | "uma"
                    | "uns"
                    | "umas"
                    | "todas"
                    | "todos"
                    | "me"
                    | "apenas"
                    | "somente"
                    | "so"
                    | "novas"
                    | "novos"
                    | "quero"
                    | "queria"
                    | "preciso"
                    | "procuro"
                    | "busco"
                    | "de"
                    | "das"
                    | "dos"
                    | "concorrentes"
                    | "concorrente"
                    | "contatos"
                    | "telefones"
                    | "emails"
                    | "leads"
                    | "prospects"
                    | "vender"
                    | "prospectar"
                    | "encontrar"
                    | "achar"
                    | "para"
                    | "clientes"
                    | "gostaria"
                    | "quais"
                    | "sao"
            )
        })
        .map(|at| at + opening);
    // A row limit can come between a hand-over command and the target noun.
    let leading_limit = head.filter(|_| list_command || list_cue || export).and_then(|at| {
        let (quantity, end) = time::quantity(&words, at)?;
        let target = words.get(end)?;
        let sector = hits.iter().any(|h| h.start == end && h.values.iter().any(|e| matches!(e, Entity::Sector(_))));
        (quantity <= 1_000_000
            && !(1900..=2100).contains(&quantity)
            && (plural_noun(target) || sector || matches!(target.as_str(), "matrizes" | "filiais")))
        .then_some((quantity as u32, at..end))
    });
    let head = leading_limit.as_ref().map(|(_, span)| span.end).or(head);
    let tokens = normalize::tokens(text);
    let tail = delivery.record_closing(&tokens, tokens.len());
    let trailing_limit = (export || list_command || list_cue)
        .then(|| {
            (tail.saturating_sub(2).max(1)..tail).find_map(|at| {
                if !matches!(tokens[at - 1].as_str(), "so" | "apenas" | "somente") {
                    return None;
                }
                let (limit, end) = time::quantity(&tokens, at)?;
                (end == tail && limit <= 1_000_000 && !(1900..=2100).contains(&limit)).then_some((limit as u32, at..end))
            })
        })
        .flatten();
    let headed_by_sector =
        head.is_some_and(|at| hits.iter().any(|h| h.start == at && h.values.iter().any(|e| matches!(e, Entity::Sector(_)))));
    let headed_by_companies = head.is_some_and(|at| {
        plural_noun(&words[at]) || words[at] == "mei" || list_command && matches!(words[at].as_str(), "matrizes" | "filiais")
    });
    // Words before the head are the request itself; the rest must be read by a filter.
    let foreign: Vec<&str> = words
        .iter()
        .enumerate()
        .filter(|(at, w)| head.is_some_and(|head| *at >= head) && *at < closing && !covered(*at) && !vocabulary.searches(w))
        .map(|(_, w)| w.as_str())
        .collect();
    let search_shape = (headed_by_sector || headed_by_companies) && (named_place || state.last.is_none());
    let search_style = search_shape && foreign.is_empty();
    let rank_cue = cue(m, &cues.rank);
    let rank_intent = rank_cue && has(n, &["mais", "menos", "maior", "menor", "variacao", "top", "ranking"]);
    let analytical_intent =
        rank_intent || [&cues.count, &cues.compare, &cues.trend, &cues.breakdown, &cues.growth].into_iter().any(|phrases| cue(m, phrases));
    let analytical_cue = analytical_intent || rank_cue;
    // Imperatives select a list only when no explicit analytical operation was requested.
    let search_list = search_style
        && !analytical_intent
        && (list_command || has(n, &["quero todos", "quero todas"]) || headed_by_sector && has(m, &["quais"]));
    // Contacts and leads are asked for as a list of companies.
    let wants_contacts = (has(n, &["contatos", "telefones", "emails", "e mails", "leads", "prospects", "mailing"])
        || search_style && has(n, &["telefone", "fone", "celular", "whatsapp", "zap", "email", "e mail", "contato"]))
        && !analytical_intent;
    // "os maiores supermercados de Sinop": companies ordered by the only size the registry
    // holds, the declared capital. "As maiores cidades" ranks a dimension instead.
    let biggest = has(n, &["maiores"]) && dimension(n).is_none() && (!sectors.is_empty() || company_noun);
    let full_query = bare_topic
        || search_style
        || search_shape && list_command && has(m, &["capital social"])
        || wants_contacts
        || analytical_cue
        || list_cue
        || meta.is_some()
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
    let states_status = has(
        n,
        &[
            "ativas",
            "ativos",
            "ativa",
            "ativo",
            "inativas",
            "inativos",
            "baixadas",
            "baixados",
            "fechadas",
            "fechados",
            "encerradas",
            "encerrados",
            "suspensas",
            "suspensos",
            "inaptas",
            "inaptos",
            "abertas",
            "abertos",
            "novas",
            "novos",
        ],
    );
    let period = time::period(&temporal_raw, anchor);
    let closed = cue(m, &cues.closures) || period.is_some() && cue(m, &cues.dated_closures);
    let explicit_intent =
        search_list || analytical_cue || list_cue || closed || wants_contacts || biggest || meta.is_some() || lookup(&raw).is_some();
    let assumed_stock = search_style && !bare_topic && !explicit_intent && !states_status && !followup && !export;
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
    // "empresas de energia solar em Sinop": the search names companies by a word no catalogue
    // holds. Ask about it; a sector followed by other words may be a company name instead.
    if search_shape && headed_by_companies && !full_query && !followup && meta.is_none() {
        return Err(clarify(&unrecognized(&foreign)));
    }
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
    filters(n, m, &raw, &mut request, &mut act, population, &mut notes)?;
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
        let mut temporal = period;
        if temporal.is_some() && time::relative(&temporal_raw) {
            notes.push(format!("Períodos relativos contam até a data de referência da base, {}.", date_br(anchor)));
        }
        // "novas empresas", "recém-abertas" with no period: the last ninety days of the data.
        if temporal.is_none()
            && !closed
            // Masked text: "Currais Novos" is a place; "mais novos" orders a list.
            && has(m, &["novas", "novos", "recem abertas", "recem abertos", "recem criadas", "recem criados", "recem registradas"])
            && !has(m, &["mais novas", "mais novos"])
        {
            let from = Day(anchor.0.saturating_sub(89));
            temporal = Some(Period::Days(Range { from: Some(from), to: Some(anchor) }));
            notes.push(format!(
                "Considerei como novas as empresas abertas nos últimos 90 dias, de {} a {}.",
                date_br(from),
                date_br(anchor)
            ));
        }
        let capital_floor = has(n, &["capital social a partir de", "capital a partir de"]) && request.scope.capital.is_some();
        if temporal.is_none() && (has(n, &["ultimos", "ultimas", "desde"]) || has(n, &["a partir"]) && !capital_floor) {
            return Err(clarify("Não consegui resolver o período. Informe datas válidas ou uma quantidade de meses ou dias."));
        }
        let registered = (cue(m, &cues.registrations)
            && (temporal.is_some()
                || has(n, &["abriram", "surgiram", "comecaram", "criadas", "criados", "foram registrados", "registros de inicio"])))
            || temporal.is_some() && has(m, &["abertas", "abertos", "novas", "novos", "recem"]);
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
        let isrank = rank_intent;
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
        let islist =
            (search_list || list_cue || wants_contacts || biggest || has(n, &["quais"]) && has(n, &["maior capital", "mais capital"]))
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
            let explicit = leading_limit
                .as_ref()
                .map(|(limit, _)| *limit)
                .or_else(|| trailing_limit.as_ref().map(|(limit, _)| *limit))
                .or_else(|| list_limit(n));
            // An exported list shows one page and writes the requested rows to the file.
            // Above 100 rows a chat list is refused with a clamped option, never truncated.
            let limit = match explicit {
                Some(rows) if export => rows.min(100) as u16,
                Some(rows) => u16::try_from(rows).unwrap_or(u16::MAX),
                None => 20,
            };
            rows = explicit.filter(|_| export);
            let order = if biggest || has(n, &["capital social", "capital"]) && has(n, &["maior", "mais", "ordenados"]) {
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
            if biggest {
                notes.push("Ordenei pelo capital social declarado, o único indicador de tamanho do cadastro.".into());
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
    if assumed_stock && matches!(request.op, Op::Count { metric: Metric::Stock }) && request.scope.opened.is_none() {
        notes.insert(0, "Considerei o estoque de estabelecimentos ativos.".into());
    }
    if hits.iter().any(|h| h.fuzzy) {
        notes.push("Confirme o município interpretado por aproximação.".into());
    }
    // Closed vocabulary: a word that is neither a catalogue entity nor known to the grammar
    // names something no filter applied. Dropping it would answer a broader question than
    // the one asked ("empresas de energia solar" answered with every company).
    if !matches!(request.op, Op::Lookup { .. } | Op::Meta { .. }) {
        // A preamble closed by a comma or colon ("Para vender meu software, quantas…") sets
        // the scene and is not part of the question.
        let question_start = preamble_end(text).max(opening);
        // An explicit CNAE code pins the activity; words after it describe the code.
        let coded = matches!(request.scope.activity, ActivityFilter::Cnae { .. });
        let unknown: Vec<&str> = words
            .iter()
            .enumerate()
            .filter(|(i, word)| (question_start..closing).contains(i) && !covered(*i) && !vocabulary.knows(word))
            .map(|(_, word)| word.as_str())
            .collect();
        if !unknown.is_empty() && !coded {
            return Err(clarify(&unrecognized(&unknown)));
        }
        let is_list = matches!(request.op, Op::List { .. });
        let leading = leading_limit.as_ref().map(|(_, span)| span).filter(|_| is_list);
        let trailing = trailing_limit.as_ref().map(|(_, span)| span).filter(|_| is_list);
        if let Some(number) = unexplained_number(&tokens, &words, &hits, leading, trailing) {
            return Err(clarify(&format!(
                "Não consegui aplicar o número “{number}” a um filtro. Diga o que ele representa: ano, capital social ou quantidade de linhas."
            )));
        }
    }
    Ok(Frame { request, act, notes, rows })
}

fn date_br(day: Day) -> String {
    day.to_ymd().map_or_else(String::new, |(year, month, day)| format!("{day:02}/{month:02}/{year}"))
}

/// The question for words outside the closed vocabulary. It starts like the other
/// "unrecognized" questions, which a company-name search may still answer.
fn unrecognized(unknown: &[&str]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for word in unknown {
        if !seen.contains(word) {
            seen.push(word);
        }
    }
    let quoted = seen.iter().take(4).map(|word| format!("“{word}”")).collect::<Vec<_>>().join(", ");
    format!("Não reconheci {quoted}. Diga a atividade ou o filtro com outras palavras, ou informe o código CNAE.")
}

/// Index, in [`normalize::words`], of the first word after a leading preamble: a clause that
/// ends in a comma or colon and precedes the question word or request verb.
fn preamble_end(text: &str) -> usize {
    let mut at = 0;
    let mut end = 0;
    for token in text.split_whitespace() {
        let parts = normalize::words(token);
        let opens = parts.first().is_some_and(|w| {
            matches!(
                w.as_str(),
                "quantos"
                    | "quantas"
                    | "qual"
                    | "quais"
                    | "liste"
                    | "listar"
                    | "conte"
                    | "compare"
                    | "mostre"
                    | "traga"
                    | "distribua"
                    | "informe"
            )
        });
        if opens {
            return end;
        }
        at += parts.len();
        if token.ends_with([',', ':']) {
            end = at;
        }
    }
    0
}

/// A number no rule reads: neither a date, a row count, an amount, a population, a code nor
/// part of a catalogue name. Like an unknown word, it must not be dropped.
fn unexplained_number<'a>(
    tokens: &'a [String],
    words: &[String],
    hits: &[Hit],
    leading: Option<&std::ops::Range<usize>>,
    trailing: Option<&std::ops::Range<usize>>,
) -> Option<&'a str> {
    // Digits inside entity names ("Dois Irmãos", "4 de Julho") are part of the entity.
    let entity_words: Vec<&str> = hits.iter().flat_map(|h| words[h.start..h.end.min(words.len())].iter().map(String::as_str)).collect();
    let near = |at: usize, back: usize, ahead: usize, list: &[&str]| {
        tokens[at.saturating_sub(back)..(at + 1 + ahead).min(tokens.len())].iter().any(|t| list.contains(&t.as_str()))
    };
    let mut word_offset = 0;
    for (at, token) in tokens.iter().enumerate() {
        let start = word_offset;
        word_offset += normalize::word_parts(token).count();
        if !token.chars().any(|c| c.is_ascii_digit())
            || entity_words.contains(&token.as_str())
            || leading.is_some_and(|span| span.contains(&start))
            || trailing.is_some_and(|span| span.contains(&at))
        {
            continue;
        }
        let digits: String = token.chars().filter(char::is_ascii_digit).collect();
        let plain = token.chars().all(|c| c.is_ascii_digit() || c == '.');
        let year = plain && digits.len() == 4 && digits.parse::<u16>().is_ok_and(|y| (1900..=2100).contains(&y));
        let explained = year
            || time::date_shaped(token)
            || !time::competences(token).is_empty()
            // CNPJ, CNPJ root and other long identifiers are validated by the lookup rule.
            || digits.len() >= 8
            || token.contains('/')
            || at == 0
            || near(at, 2, 0, &["ultimos", "ultimas", "top", "liste", "listar", "traga", "tragam", "traz", "trazem", "os", "as", "das", "dos", "lista", "listagem", "relacao", "primeiros", "primeiras", "pagina", "entre", "e", "a", "favor"])
            || near(at, 0, 2, &["dias", "meses", "anos", "semanas", "trimestre", "tri", "semestre", "km", "passos", "pagina", "primeiros", "primeiras", "maiores", "linhas", "resultados", "habitantes", "hab", "reais", "mil", "milhao", "milhoes", "%", "cidades", "municipios", "estados", "setores", "empresas", "estabelecimentos", "cnpjs"])
            || tokens.get(at + 1).is_some_and(|next| entity_words.contains(&next.as_str()))
            || near(at, 3, 0, &["individual", "limitada", "ltda", "anonima", "fechada", "juridica", "ciclo", "expansao", "rodada", "etapa", "fase", "lote"])
            || near(at, 4, 0, &["base", "minima", "minimo", "inicial"])
            || near(at, 8, 0, &["capital", "habitantes"])
            || near(at, 0, 4, &["habitantes", "hab", "capital"])
            || near(at, 4, 0, &["cnae", "cnaes", "subclasse", "subclasses", "classe", "grupo", "divisao", "codigo", "codigos", "secao", "natureza", "situacao", "status", "porte", "cnpj", "raiz", "registro"])
            || token.ends_with('%')
            || token.ends_with('k');
        if !explained {
            return Some(token);
        }
    }
    None
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
fn masked_words(words: &[String], hits: &[Hit]) -> Vec<String> {
    let mut masked = words.to_vec();
    for hit in hits {
        let end = hit.end.min(masked.len());
        for word in &mut masked[hit.start.min(end)..end] {
            *word = "entidade".into();
        }
    }
    masked
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

/// Read the amount from the capital clause itself. A later opening date or
/// page number cannot supply a missing monetary value.
fn read_social_capital(tokens: &[String]) -> Result<MoneyRange, Box<Interpretation>> {
    enum Bound {
        Minimum,
        Above,
        Maximum,
        Below,
        Equal,
        Between,
    }

    let tokens = if tokens.first().is_some_and(|word| word == "de") { &tokens[1..] } else { tokens };
    let (prefix, bound) = [
        (&["entre"][..], Bound::Between),
        (&["acima", "de"][..], Bound::Above),
        (&["mais", "de"][..], Bound::Above),
        (&["maior", "que"][..], Bound::Above),
        (&["superior", "a"][..], Bound::Above),
        (&["abaixo", "de"][..], Bound::Below),
        (&["menos", "de"][..], Bound::Below),
        (&["menor", "que"][..], Bound::Below),
        (&["inferior", "a"][..], Bound::Below),
        (&["a", "partir", "de"][..], Bound::Minimum),
        (&["pelo", "menos"][..], Bound::Minimum),
        (&["no", "minimo"][..], Bound::Minimum),
        (&["ate"][..], Bound::Maximum),
        (&["no", "maximo"][..], Bound::Maximum),
        (&["igual", "a"][..], Bound::Equal),
    ]
    .into_iter()
    .find(|(phrase, _)| tokens.get(..phrase.len()).is_some_and(|words| words.iter().zip(*phrase).all(|(word, part)| word == part)))
    .unwrap_or((&[], Bound::Minimum));
    let amount = |at: usize| {
        // Folding leaves the currency symbol "R$" as the token "r".
        let at = at + usize::from(tokens.get(at).is_some_and(|word| word == "r"));
        time::money_quantity(tokens, at)
            .filter(|(_, end)| tokens.get(*end).is_none_or(|next| !matches!(next.as_str(), "anos" | "meses" | "dias" | "habitantes")))
    };
    let (value, end) =
        amount(prefix.len()).ok_or_else(|| clarify("Qual o valor ou intervalo de capital social? Informe o valor em reais."))?;
    let (from, to) = match bound {
        Bound::Minimum => (Some(value), None),
        Bound::Above => (Some(value.saturating_add(1)), None),
        Bound::Maximum => (None, Some(value)),
        Bound::Below => (None, Some(value.saturating_sub(1))),
        Bound::Equal => (Some(value), Some(value)),
        Bound::Between => {
            let end = end + usize::from(tokens.get(end).is_some_and(|word| matches!(word.as_str(), "real" | "reais")));
            let upper = tokens.get(end).filter(|word| *word == "e").and_then(|_| amount(end + 1));
            let (upper, _) = upper.ok_or_else(|| clarify("Informe os dois limites de capital social."))?;
            (Some(value), Some(upper))
        }
    };
    Ok(Range { from, to })
}

fn filters(
    n: &str,
    m: &str,
    raw: &str,
    r: &mut Request,
    act: &mut String,
    population: PopulationRef,
    notes: &mut Vec<String>,
) -> Result<(), Box<Interpretation>> {
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
                    "nao e mei",
                    "sem ser mei",
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
        if has(n, &["medio porte", "grande porte", "medio e grande porte", "grande e medio porte", "medias empresas"]) {
            r.scope.porte = PorteSet::only(Porte::Other);
            notes.push(
                "A Receita Federal classifica o porte em microempresa, pequeno porte e demais; considerei “demais”, que reúne médias e grandes."
                    .into(),
            );
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
    let phone = has(n, PHONE_WORDS);
    let email = has(n, EMAIL_WORDS);
    if phone || email {
        r.scope.contact = match (phone, email) {
            (true, true) => ContactNeed::Both,
            (true, false) => ContactNeed::Phone,
            _ => ContactNeed::Email,
        };
    }
    if has(n, &["whatsapp", "zap"]) {
        notes.push("O cadastro não distingue WhatsApp: considerei os estabelecimentos com telefone informado.".into());
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
    if has(n, &["sociedade anonima", "sociedades anonimas"]) || t.iter().any(|token| matches!(token.as_str(), "s/a" | "s.a")) {
        r.scope.natureza = SmallSet::new(if has(n, &["capital aberto"]) {
            vec![NaturezaCode::Code(2046)]
        } else if has(n, &["capital fechado"]) {
            vec![NaturezaCode::Code(2054)]
        } else {
            vec![NaturezaCode::Code(2046), NaturezaCode::Code(2054)]
        });
    }
    if has(n, &["sociedade anonima fechada"]) {
        r.scope.natureza = SmallSet::new([NaturezaCode::Code(2054)]);
    }
    for (phrases, codes) in [
        (&["cooperativa", "cooperativas"][..], &[2143][..]),
        (&["eireli"][..], &[2305, 2313][..]),
        (&["associacao", "associacoes"][..], &[3999][..]),
        (&["fundacao", "fundacoes"][..], &[3069][..]),
    ] {
        if has(n, phrases) {
            r.scope.natureza = SmallSet::new(codes.iter().map(|code| NaturezaCode::Code(*code)));
        }
    }
    if has(n, &["grupo de natureza juridica 2", "entidades empresariais"]) {
        r.scope.natureza = SmallSet::new([NaturezaCode::Group(2)]);
    }
    if let Some(at) = (1..t.len()).find(|&i| {
        t[i] == "social"
            && t[i - 1] == "capital"
            && i.checked_sub(2).is_none_or(|prev| !matches!(t[prev].as_str(), "maior" | "mais" | "por"))
    }) {
        r.scope.capital = Some(read_social_capital(&t[at + 1..])?);
    }
    // "capital acima de 100 mil": an amount right after "capital" bounds the declared capital.
    if r.scope.capital.is_none()
        && !has(n, &["capital social"])
        && let Some(at) =
            (0..t.len()).find(|&i| t[i] == "capital" && (i + 1..(i + 6).min(t.len())).any(|j| time::money_quantity(&t, j).is_some()))
    {
        let amounts: Vec<(usize, u64)> =
            (at + 1..(at + 9).min(t.len())).filter_map(|i| time::money_quantity(&t, i).map(|(value, _)| (i, value))).collect();
        let (first_at, first) = amounts[0];
        let between = &t[at + 1..first_at];
        let said = |list: &[&str]| between.iter().any(|w| list.contains(&w.as_str()));
        let range = if said(&["entre"]) {
            let second = amounts.get(1).ok_or_else(|| clarify("Informe os dois limites de capital social."))?.1;
            Range { from: Some(first.min(second)), to: Some(first.max(second)) }
        } else if said(&["pelo", "minimo", "partir"]) {
            Range { from: Some(first), to: None }
        } else if said(&["ate", "maximo"]) {
            Range { from: None, to: Some(first) }
        } else if said(&["acima", "maior", "superior", "mais"]) {
            Range { from: Some(first.saturating_add(1)), to: None }
        } else if said(&["abaixo", "menor", "inferior", "menos"]) {
            Range { from: None, to: Some(first.saturating_sub(1)) }
        } else {
            Range { from: Some(first), to: None }
        };
        r.scope.capital = Some(range);
    }
    // "com mais de 10 anos": time since the start of activity, counted back from the data date.
    for i in 3..t.len() {
        if t[i] == "anos"
            && t[i - 2] == "de"
            && matches!(t[i - 3].as_str(), "mais" | "menos")
            && !t[i.saturating_sub(5)..i].iter().any(|w| matches!(w.as_str(), "ultimos" | "ultimas"))
            && let Some(years) = time::number(&t[i - 1]).and_then(|years| u16::try_from(years).ok())
            && let Some((year, month, day)) = r.time.anchor.to_ymd()
            && let Some(cutoff) = year.checked_sub(years).and_then(|y| Day::from_ymd(y, month, day.min(28)))
        {
            r.scope.opened = Some(if t[i - 3] == "mais" {
                Range { from: None, to: Some(cutoff) }
            } else {
                Range { from: Some(Day(cutoff.0 + 1)), to: Some(r.time.anchor) }
            });
            notes.push(format!("Contei a idade pelo início de atividade até {}.", date_br(r.time.anchor)));
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
