//! Deterministic Brazilian Portuguese interpretation with selective, validated execution.

pub mod context;
pub mod grammar;
pub mod lang;
pub mod normalize;
pub mod policy;
mod skills;
pub mod spotter;
pub mod time;
mod vocabulary;

use context::Conversation;
use lang::LanguagePack;
use opencnpj_query::{
    ir::*,
    validate::{self, Capabilities, IssueKind},
};
use policy::{CompanySearch, Decision, Interpretation};
use spotter::Entity;

/// An immutable catalogue pack and the exact capabilities of the served data.
pub struct Interpreter {
    pack: LanguagePack,
    capabilities: Capabilities,
    anchor: Day,
    cues: grammar::Cues,
    policy: grammar::Policy,
    delivery: skills::Vocabulary,
    vocabulary: vocabulary::Known,
}

impl Interpreter {
    pub fn new(pack: LanguagePack, capabilities: Capabilities, anchor: Day) -> Self {
        Self {
            pack,
            capabilities,
            anchor,
            cues: toml::from_str(include_str!("../lang/pt-BR/cues.toml")).expect("embedded grammar cues"),
            policy: toml::from_str(include_str!("../lang/pt-BR/policy.toml")).expect("embedded policy"),
            delivery: skills::Vocabulary::embedded(),
            vocabulary: vocabulary::Known::embedded(),
        }
    }

    /// Catalogue entities spotted in `text` as `(start, end, readings)` over
    /// [`normalize::words`]; homonym cities share a span. Read-only, for audits and contracts.
    pub fn entities(&self, text: &str) -> Vec<(usize, usize, Vec<Entity>)> {
        self.pack.lexicon.scan(&normalize::words(text)).into_iter().map(|hit| (hit.start, hit.end, hit.values)).collect()
    }

    /// Whether `word` (as produced by [`normalize::words`]) belongs to the closed vocabulary
    /// of the grammar. Catalogue entities are known by their span, not by this list.
    pub fn knows(&self, word: &str) -> bool {
        self.vocabulary.knows(word)
    }

    pub fn interpret(&self, text: &str, state: &mut Conversation) -> Interpretation {
        if text.trim().is_empty() || text.chars().count() > 500 || text.chars().any(|c| c.is_control() && !c.is_whitespace()) {
            return Interpretation::stop(Decision::Reject, "Envie uma pergunta de até 500 caracteres, sem caracteres de controle.");
        }
        if state.last.as_ref().is_some_and(|r| r.time.anchor != self.anchor || r.versions != self.capabilities.catalogue_versions) {
            *state = Conversation::default();
            return Interpretation::stop(
                Decision::Clarify,
                "O contexto pertence a outra versão dos dados. Refaça a pesquisa com a base atual.",
            );
        }
        // A delivery phrase is cut before the grammar; a bare column list refines the
        // export of the current thread.
        let Some(delivery) = self.delivery.delivery(text, state.export.as_ref().filter(|_| state.last.is_some())) else {
            let (result, _) = self.answer(text, state, false);
            if let Some(company) = self.company(text, &result, state) {
                return company;
            }
            // Refinements and clarification answers keep the file of the thread; a new
            // question starts without one.
            state.export = if matches!(result.act.as_str(), "new_query" | "meta_question") { None } else { state.export.take() };
            return Interpretation { export: state.export.clone(), ..result };
        };
        if let Some(format) = &delivery.unsupported {
            return Interpretation::stop(
                Decision::Clarify,
                format!("Ainda não gero arquivos {format}. Posso entregar em Excel, CSV ou TXT: qual prefere?"),
            );
        }
        if state.last.is_none() && normalize::words(&delivery.rest).is_empty() {
            return Interpretation::stop(
                Decision::Clarify,
                "Quais empresas devo colocar no arquivo? Informe a atividade, o local ou a situação cadastral.",
            );
        }
        // "Agora em CSV", "manda essa lista": the file of the previous answer.
        let previous = state.last.is_some() && self.delivery.refers_to_previous(&delivery.rest);
        let (mut result, rows) = self.answer(if previous { "" } else { &delivery.rest }, state, true);
        if previous && result.request.is_some() {
            result.act = "export".into();
        }
        let mut export = delivery.export;
        if let Some(request) = &result.request {
            if matches!(request.op, Op::Meta { .. }) {
                result.notes.push("Essa resposta não gera arquivo.".into());
                state.export = None;
                return result;
            }
            if matches!(request.op, Op::List { .. }) {
                export.rows = rows;
            }
            result.notes.extend(delivery.notes);
            if result.decision == Decision::Execute && !result.notes.is_empty() {
                result.decision = Decision::ExecuteWithNote;
            }
        }
        state.export = Some(export.clone());
        result.export = Some(export);
        result
    }

