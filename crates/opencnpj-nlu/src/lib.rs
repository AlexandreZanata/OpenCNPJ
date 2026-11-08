//! Deterministic Brazilian Portuguese interpretation with selective, validated execution.

pub mod context;
pub mod grammar;
pub mod lang;
pub mod normalize;
pub mod policy;
pub mod spotter;
pub mod time;

use context::Conversation;
use lang::LanguagePack;
use opencnpj_query::{
    ir::*,
    validate::{self, Capabilities, IssueKind},
};
use policy::{Decision, Interpretation};

/// An immutable catalogue pack and the exact capabilities of the served data.
pub struct Interpreter {
    pack: LanguagePack,
    capabilities: Capabilities,
    anchor: Day,
    cues: grammar::Cues,
    policy: grammar::Policy,
}

impl Interpreter {
    pub fn new(pack: LanguagePack, capabilities: Capabilities, anchor: Day) -> Self {
        Self {
            pack,
            capabilities,
            anchor,
            cues: toml::from_str(include_str!("../lang/pt-BR/cues.toml")).expect("embedded grammar cues"),
            policy: toml::from_str(include_str!("../lang/pt-BR/policy.toml")).expect("embedded policy"),
        }
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
                return Interpretation {
                    decision: Decision::Execute,
                    request: Some(valid),
                    act: "answer_clarification".into(),
                    reason: String::new(),
                    notes: Vec::new(),
                    evidence: 1.0,
                    options: Vec::new(),
                };
            }
            // A new complete question discards a pending clarification.
            state.pending.clear();
        }
        let frame = match grammar::resolve(&env, text, state) {
            Ok(f) => f,
            Err(result) => {
                let mut result = *result;
                if result.decision == Decision::Clarify && result.reason.contains("UF do município") {
                    for uf in lang::UFS {
                        if let Ok(frame) = grammar::resolve(&env, &format!("{text} {uf}"), state) {
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
                return result;
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
                return Interpretation::stop(
                    Decision::Clarify,
                    format!("Os filtros não são compatíveis: {error}. Confira município, UF e período."),
                );
            }
        };
        match validate::validate(request, &self.capabilities) {
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
        }
    }
}

#[cfg(test)]
mod tests;