    /// A company looked up by name, for a question the cadastral grammar did not execute
    /// ("encontre a empresa Zaflas", "zaflas"). Privacy and unavailable-data refusals stand.
    fn company(&self, text: &str, result: &Interpretation, state: &mut Conversation) -> Option<Interpretation> {
        // Validation failures for impossible requests must stand.
        if result.reason.starts_with("A consulta não é compatível") {
            return None;
        }
        // Only a refusal, or a question about words the grammar did not recognize, gives way.
        let unrecognized = result.decision == Decision::Clarify
            && ["Não reconheci", "Qual atividade", "Que tipo de loja"].iter().any(|start| result.reason.starts_with(start));
        if result.request.is_some() || !result.options.is_empty() || !(result.decision == Decision::Reject || unrecognized) {
            return None;
        }
        let n = normalize::words(text).join(" ");
        if grammar::cue(&n, &self.policy.privacy) || grammar::cue(&n, &self.policy.unavailable) {
            return None;
        }
        let named = self.delivery.company_name(text, result.decision == Decision::Reject)?;
        // A name typed alone never states an intent ("liste as autoescolas de Sinop").
        let opened = self.delivery.company_name(text, false).is_some();
        let stated = [&self.cues.list, &self.cues.count, &self.cues.compare, &self.cues.rank, &self.cues.trend, &self.cues.breakdown]
            .iter()
            .any(|list| grammar::cue(&n, list));
        if !opened && stated {
            return None;
        }
        // "filiais do Bradesco em MT": the establishments of a named company.
        let named = strip_establishment_noun(&named);
        // A CNPJ, valid or not, is answered by the lookup rules.
        if named.split_whitespace().any(|token| token.chars().filter(char::is_ascii_digit).count() >= 8) {
            return None;
        }
        let (name, municipalities, ufs) = self.place_suffix(&named);
        // A bare name stays short once its place is split off, and never reads like a data
        // question ("empresas ativas em Várzea Grande", "nr de pet shop em Florianópolis").
        let bare = self.delivery.company_name(text, false).is_none();
        let data_words = normalize::words(&name).iter().any(|w| {
            matches!(
                w.as_str(),
                "nr" | "numero"
                    | "quantidade"
                    | "total"
                    | "empresas"
                    | "estabelecimentos"
                    | "cnpjs"
                    | "negocios"
                    | "ativas"
                    | "ativos"
                    | "abertas"
                    | "abertos"
                    | "baixadas"
                    | "baixados"
                    | "lista"
                    | "listagem"
            )
        });
        if bare && (normalize::words(&name).len() > 5 || data_words) {
            return None;
        }
        let words = normalize::words(&name);
        // A plural activity request must not fall back to a company name. An explicit
        // name qualifier still allows a legal name such as "Empresas de Transporte União".
        if !time::has(&n, &["a empresa", "a firma", "razao social", "nome fantasia", "com o nome"])
            && matches!(words.as_slice(), [noun, link, ..]
                if matches!(noun.as_str(), "empresas" | "estabelecimentos" | "negocios" | "cnpjs" | "firmas" | "companhias")
                    && (matches!(link.as_str(), "de" | "do" | "da" | "dos" | "das" | "com" | "sem")
                        || matches!(link.as_str(), "no" | "na" | "em")
                            && words.get(2).is_some_and(|w| grammar::SECTOR_QUALIFIERS.contains(&w.as_str()))))
        {
            return None;
        }
        let hits = self.pack.lexicon.scan(&words);
        let only_sectors = !hits.is_empty()
            && hits.iter().map(|h| h.end - h.start).sum::<usize>() == words.len()
            && hits.iter().all(|h| h.values.iter().all(|e| matches!(e, Entity::Sector(_))));
        if only_sectors {
            // "busque padarias em Cuiabá" names an activity, not a company: list it.
            let (listed, _) = self.answer(&format!("liste {named}"), state, false);
            return listed.request.is_some().then_some(listed);
        }
        let has_sector = hits.iter().any(|h| h.values.iter().any(|e| matches!(e, Entity::Sector(_))));
        if has_sector {
            let cadastral_modifier = words.iter().any(|w| {
                matches!(
                    w.as_str(),
                    "telefone"
                        | "email"
                        | "contato"
                        | "abertas"
                        | "aberta"
                        | "ativas"
                        | "ativa"
                        | "baixadas"
                        | "baixada"
                        | "inaptas"
                        | "inapta"
                        | "fechadas"
                        | "fechada"
                        | "menos"
                        | "tirando"
                        | "sem"
                        | "exceto"
                        | "x"
                        | "versus"
                        | "vs"
                        | "qtd"
                        | "quantidade"
                        | "total"
                        | "cresceram"
                        | "cresceu"
                        | "variacao"
                        | "crescimento"
                        | "agora"
                )
            });
            if cadastral_modifier {
                return None;
            }
        }
        state.export = None;
        state.pending.clear();
        Some(Interpretation {
            decision: Decision::Execute,
            request: None,
            act: "company_search".into(),
            reason: String::new(),
            notes: Vec::new(),
            evidence: 1.0,
            options: Vec::new(),
            export: None,
            company: Some(CompanySearch { name, municipalities, ufs }),
        })
    }

    /// Split a trailing place ("Padaria Pão Quente em Cuiabá/MT") from a company name.
    fn place_suffix(&self, name: &str) -> (String, Vec<MunicipalityId>, Vec<UfId>) {
        let tokens: Vec<&str> = name.split_whitespace().collect();
        for at in (1..tokens.len().saturating_sub(1)).rev() {
            if !matches!(normalize::fold(tokens[at]).as_str(), "em" | "de" | "do" | "da" | "no" | "na") {
                continue;
            }
            let words = normalize::words(&tokens[at + 1..].join(" "));
            let hits = self.pack.lexicon.scan(&words);
            let covered = hits.iter().map(|h| h.end - h.start).sum::<usize>() == words.len();
            if !covered || hits.iter().any(|h| h.values.iter().any(|e| matches!(e, Entity::Sector(_)))) {
                continue;
            }
            let ufs: Vec<UfId> =
                hits.iter().flat_map(|h| &h.values).filter_map(|e| if let Entity::Uf(id) = e { Some(*id) } else { None }).collect();
            // A name that is both a state and a city ("Goiás", "São Paulo") reads as the state,
            // as in the cadastral grammar.
            let municipalities: Vec<MunicipalityId> = hits
                .iter()
                .filter(|h| !h.values.iter().any(|e| matches!(e, Entity::Uf(_))))
                .flat_map(|h| &h.values)
                .filter_map(|e| match e {
                    Entity::Municipality { id, uf } if ufs.is_empty() || ufs.contains(uf) => Some(*id),
                    _ => None,
                })
                .collect();
            if municipalities.is_empty() && ufs.is_empty() {
                continue;
            }
            let ufs = if municipalities.is_empty() { ufs } else { Vec::new() };
            return (tokens[..at].join(" "), municipalities, ufs);
        }
        (name.to_owned(), Vec::new(), Vec::new())
    }

    /// Interpret the data question. `export` makes a record count a record list; the
    /// second value is an explicit row count ("as 50 primeiras").
    fn answer(&self, text: &str, state: &mut Conversation, export: bool) -> (Interpretation, Option<u32>) {
        let population = self
            .capabilities
            .populations
            .iter()
            .filter_map(|p| if let PopulationRef::IbgeEstimate(y) = p { Some(*y) } else { None })
            .max()
            .map(PopulationRef::IbgeEstimate)
            .unwrap_or(PopulationRef::IbgeCensus(2022));
        let env = grammar::ResolveContext {
            pack: &self.pack,
            cues: &self.cues,
            policy: &self.policy,
            anchor: self.anchor,
            current: self.capabilities.current,
            competences: &self.capabilities.competences,
            versions: self.capabilities.catalogue_versions,
            population,
            vocabulary: &self.vocabulary,
            delivery: &self.delivery,
        };
        if !state.pending.is_empty() {
            let reply = normalize::words(text).join(" ");
            let numeric = time::number(&reply).and_then(|n| usize::try_from(n).ok()).and_then(|n| n.checked_sub(1));
            let matched = numeric.filter(|i| *i < state.pending.len()).or_else(|| {
                let indices: Vec<_> = state
                    .pending
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| {
                        let mut ufs = Vec::new();
                        match &r.scope.place {
                            PlaceFilter::Municipalities(ids) => {
                                ufs.extend(ids.iter().filter_map(|id| self.pack.municipality_ufs.get(id)).copied())
                            }
                            PlaceFilter::Ufs(ids) => ufs.extend(ids.iter().copied()),
                            _ => {}
                        }
                        let uf_match = ufs.iter().any(|id| {
                            lang::UFS.get(id.0 as usize).is_some_and(|uf| time::has(&reply, &[uf.to_ascii_lowercase().as_str()]))
                        });
                        (uf_match || time::has(&reply, &["demais"]) && r.scope.porte == PorteSet::only(Porte::Other)).then_some(i)
                    })
                    .collect();
                (indices.len() == 1).then(|| indices[0])
            });
            if let Some(i) = matched
                && let Ok(valid) = validate::validate(state.pending[i].clone(), &self.capabilities)
            {
                state.remember(valid.as_request());
                let result = Interpretation {
                    decision: Decision::Execute,
                    request: Some(valid),
                    act: "answer_clarification".into(),
                    reason: String::new(),
                    notes: Vec::new(),
                    evidence: 1.0,
                    options: Vec::new(),
                    export: None,
                    company: None,
                };
                return (result, None);
            }
            // A new complete question discards a pending clarification.
            state.pending.clear();
        }
        let frame = match grammar::resolve(&env, text, state, export) {
            Ok(f) => f,
            Err(result) => {
                let mut result = *result;
                if result.decision == Decision::Clarify && result.reason.contains("UF do município") {
                    for uf in lang::UFS {
                        if let Ok(frame) = grammar::resolve(&env, &format!("{text} {uf}"), state, export) {
                            let ctx = CanonicalContext {
                                first_day: Day(1),
                                last_day: self.anchor,
                                municipality_ufs: &self.pack.municipality_ufs,
                                sector_cnaes: &self.pack.sector_cnaes,
                            };
                            if let Ok((request, _)) = frame.request.canonicalize(&ctx)
                                && let Ok(valid) = validate::validate(request, &self.capabilities)
                                && !result.options.contains(valid.as_request())
                            {
                                result.options.push(valid.into_request());
                            }
                        }
                        if result.options.len() == 4 {
                            break;
                        }
                    }
                }
                state.pending = result.options.clone();
                return (result, None);
            }
        };
        let context = CanonicalContext {
            first_day: Day(1),
            last_day: self.anchor,
            municipality_ufs: &self.pack.municipality_ufs,
            sector_cnaes: &self.pack.sector_cnaes,
        };
        let request = match frame.request.canonicalize(&context) {
            Ok((r, _)) => r,
            Err(error) => {
                let reason = format!("Os filtros não são compatíveis: {error}. Confira município, UF e período.");
                return (Interpretation::stop(Decision::Clarify, reason), None);
            }
        };
        let result = match validate::validate(request, &self.capabilities) {
            Ok(valid) => {
                state.remember(valid.as_request());
                Interpretation {
                    decision: if frame.notes.is_empty() { Decision::Execute } else { Decision::ExecuteWithNote },
                    request: Some(valid),
                    act: frame.act,
                    reason: String::new(),
                    notes: frame.notes,
                    evidence: 1.0,
                    options: Vec::new(),
                    export: None,
                    company: None,
                }
            }
            Err(issues) => {
                let decision = if issues.iter().any(|i| i.kind == IssueKind::Impossible) { Decision::Reject } else { Decision::Clarify };
                let mut result = Interpretation::stop(
                    decision,
                    "A consulta não é compatível com os dados disponíveis. Confira o período, filtros e limite de resultados.",
                );
                result.notes = issues.iter().map(|i| i.code.message().to_owned()).collect();
                result.options = issues.into_iter().flat_map(|i| i.options).take(4).map(|r| r.into_request()).collect();
                state.pending = result.options.clone();
                result
            }
        };
        (result, frame.rows)
    }
}

/// Drop a leading "filiais do", "unidades da", "lojas da" from a company name.
fn strip_establishment_noun(name: &str) -> String {
    let tokens: Vec<&str> = name.split_whitespace().collect();
    let noun = tokens.first().map(|t| normalize::fold(t)).unwrap_or_default();
    let article = tokens.get(1).map(|t| normalize::fold(t)).unwrap_or_default();
    if tokens.len() > 2
        && matches!(noun.as_str(), "filiais" | "filial" | "unidades" | "lojas" | "agencias" | "matriz")
        && matches!(article.as_str(), "do" | "da" | "de" | "dos" | "das")
    {
        tokens[2..].join(" ")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests;
