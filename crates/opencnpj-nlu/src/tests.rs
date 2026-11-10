use super::*;
use crate::lang::LanguagePack;

fn interpreter() -> Interpreter {
    interpreter_with(&[])
}

/// The shared test pack plus `extra` sectors, numbered after the three fixed ones.
fn interpreter_with(extra: &[(&[&str], u32)]) -> Interpreter {
    let mut pack = LanguagePack::new();
    pack.municipality(MunicipalityId(1), UfId(12), "Sorriso", &[]);
    pack.municipality(MunicipalityId(2), UfId(12), "Sinop", &[]);
    pack.municipality(MunicipalityId(3), UfId(16), "Bom Jesus", &[]);
    pack.municipality(MunicipalityId(4), UfId(22), "Bom Jesus", &[]);
    pack.municipality(MunicipalityId(5), UfId(11), "Campo Grande", &[]);
    pack.municipality(MunicipalityId(6), UfId(7), "Serra", &[]);
    pack.municipality(MunicipalityId(7), UfId(14), "Serraria", &[]);
    pack.municipality(MunicipalityId(8), UfId(11), "Três Lagoas", &[]);
    pack.municipality(MunicipalityId(9), UfId(24), "Itaporanga d'Ajuda", &[]);
    pack.municipality(MunicipalityId(10), UfId(10), "Contagem", &[]);
    pack.municipality(MunicipalityId(11), UfId(22), "Não-Me-Toque", &[]);
    pack.municipality(MunicipalityId(12), UfId(10), "Juiz de Fora", &[]);
    pack.sector(SectorId(0), &["padaria".into(), "padarias".into()], SmallSet::new([CnaeCode::Subclass(4721102)]));
    pack.sector(SectorId(1), &["farmacia".into(), "farmacias".into()], SmallSet::new([CnaeCode::Subclass(4771701)]));
    pack.sector(SectorId(2), &["serraria".into(), "serrarias".into()], SmallSet::new([CnaeCode::Subclass(1610203)]));
    for (i, (aliases, cnae)) in extra.iter().enumerate() {
        let aliases: Vec<String> = aliases.iter().map(|a| (*a).to_owned()).collect();
        pack.sector(SectorId(3 + i as u16), &aliases, SmallSet::new([CnaeCode::Subclass(*cnae)]));
    }
    let anchor = Day::from_ymd(2026, 10, 11).unwrap();
    let caps = Capabilities {
        current: Competence(321),
        competences: (299..=321).map(Competence).collect(),
        flow_days: Day::from_ymd(2025, 1, 1).unwrap()..=anchor,
        populations: vec![PopulationRef::IbgeEstimate(2026), PopulationRef::IbgeCensus(2022)],
        catalogue_versions: CatalogueVersions { municipalities: 202610, sectors: 1 },
        sector_count: 3 + extra.len() as u16,
        geography: true,
        list_limit: 100,
    };
    Interpreter::new(pack, caps, anchor)
}
fn parse(text: &str) -> Interpretation {
    interpreter().interpret(text, &mut Conversation::default())
}
fn req(text: &str) -> Request {
    let result = parse(text);
    result.request.clone().unwrap_or_else(|| panic!("{text}: {result:?}")).into_request()
}

#[test]
fn registrations_and_active_stock_are_distinct() {
    let a = req("quantas padarias abriram em Sorriso em 2025");
    let b = req("quantas padarias ativas existem em Sorriso");
    assert_eq!(a.op.metric(), Some(Metric::Flow(FlowMetric::Registrations)));
    assert_eq!(a.scope.status, Some(StatusSet::ALL));
    assert_eq!(b.op.metric(), Some(Metric::Stock));
    assert_eq!(b.scope.status, Some(StatusSet::only(Status::Active)));
}
#[test]
fn explicit_current_status_survives_a_registration_flow() {
    let r = req("Quantas padarias abriram em Sorriso em 2025 e continuam ativas?");
    assert_eq!(r.scope.status, Some(StatusSet::only(Status::Active)));
}
#[test]
fn negation_and_legal_form_are_independent() {
    let r = req("Conte padarias ativas em Sorriso que nao sao MEI e sem filiais");
    assert_eq!(r.scope.mei, Tri::No);
    assert_eq!(r.scope.head_office, Tri::Yes);
}
#[test]
fn qualified_homonyms_and_city_words_do_not_trigger_quality() {
    assert_eq!(req("Quantas padarias em Bom Jesus/PI?").scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(3)])));
    assert_eq!(parse("Quantas padarias em Bom Jesus?").decision, Decision::Clarify);
    assert!(parse("Quantas padarias na cidade de Campo Grande/MS?").request.is_some());
}
#[test]
fn no_revenue_or_person_search_can_be_executed() {
    for text in [
        "Qual o faturamento das padarias em Sorriso?",
        "Liste os socios das padarias em Sorriso",
        "Liste os CPFs dos donos",
        "Preveja as empresas em 2028",
    ] {
        assert_eq!(parse(text).decision, Decision::Reject, "{text}");
    }
}
#[test]
fn list_order_pagination_and_capital_have_different_roles() {
    let r = req("Liste 15 padarias ativas em Sorriso com e-mail dos mais novos para os mais antigos");
    assert_eq!(r.op, Op::List { order: ListOrder::NewestFirst, limit: 15 });
    assert_eq!(r.scope.contact, ContactNeed::Email);
    let r = req("Liste 25 padarias ativas em Sorriso na segunda pagina em ordem de CNPJ");
    assert_eq!(r.output.page, 2);
    let r = req("Liste 10 padarias em Sorriso por maior capital social");
    assert_eq!(r.scope.capital, None);
}
#[test]
fn context_replaces_and_removes_and_resets() {
    let engine = interpreter();
    let mut state = Conversation::default();
    for text in ["quantas padarias em Sorriso", "e em Sinop?", "agora so as matrizes"] {
        assert!(engine.interpret(text, &mut state).request.is_some(), "{text}");
    }
    assert_eq!(state.last.as_ref().unwrap().scope.head_office, Tri::Yes);
    let result = engine.interpret("Retire o filtro de matriz mantendo o restante", &mut state);
    assert_eq!(result.request.unwrap().scope.head_office, Tri::Any);
    let result = engine.interpret("Nova pesquisa: quantas farmacias em Sorriso?", &mut state);
    let request = result.request.unwrap();
    assert_eq!(request.scope.head_office, Tri::Any);
    assert_eq!(request.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(1)])));
}
#[test]
fn malformed_and_stale_input_never_executes() {
    assert_eq!(parse(&"a".repeat(501)).decision, Decision::Reject);
    assert_eq!(parse("cnpj\0").decision, Decision::Reject);
    assert_eq!(parse("Consulte o CNPJ 33.000.167/0001-00").decision, Decision::Clarify);
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("quantas padarias em Sorriso", &mut state);
    state.last.as_mut().unwrap().versions.sectors = 99;
    assert_eq!(engine.interpret("e em Sinop?", &mut state).decision, Decision::Clarify);
    assert!(state.last.is_none());
}
#[test]
fn density_is_not_a_minimum_population_filter() {
    let r = req("Quais os 10 municipios de MT com menos padarias por 10 mil habitantes?");
    assert!(matches!(r.op, Op::Rank { metric: Metric::Per10k(_), dir: Dir::Asc, .. }));
    assert_eq!(r.scope.place_population, None);
}
#[test]
fn unknown_geography_or_activity_never_broadens_the_query() {
    for text in [
        "Quantas padarias em Foobaria?",
        "Quantas unicornarias em Sorriso?",
        "Conte padarias em Sorriso sem frota propria",
        "Quantas padarias em Sorriso com mais de 20 funcionarios?",
    ] {
        assert!(parse(text).request.is_none(), "{text}");
    }
}

#[test]
fn clarification_options_are_valid_and_can_be_answered() {
    let engine = interpreter();
    let mut state = Conversation::default();
    let ambiguous = engine.interpret("Quantas padarias existem em Bom Jesus?", &mut state);
    assert_eq!(ambiguous.decision, Decision::Clarify);
    assert_eq!(ambiguous.options.len(), 2);
    let answer = engine.interpret("PI", &mut state);
    assert_eq!(answer.act, "answer_clarification");
    assert_eq!(answer.request.unwrap().scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(3)])));
}
#[test]
fn a_city_name_does_not_hide_a_large_company_ambiguity() {
    assert_eq!(parse("Quantas padarias grandes na cidade de Campo Grande/MS?").decision, Decision::Clarify);
    assert!(parse("Quantas padarias em Sorriso/SP?").request.is_none());
}
#[test]
fn references_use_actual_result_members() {
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("Quais os 10 municipios de MT com mais padarias?", &mut state);
    state.remember_result(vec![Member::Municipality(MunicipalityId(1)), Member::Municipality(MunicipalityId(2))]);
    let result = engine.interpret("Liste 10 padarias dessas cidades", &mut state);
    assert_eq!(result.act, "refer_to_result");
    assert_eq!(result.request.unwrap().scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(1), MunicipalityId(2)])));
}
#[test]
fn periods_use_calendar_months_and_explicit_dates() {
    let anchor = Day::from_ymd(2026, 10, 11).unwrap();
    assert_eq!(
        time::period("ultimos seis meses", anchor),
        Some(Period::Days(Range { from: Day::from_ymd(2026, 4, 12), to: Some(anchor) }))
    );
    assert_eq!(
        time::period("de 01/03/2025 a 31/05/2025", anchor),
        Some(Period::Days(Range { from: Day::from_ymd(2025, 3, 1), to: Day::from_ymd(2025, 5, 31) }))
    );
    assert_eq!(time::period("desde marco", anchor), Some(Period::Days(Range { from: Day::from_ymd(2026, 3, 1), to: Some(anchor) })));
}

#[test]
fn complete_dates_survive_entity_masking_in_real_questions() {
    for dates in ["de 01/03/2025 a 31/05/2025", "de 2025-03-01 a 2025-05-31"] {
        let r = req(&format!("Quantas padarias abriram em Sorriso {dates}?"));
        assert_eq!(r.op.metric(), Some(Metric::Flow(FlowMetric::Registrations)));
        assert_eq!(r.time.period, Some(Period::Days(Range { from: Day::from_ymd(2025, 3, 1), to: Day::from_ymd(2025, 5, 31) })));
    }
    for text in ["Quantas padarias abriram em Sorriso de 31/02/2025 a 31/05/2025?", "Quantas padarias abriram em Sorriso em 2025-13-01?"] {
        assert_eq!(parse(text).decision, Decision::Clarify, "{text}");
    }
}

#[test]
fn historical_stock_requires_an_explicit_month() {
    for text in ["Quantas padarias estavam ativas em Sorriso em 2025?", "Quantas padarias tinham em Sorriso em 2025?"] {
        assert_eq!(parse(text).decision, Decision::Clarify, "{text}");
    }
    let r = req("Quantas padarias estavam ativas em Sorriso na competencia 2025-06?");
    assert_eq!(r.time.competence, Competence::from_year_month(2025, 6).unwrap());
    assert_eq!(parse("Quantas empresas estavam ativas em Sorriso em 2010?").decision, Decision::Reject);
}

#[test]
fn closure_dates_cannot_be_inferred_from_registration_dates() {
    let text = "Quantas padarias tiveram baixa cadastral com inicio de atividade em 2025 em Sorriso?";
    assert_eq!(parse(text).decision, Decision::Clarify);
    let r = req("Quantas padarias tiveram baixa cadastral em 2026 com inicio de atividade em 2025 em Sorriso?");
    assert_eq!(r.op.metric(), Some(Metric::Flow(FlowMetric::Closures)));
    assert_eq!(r.time.period, Some(Period::Days(Range { from: Day::from_ymd(2026, 1, 1), to: Day::from_ymd(2026, 10, 11) })));
    assert_eq!(r.scope.opened, Some(Range { from: Day::from_ymd(2025, 1, 1), to: Day::from_ymd(2025, 12, 31) }));
}

#[test]
fn registration_date_filters_are_not_dropped_from_lists_or_stock() {
    for text in [
        "Liste 10 padarias com inicio de atividade em 2025 em Sorriso",
        "Quantas padarias ativas com inicio de atividade em 2025 existem em Sorriso?",
    ] {
        let r = req(text);
        assert_eq!(r.scope.opened, Some(Range { from: Day::from_ymd(2025, 1, 1), to: Day::from_ymd(2025, 12, 31) }));
        assert_eq!(r.time.period, None);
    }
}

#[test]
fn informal_company_size_is_still_ambiguous() {
    assert_eq!(parse("empresas grandonas em Sinop quantas").decision, Decision::Clarify);
}

#[test]
fn a_numeric_filter_is_not_a_cnpj_root_lookup() {
    let r = req("Quantos CNPJs de padarias em Sorriso tem capital social acima de 10000000 reais?");
    assert_eq!(r.op, Op::Count { metric: Metric::Stock });
    assert_eq!(r.scope.capital, Some(Range { from: Some(1_000_000_001), to: None }));
    assert!(matches!(req("33000167").op, Op::Lookup { .. }));
    assert!(matches!(req("Consulte a raiz 33000167").op, Op::Lookup { .. }));
    assert!(matches!(req("Busque o registro 33.000.167 nesta base").op, Op::Lookup { .. }));
    assert!(matches!(req("Consulte o CNPJ 33.000.167/0001-01").op, Op::Lookup { .. }));
}

#[test]
fn decimal_currency_and_literal_status_codes_are_not_dropped() {
    let r = req("Conte padarias em Sorriso com capital social entre R$ 1.000,50 e R$ 2.500,75");
    assert_eq!(r.scope.capital, Some(Range { from: Some(100050), to: Some(250075) }));
    let r = req("Conte padarias em Sorriso com situacao cadastral 08 e porte 03");
    assert_eq!(r.scope.status, Some(StatusSet::only(Status::Closed)));
    assert_eq!(r.scope.porte, PorteSet::only(Porte::Small));
    assert!(parse("Conte padarias em Sorriso com capital social acima de muito").request.is_none());
    assert!(parse("Conte padarias em Sorriso com situacao cadastral 99").request.is_none());
}
#[test]
fn followup_comparison_keeps_the_new_and_previous_places() {
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("Quantas padarias em Sorriso?", &mut state);
    let r = engine.interpret("E com Sinop?", &mut state).request.unwrap();
    assert!(matches!(r.op, Op::Compare { .. }));
    assert_eq!(
        r.op,
        Op::Compare {
            dim: Dim::Municipality,
            members: SmallSet::new([Member::Municipality(MunicipalityId(1)), Member::Municipality(MunicipalityId(2))]),
            metric: Metric::Stock
        }
    );
}

#[test]
fn entity_month_names_cannot_change_the_requested_period() {
    let mut pack = LanguagePack::new();
    pack.municipality(MunicipalityId(1), UfId(18), "Rio de Janeiro", &[]);
    pack.sector(SectorId(0), &["padarias".into()], SmallSet::new([CnaeCode::Subclass(4721102)]));
    let anchor = Day::from_ymd(2026, 10, 11).unwrap();
    let mut engine = interpreter();
    engine.pack = pack;
    let r =
        engine.interpret("Quantas padarias abriram na cidade do Rio de Janeiro em 2025?", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.time.period, Some(Period::Days(Range { from: Day::from_ymd(2025, 1, 1), to: Day::from_ymd(2025, 12, 31) })));
    assert_eq!(r.time.anchor, anchor);
}
#[test]
fn short_geographic_exclusions_and_future_requests_do_not_silently_change_meaning() {
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("Quantas padarias em MT?", &mut state);
    let r = engine.interpret("sem Sorriso", &mut state).request.unwrap();
    assert_eq!(r.scope.exclude.municipalities, SmallSet::new([MunicipalityId(1)]));
    assert_eq!(parse("Quantas padarias vai ter em 2030?").decision, Decision::Reject);
    assert_eq!(req("Quantas padarias baixaram em Sorriso em 2025?").op.metric(), Some(Metric::Flow(FlowMetric::Closures)));
}

#[test]
fn negation_attaches_to_its_target_and_cannot_leak_to_later_entities() {
    let a = req("Quantas padarias sem filiais existem em Sorriso?");
    assert_eq!(a.scope.head_office, Tri::Yes);
    assert_eq!(a.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(1)])));
    assert!(a.scope.exclude.municipalities.is_empty());
    let b = req("Quantas padarias exceto Sorriso existem em Mato Grosso?");
    assert_eq!(b.scope.place, PlaceFilter::Ufs(SmallSet::new([UfId(12)])));
    assert!(b.scope.exclude.ufs.is_empty());
    assert_eq!(b.scope.exclude.municipalities, SmallSet::new([MunicipalityId(1)]));
}
#[test]
fn a_bare_topic_defaults_only_when_all_words_are_explained() {
    let a = req("farmacias Sinop");
    assert_eq!(a.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)])));
    assert!(parse("farmacias Sinop cartoes premium").request.is_none());
    assert!(parse("Quantas lojas tem em Sinop?").request.is_none());
    assert!(parse("Quantas farmacias nas cidades pequenas de MT?").request.is_none());
}

#[test]
fn count_nouns_are_not_automatically_the_city_of_contagem() {
    let mut engine = interpreter();
    engine.pack.municipality(MunicipalityId(6), UfId(10), "Contagem", &[]);
    let mut state = Conversation::default();
    engine.interpret("Quantas padarias em Sinop?", &mut state);
    let r = engine.interpret("Mostra a contagem apenas das matrizes", &mut state).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)])));
    let r = engine.interpret("Volte a incluir filiais nessa contagem", &mut state).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)])));
}

#[test]
fn excluding_mei_does_not_invert_an_explicit_branch_filter() {
    let r = req("Conte padarias em Sorriso sem MEI e somente filiais");
    assert_eq!(r.scope.mei, Tri::No);
    assert_eq!(r.scope.head_office, Tri::No);
}

#[test]
fn available_historical_flows_do_not_require_an_old_stock_snapshot() {
    let mut engine = interpreter();
    let anchor = Day::from_ymd(2026, 10, 11).unwrap();
    engine.capabilities.flow_days = Day(1)..=anchor;
    let r = engine.interpret("Quantas padarias abriram em Sorriso em 2024?", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.time.period, Some(Period::Days(Range { from: Day::from_ymd(2024, 1, 1), to: Day::from_ymd(2024, 12, 31) })));
    assert_eq!(r.scope.status, Some(StatusSet::ALL));
    assert!(engine.interpret("Quantas padarias estavam ativas em Sorriso em 2020?", &mut Conversation::default()).request.is_none());
}

#[test]
fn a_later_place_cannot_hide_an_unknown_negated_condition() {
    assert!(parse("Quantas padarias que nao sejam fraudulentas tem em Sorriso?").request.is_none());
    let r = req("Quantas padarias que nao estao ativas tem em Sorriso?");
    assert_eq!(r.scope.status, Some(StatusSet::new([Status::Suspended, Status::Unfit, Status::Closed])));
}

#[test]
fn commercial_round_02_grammar_regressions() {
    let r = req("Traga 5 padarias ativas em Sorriso com telefone");
    assert_eq!(r.op, Op::List { order: ListOrder::Cnpj, limit: 5 });
    let r = req("Quantas padarias ativas em Sorriso nao sao optantes pelo MEI?");
    assert_eq!(r.scope.mei, Tri::No);
    let r = req("Quantas padarias ativas em Sorriso nao sao optantes pelo Simples?");
    assert_eq!(r.scope.simples, Tri::No);
    let r = req("Qual a densidade por 10 mil habitantes de padarias em Sorriso, usando Censo IBGE 2022?");
    assert_eq!(r.op.metric(), Some(Metric::Per10k(PopulationRef::IbgeCensus(2022))));
    let r = req("Quais os 10 grupos de municipios com menor densidade de padarias em MT com ate 30 mil habitantes, usando IBGE 2026?");
    assert_eq!(r.scope.place_population, Some(Range { from: None, to: Some(30000) }));
    let r = req("Quais os 10 grupos de municipios com maior densidade de padarias em MT com mais de 100 mil habitantes, usando IBGE 2026?");
    assert_eq!(r.scope.place_population, Some(Range { from: Some(100001), to: None }));
    // A two-letter UF hit must not erase matching substrings in time words.
    let r = req("Quantas padarias foram registradas com inicio de atividade nos ultimos 12 meses na cidade de Serra/ES?");
    assert_eq!(r.time.period, Some(Period::Days(Range { from: Day::from_ymd(2025, 10, 12), to: Day::from_ymd(2026, 10, 11) })));
    // Singular head-office nouns behave like their plurals in follow-ups.
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("quantas padarias em Sorriso", &mut state);
    let r = engine.interpret("agora so as matrizes", &mut state).request.unwrap();
    assert_eq!(r.scope.head_office, Tri::Yes);
    let r = engine.interpret("Conte matriz e filial.", &mut state).request.unwrap();
    assert_eq!(r.scope.head_office, Tri::Any);
}

#[test]
fn commercial_round_05_grammar_regressions() {
    // Trailing filler punctuation after an explicit CNAE division/group code
    // must not drop the activity filter (round-05 dedup filler ", por favor?").
    let r = req("Quantos estabelecimentos ativos na cidade de Sorriso/MT têm CNAE principal na divisão 43, por favor?");
    assert_eq!(r.scope.activity, ActivityFilter::Cnae { codes: SmallSet::new([CnaeCode::Division(43)]), scope: CnaeScope::Main });
    let r = req("Quantos estabelecimentos ativos na cidade de Sorriso/MT têm CNAE principal no grupo 474, por favor?");
    assert_eq!(r.scope.activity, ActivityFilter::Cnae { codes: SmallSet::new([CnaeCode::Group(474)]), scope: CnaeScope::Main });
}

#[test]
fn commercial_round_06_grammar_regressions() {
    // Trailing punctuation after a valid CNPJ must not turn a lookup into a clarify.
    let r = req("O que consta na base sobre o CNPJ 51.259.606/0001-87, por favor?");
    assert_eq!(r.op, Op::Lookup { id: CnpjRef::parse("51259606000187").unwrap() });
    // "Serraria" is both a curated sector and a municipality; in an activity
    // construction the sector wins, even after the trend metric word "menos".
    let r = req(
        "Mostre a evolução mensal em 2025 de saldo de registros menos baixas cadastrais de estabelecimentos de serraria na cidade de Três Lagoas/MS.",
    );
    assert!(matches!(r.scope.activity, ActivityFilter::Sectors { ref ids, .. } if ids.contains(&SectorId(2))));
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(8)])));
    // A geographic use of the same name still resolves the municipality.
    let r = req("Quantas padarias existem em Serraria/PB?");
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(7)])));
    assert_eq!(
        r.scope.activity,
        ActivityFilter::Sectors {
            ids: SmallSet::new([SectorId(0)]),
            cnaes: SmallSet::new([CnaeCode::Subclass(4721102)]),
            scope: CnaeScope::Main
        }
    );
    // Cue words inside entity names must not change the intent: "Ajuda" in a
    // municipality is not a help request, and "Contagem" is not a count cue.
    assert_eq!(parse("Bom dia, tudo bem? Queria conversar sobre na cidade de Itaporanga d'Ajuda/SE.").decision, Decision::Reject);
    let r = req("Quantos estabelecimentos ativos de padaria existem na cidade de Itaporanga d'Ajuda/SE?");
    assert_eq!(r.op, Op::Count { metric: Metric::Stock });
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(9)])));
    assert!(matches!(
        parse("Que pesquisas posso fazer em Itaporanga d'Ajuda/SE?").request.unwrap().op,
        Op::Meta { topic: MetaTopic::Help }
    ));
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("Quantos estabelecimentos ativos de padaria existem em Sorriso?", &mut state);
    let r = engine.interpret("Só matrizes na cidade de Contagem/MG.", &mut state).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(10)])));
    assert_eq!(r.scope.head_office, Tri::Yes);
    // "Não" inside the municipality name is not an unsupported exclusion.
    let r = req("Quantos estabelecimentos ativos de padaria existem na cidade de Não-Me-Toque/RS?");
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(11)])));
    assert_eq!(parse("Como faço para abrir uma empresa na cidade de Não-Me-Toque/RS?").decision, Decision::Reject);
    // "Fora" inside Juiz de Fora must not hide the real "exceto" marker.
    // Canonicalization applies the MS exclusion to the place, leaving AC.
    let r = req("Quantos estabelecimentos suspensos de padaria existem em AC e MS, exceto os da cidade de Juiz de Fora/MG e os de MS?");
    assert_eq!(r.scope.place, PlaceFilter::Ufs(SmallSet::new([UfId(0)])));
    assert!(r.scope.exclude.ufs.is_empty());
    assert_eq!(
        req("Quantas padarias existem em Juiz de Fora/MG?").scope.place,
        PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(12)]))
    );
    // In "exceto os de serraria" the shared sector/municipality name is a sector.
    let r = req("Quantos estabelecimentos ativos de padaria existem na cidade de Sorriso/MT, exceto os de serraria?");
    assert_eq!(r.scope.exclude.sectors, SmallSet::new([SectorId(2)]));
    // A city reading still wins when the exclusion explicitly says "cidade de".
    let r = req("Quantos estabelecimentos ativos de padaria existem em PB, exceto os da cidade de Serraria/PB?");
    assert_eq!(r.scope.exclude.municipalities, SmallSet::new([MunicipalityId(7)]));
}

#[test]
fn probe_baixas_debug() {
    for text in [
        "Quantas baixas de padarias ocorreram em 2025 na cidade de Sorriso/MT?",
        "Quantas padarias tiveram baixa cadastral em 2025 na cidade de Sorriso/MT?",
    ] {
        let r = parse(text);
        println!("TEXT: {text}\n  decision={:?} reason={}", r.decision, r.reason);
    }
}

#[test]
fn commercial_round_03_grammar_regressions() {
    // "Quantas baixas de X" reads as a closure count like its verb forms.
    let r = req("Quantas baixas de padarias ocorreram em 2025 na cidade de Sorriso/MT?");
    assert_eq!(r.op.metric(), Some(Metric::Flow(FlowMetric::Closures)));
    assert_eq!(r.scope.status, Some(StatusSet::only(Status::Closed)));
    // Masculine agreement selects secondary CNAE scope like the feminine forms.
    let r = req("Quero o total com CNAE principal ou secundario 4721102 na cidade de Sorriso/MT?");
    assert_eq!(r.scope.activity, ActivityFilter::Cnae { codes: SmallSet::new([CnaeCode::Subclass(4721102)]), scope: CnaeScope::Any });
    // Resetting "todos os portes" restores the full porte set.
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("quantas padarias em Sorriso", &mut state);
    engine.interpret("so de micro porte", &mut state);
    let r = engine.interpret("Volte a incluir todos os portes.", &mut state).request.unwrap();
    assert_eq!(r.scope.porte, PorteSet::ALL);
    // Lottery questions are unavailable data, like revenue and forecasts.
    assert_eq!(parse("Quais os numeros da sorte para abrir empresa em Sorriso?").decision, Decision::Reject);
    // Combined MEI+Simples negation scopes each flag separately.
    let r = req("Quantas padarias ativas em Sorriso nao sao optantes pelo MEI e sao optantes pelo Simples?");
    assert_eq!(r.scope.mei, Tri::No);
    assert_eq!(r.scope.simples, Tri::Yes);
    let r = req("Quantas padarias ativas em Sorriso sao optantes pelo MEI e estao fora do Simples?");
    assert_eq!(r.scope.mei, Tri::Yes);
    assert_eq!(r.scope.simples, Tri::No);
    // Coordinated exclusions keep both places and record both exclusions.
    let r = req("Quantas padarias ativas existem em MT e RO, exceto os de RO?");
    assert_eq!(r.scope.place, PlaceFilter::Ufs(SmallSet::new([UfId(12)])));
    // An exclusion naming a city and a UF excludes both.
    let r = req("Quantas padarias ativas existem em MT e MS, exceto Sinop e os de MS?");
    assert_eq!(r.scope.place, PlaceFilter::Ufs(SmallSet::new([UfId(12)])));
    assert_eq!(r.scope.exclude.municipalities, SmallSet::new([MunicipalityId(2)]));
}

#[test]
fn commercial_round_09_grammar_regressions() {
    let mut engine = interpreter();
    engine.pack.municipality(MunicipalityId(20), UfId(1), "Matriz de Camaragibe", &[]);
    engine.pack.municipality(MunicipalityId(21), UfId(17), "Dois Vizinhos", &[]);
    engine.pack.municipality(MunicipalityId(22), UfId(14), "Logradouro", &[]);
    engine.pack.municipality(MunicipalityId(23), UfId(10), "Passos", &[]);

    // "Quantos habitantes por estabelecimento..." selects InhabitantsPer.
    let r = req(
        "Quantos habitantes por estabelecimento ativo de padaria existem na cidade de Sorriso/MT, usando a população estimada IBGE 2026?",
    );
    assert_eq!(r.op, Op::Count { metric: Metric::InhabitantsPer(PopulationRef::IbgeEstimate(2026)) });

    // "Qual o saldo de registros menos baixas..." selects NetRegistrations count.
    let r = req("Qual o saldo de registros menos baixas cadastrais de estabelecimentos de padaria na cidade de Sorriso/MT em 2025?");
    assert_eq!(r.op, Op::Count { metric: Metric::Flow(FlowMetric::NetRegistrations) });

    // Capital range filter survives when the list is also ordered by largest capital social.
    let r = req(
        "Liste os 20 primeiros estabelecimentos ativos de padaria na cidade de Sorriso/MT com capital social acima de 100 mil reais, ordenados pelo maior capital social.",
    );
    assert_eq!(r.op, Op::List { order: ListOrder::CapitalDesc, limit: 20 });
    assert_eq!(r.scope.capital, Some(Range { from: Some(10_000_001), to: None }));

    // All-uppercase preposition "PARA" in "DOS MAIS ANTIGOS PARA OS MAIS NOVOS" is not the UF Pará.
    let r = req("LISTE OS 20 PRIMEIROS ESTABELECIMENTOS ATIVOS DE PADARIA EM SORRISO/MT DOS MAIS ANTIGOS PARA OS MAIS NOVOS.");
    assert_eq!(r.op, Op::List { order: ListOrder::OldestFirst, limit: 20 });
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(1)])));

    // Municipality names containing "Matriz", "Vizinhos", or "Passos" do not trigger command filters.
    let r = engine
        .interpret("Quantos estabelecimentos ativos de padaria existem na cidade de Matriz de Camaragibe/AL?", &mut Conversation::default())
        .request
        .unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(20)])));
    assert_eq!(r.scope.head_office, Tri::Any);

    let r = engine
        .interpret("Quantos estabelecimentos ativos de padaria existem na cidade de Dois Vizinhos/PR?", &mut Conversation::default())
        .request
        .unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(21)])));

    let r = engine
        .interpret(
            "Quantos estabelecimentos ativos de padaria existem na cidade de Sorriso/MT e nos municípios vizinhos em até 1 passos?",
            &mut Conversation::default(),
        )
        .request
        .unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Near { center: MunicipalityId(1), rule: NearRule::Neighbours(1) });

    // Sub-municipal "no logradouro principal" is not executed as Logradouro/PB even when Logradouro/PB is in the lexicon.
    let res = engine.interpret(
        "Quantos estabelecimentos ativos de padaria existem no logradouro principal de Sorriso/MT?",
        &mut Conversation::default(),
    );
    assert_eq!(res.decision, Decision::Clarify);
    assert!(res.request.is_none());

    // External statistical metrics (IBGE PAM/PPM/CEMPRE, Pearson correlation) are rejected.
    for text in [
        "Qual a correlação de Pearson entre estabelecimentos de padaria e a produção agrícola da PAM IBGE em Sorriso/MT?",
        "Qual a área plantada de soja na PAM IBGE para Sorriso/MT?",
        "Qual o salário médio mensal no CEMPRE IBGE das padarias em Sorriso/MT?",
    ] {
        assert_eq!(parse(text).decision, Decision::Reject, "{text}");
    }

    // Umbrella agro and subjective market-saturation terms trigger clarification.
    for text in ["Quantas empresas de agronegócio existem em Sorriso/MT?", "O mercado de padaria está saturado na cidade de Sorriso/MT?"]
    {
        assert_eq!(parse(text).decision, Decision::Clarify, "{text}");
    }
}

#[test]
fn commercial_round_10_prefix_and_scope_edit_regressions() {
    let mut engine = interpreter();
    engine.pack.municipality(MunicipalityId(30), UfId(25), "Cruzeiro", &[]);
    engine.pack.municipality(MunicipalityId(31), UfId(17), "Cruzeiro do Oeste", &[]);
    engine.pack.municipality(MunicipalityId(32), UfId(0), "Cruzeiro do Sul", &[]);
    engine.pack.municipality(MunicipalityId(33), UfId(22), "Cruzeiro do Sul", &[]);
    engine.pack.municipality(MunicipalityId(34), UfId(22), "Santa Rosa", &[]);
    engine.pack.municipality(MunicipalityId(35), UfId(10), "Santa Rosa da Serra", &[]);
    engine.pack.sector(SectorId(3), &["transportadora".into(), "transportadoras".into()], SmallSet::new([CnaeCode::Subclass(4930201)]));
    engine.pack.municipality(MunicipalityId(36), UfId(10), "Desterro de Entre Rios", &[]);

    // Longest-match preserves the exact single-word municipality and does not
    // widen it to longer names that merely share the prefix.
    let r = engine.interpret("Quantas padarias em Cruzeiro?", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(30)])));
    let r = engine.interpret("Quantas padarias em Cruzeiro do Oeste?", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(31)])));
    assert_eq!(engine.interpret("Quantas padarias em Cruzeiro do Sul?", &mut Conversation::default()).decision, Decision::Clarify);
    let r = engine.interpret("Quantas padarias em Santa Rosa?", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(34)])));
    let r = engine.interpret("Quantas padarias em Santa Rosa da Serra?", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(35)])));

    // Abbreviated list imperative keeps the explicit limit and head-office filter.
    let interpret_list = |question| engine.interpret(question, &mut Conversation::default()).request.unwrap();
    let r = interpret_list("Mostre as matrizes de transportadora em Sorriso");
    assert_eq!(interpret_list("Me mande apenas as matrizes de transportadora em Sorriso"), r);
    assert_eq!(r.op, Op::List { order: ListOrder::Cnpj, limit: 20 });
    assert_eq!(r.scope.head_office, Tri::Yes);
    assert_eq!(
        r.scope.activity,
        ActivityFilter::Sectors {
            ids: SmallSet::new([SectorId(3)]),
            cnaes: SmallSet::new([CnaeCode::Subclass(4930201)]),
            scope: CnaeScope::Main,
        }
    );
    let r =
        engine.interpret("Me passa a lista de 10 transportadoras com telefone em Sorriso", &mut Conversation::default()).request.unwrap();
    assert_eq!(r.op, Op::List { order: ListOrder::Cnpj, limit: 10 });
    assert_eq!(r.scope.contact, ContactNeed::Phone);

    // Multi-turn compound edit: "mantenha matriz" must survive "tire o MEI".
    let mut state = Conversation::default();
    engine.interpret("Quantas padarias ativas existem em Sorriso/MT?", &mut state);
    engine.interpret("Agora só matrizes e sem MEI.", &mut state);
    assert_eq!(state.last.as_ref().unwrap().scope.head_office, Tri::Yes);
    assert_eq!(state.last.as_ref().unwrap().scope.mei, Tri::No);
    let r = engine.interpret("Na verdade mantenha o filtro de matriz, tire o MEI e troque para Sinop/MT.", &mut state).request.unwrap();
    assert_eq!(r.scope.head_office, Tri::Yes);
    assert_eq!(r.scope.mei, Tri::Any);
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)])));
}

#[test]
fn round10_state_name_does_not_trigger_abundance_metaphor() {
    let text = "Compare padarias ativas entre os estados de Minas Gerais e Mato Grosso.";
    let r = req(text);
    assert!(matches!(r.op, Op::Compare { dim: Dim::Uf, .. }));
    assert_eq!(r.scope.place, PlaceFilter::Any);
    assert_eq!(parse("Em Sorriso/MT a padaria e mato mesmo?").decision, Decision::Clarify);
}

#[test]
fn round10_lowercase_para_in_state_list_is_para_uf() {
    let text = "faca a comparacao de padarias ativas entre os estados de para e mato grosso.";
    let r = req(text);
    assert!(matches!(r.op, Op::Compare { dim: Dim::Uf, .. }));
    let Op::Compare { members, .. } = r.op else { panic!("expected compare") };
    assert!(members.contains(&Member::Uf(UfId(13))));
}

#[test]
fn round10_place_name_entre_does_not_force_capital_interval() {
    let mut engine = interpreter();
    engine.pack.municipality(MunicipalityId(36), UfId(10), "Desterro de Entre Rios", &[]);
    let r = engine
        .interpret(
            "Liste 20 padarias ativas no municipio de Desterro de Entre Rios/MG que sejam matriz, com telefone e capital social acima de 50000 reais, ordenados do maior capital para o menor.",
            &mut Conversation::default(),
        )
        .request
        .unwrap();
    assert_eq!(r.scope.capital, Some(Range { from: Some(5_000_001), to: None }));
    assert_eq!(r.op, Op::List { order: ListOrder::CapitalDesc, limit: 20 });
}

#[test]
fn round10_sector_ranking_is_cadastral_domain() {
    let r = req("Quais os 10 setores com maior estoque ativo no Brasil?");
    assert!(matches!(r.op, Op::Rank { dim: Dim::Sector, .. }));
}

#[test]
fn round11_followup_preserves_inherited_place_exclusion() {
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("Quantas padarias ativas existem na cidade de Sorriso/MT?", &mut state);
    engine.interpret("Agora conte o estado de MT inteiro, exceto a cidade de Sorriso/MT.", &mut state);
    assert_eq!(state.last.as_ref().unwrap().scope.exclude.municipalities, SmallSet::new([MunicipalityId(1)]));
    let out = engine.interpret("E liste 10 padarias com telefone nesse estado.", &mut state);
    let r = out.request.unwrap();
    assert_eq!(r.scope.exclude.municipalities, SmallSet::new([MunicipalityId(1)]));
}

#[test]
fn round12_discourse_entre_does_not_require_capital_interval() {
    let r = req(
        "No cruzamento entre porte e capital, quantas padarias ativas em Sorriso/MT sao pequeno porte com capital social acima de 100000 reais?",
    );
    assert_eq!(r.scope.capital, Some(Range { from: Some(10_000_001), to: None }));
    assert_eq!(r.scope.porte, PorteSet::only(Porte::Small));
}

#[test]
fn round13_compound_edit_can_clear_capital_ceiling() {
    let engine = interpreter();
    let mut state = Conversation::default();
    engine.interpret("Quantas padarias ativas existem na cidade de Sorriso/MT?", &mut state);
    engine.interpret("Agora so MEI com capital social de ate 50000 reais em Sorriso/MT.", &mut state);
    assert_eq!(state.last.as_ref().unwrap().scope.capital, Some(Range { from: None, to: Some(5_000_000) }));
    let out = engine.interpret("Na verdade mantenha o MEI, tire o limite de capital e troque para Sinop/MT.", &mut state);
    assert_eq!(out.act, "compound_edit");
    let r = out.request.unwrap();
    assert_eq!(r.scope.capital, None);
    assert_eq!(r.scope.mei, Tri::Yes);
}

#[test]
fn round15_cnae_group_breakdown_is_supported() {
    let r = req("Distribua as padarias ativas em Sorriso/MT por grupo CNAE.");
    assert!(matches!(r.op, Op::Breakdown { dim: Dim::Cnae(CnaeLevel::Group), .. }));
}

fn it_interpreter() -> Interpreter {
    interpreter_with(&[(&["ti", "tecnologia", "tecnologia da informacao", "servicos de ti"], 6201501)])
}

fn year(y: u16) -> DayRange {
    Range { from: Day::from_ymd(y, 1, 1), to: Day::from_ymd(y, 12, 31) }
}

#[test]
fn export_skill_lists_every_closed_company_of_the_question() {
    use opencnpj_query::export::{ExportField, ExportFormat};
    let engine = it_interpreter();
    let text = "me mande um exel com todas as empresas que tiveram baixa em mato grosso no setor de TI";
    let out = engine.interpret(text, &mut Conversation::default());
    let r = out.request.clone().unwrap_or_else(|| panic!("{out:?}")).into_request();
    assert_eq!(r.op, Op::List { order: ListOrder::Cnpj, limit: 20 });
    assert_eq!(r.scope.place, PlaceFilter::Ufs(SmallSet::new([UfId(12)])));
    assert!(matches!(&r.scope.activity, ActivityFilter::Sectors { ids, .. } if ids[..] == [SectorId(3)]));
    assert_eq!(r.scope.status, Some(StatusSet::only(Status::Closed)));
    // No period was named: every closed company, not only this year's closures.
    assert_eq!(r.scope.status_date, None);
    let export = out.export.unwrap();
    assert_eq!(export.format, ExportFormat::Xlsx);
    assert!(export.fields.is_empty());

    let text = "me mande apenas o nome e o numero em um exel com todas as empresas que tiveram baixa em mato grosso no setor de TI";
    let out = engine.interpret(text, &mut Conversation::default());
    assert_eq!(out.request.unwrap().into_request(), r);
    assert_eq!(out.export.unwrap().fields, [ExportField::RazaoSocial, ExportField::Cnpj]);
    assert_eq!(out.decision, Decision::ExecuteWithNote);
    assert!(out.notes.iter().any(|n| n.contains("CNPJ")));
}

#[test]
fn listed_events_become_record_filters() {
    use opencnpj_query::export::ExportFormat;
    let r = req("liste as padarias que fecharam em 2025 em Sorriso");
    assert_eq!(r.scope.status, Some(StatusSet::only(Status::Closed)));
    assert_eq!(r.scope.status_date, Some(year(2025)));
    let out = interpreter().interpret("Quero um CSV das padarias que fecharam em 2025 em Sorriso MT", &mut Conversation::default());
    let r = out.request.unwrap().into_request();
    assert!(matches!(r.op, Op::List { .. }));
    assert_eq!(r.scope.status_date, Some(year(2025)));
    assert_eq!(out.export.unwrap().format, ExportFormat::Csv);
    let out = interpreter().interpret("Liste as padarias abertas em 2025 em Sorriso MT em txt", &mut Conversation::default());
    let r = out.request.unwrap().into_request();
    assert_eq!(r.scope.opened, Some(year(2025)));
    assert_eq!(r.scope.status, Some(StatusSet::ALL));
    assert_eq!(out.export.unwrap().format, ExportFormat::Txt);
}

#[test]
fn export_follow_ups_reuse_the_previous_answer() {
    use opencnpj_query::export::{ExportField, ExportFormat};
    let engine = interpreter();
    let mut state = Conversation::default();
    let count = engine.interpret("Quantas padarias tiveram baixa em Sorriso em 2025?", &mut state).request.unwrap().into_request();
    assert!(matches!(count.op, Op::Count { metric: Metric::Flow(FlowMetric::Closures) }));
    let out = engine.interpret("agora me mande isso em excel", &mut state);
    assert_eq!(out.act, "export");
    let list = out.request.unwrap().into_request();
    assert!(matches!(list.op, Op::List { .. }));
    assert_eq!(list.scope.place, count.scope.place);
    assert_eq!(list.scope.status_date, Some(Range { from: Day::from_ymd(2025, 1, 1), to: Day::from_ymd(2025, 12, 31) }));
    assert_eq!(out.export.as_ref().unwrap().format, ExportFormat::Xlsx);
    let out = engine.interpret("só o nome e o CNPJ", &mut state);
    assert_eq!(out.request.unwrap().into_request(), list);
    let export = out.export.unwrap();
    assert_eq!(export.format, ExportFormat::Xlsx);
    assert_eq!(export.fields, [ExportField::RazaoSocial, ExportField::Cnpj]);
    let out = engine.interpret("e em csv", &mut state);
    let export = out.export.unwrap();
    assert_eq!(export.format, ExportFormat::Csv);
    assert_eq!(export.fields, [ExportField::RazaoSocial, ExportField::Cnpj]);
    let masculine = engine.interpret("liste os estabelecimentos baixados em 2025 em Sorriso", &mut Conversation::default());
    assert_eq!(masculine.request.unwrap().scope.status_date, Some(year(2025)));
    // A new question starts without a file.
    assert!(engine.interpret("Quantas farmacias ativas existem em Sinop?", &mut state).export.is_none());
}

#[test]
fn analytical_answers_export_their_table_and_rows_are_kept() {
    use opencnpj_query::export::ExportFormat;
    let out = interpreter().interpret("Exporte em excel o ranking das cidades de MT com mais padarias", &mut Conversation::default());
    assert!(matches!(out.request.unwrap().into_request().op, Op::Rank { dim: Dim::Municipality, .. }));
    assert_eq!(out.export.unwrap().format, ExportFormat::Xlsx);
    let out = interpreter().interpret("planilha com as 300 padarias mais recentes de Sorriso MT", &mut Conversation::default());
    assert_eq!(out.request.unwrap().into_request().op, Op::List { order: ListOrder::NewestFirst, limit: 100 });
    assert_eq!(out.export.unwrap().rows, Some(300));
    let out = interpreter().interpret("me mande um excel", &mut Conversation::default());
    assert_eq!(out.decision, Decision::Clarify);
}

#[test]
fn companies_are_found_by_name() {
    use crate::policy::CompanySearch;
    let company = |text: &str| parse(text).company;
    let named = |name: &str, municipalities: &[u16]| {
        Some(CompanySearch {
            name: name.into(),
            municipalities: municipalities.iter().map(|id| MunicipalityId(*id)).collect(),
            ufs: Vec::new(),
        })
    };
    assert_eq!(company("encontre a empresa zaflas"), named("zaflas", &[]));
    assert_eq!(company("dados da empresa Zaflas"), named("Zaflas", &[]));
    assert_eq!(company("zaflas"), named("zaflas", &[]));
    assert_eq!(company("Qual o CNPJ da Padaria Pão Quente em Sorriso?"), named("Padaria Pão Quente", &[1]));
    assert_eq!(
        company("Bah, me fala sobre a firma Zaflas em MT"),
        Some(CompanySearch { name: "Zaflas".into(), municipalities: Vec::new(), ufs: vec![UfId(12)] })
    );
    let found = parse("encontre a empresa zaflas");
    assert_eq!((found.decision, found.act.as_str()), (Decision::Execute, "company_search"));
    assert!(found.request.is_none());
    assert!(matches!(req("Dados do CNPJ 11.739.229/0001-69").op, Op::Lookup { ref id } if id.as_str() == "11739229000169"));
}

#[test]
fn company_search_keeps_refusals_and_activity_lists() {
    // An activity is listed, not looked up as a name.
    assert!(matches!(req("busque padarias em Sorriso").op, Op::List { .. }));
    for text in ["encontre o dono da zaflas", "qual o faturamento da zaflas", "bom dia", "Consulte o CNPJ 33.000.167/0001-00"] {
        let out = parse(text);
        assert!(out.company.is_none() && out.request.is_none(), "{text}: {out:?}");
    }
    // Data questions the grammar cannot read yet are not company names either.
    for text in ["empresas ativas em Sorriso", "nr de padarias em Sorriso"] {
        assert!(parse(text).company.is_none(), "{text}");
    }
    // Cadastral questions are unchanged.
    assert!(parse("quantas padarias ativas existem em Sorriso").company.is_none());
}

#[test]
fn an_unknown_kind_of_company_is_asked_not_dropped() {
    // "agricolas" was once ignored and every company of the city was listed.
    for text in [
        "liste as empresas xyzolicas de Sorriso",
        "quantas empresas xyzolicas existem em Sorriso",
        "me mande um excel das empresas xyzolicas de Sorriso",
    ] {
        let out = parse(text);
        assert_eq!(out.decision, Decision::Clarify, "{text}: {out:?}");
        assert!(out.request.is_none());
    }
    for text in [
        "liste as empresas ativas de Sorriso",
        "quantas empresas abertas em 2025 em Sorriso",
        "liste as empresas de padaria de Sorriso",
        "quantas empresa abriu em Sorriso esse ano",
    ] {
        assert!(parse(text).request.is_some(), "{text}");
    }
}

#[test]
fn status_words_are_never_read_as_their_opposite() {
    assert_eq!(req("liste as padarias fechadas em Sorriso").scope.status, Some(StatusSet::only(Status::Closed)));
    assert_eq!(req("quantas padarias encerradas existem em Sorriso").scope.status, Some(StatusSet::only(Status::Closed)));
    let inactive = parse("quantas padarias inativas existem em Sorriso");
    assert_eq!(inactive.request.unwrap().scope.status, Some(StatusSet::new([Status::Suspended, Status::Unfit, Status::Closed])));
    assert!(!inactive.notes.is_empty());
    // A legal form, not a status.
    assert_eq!(
        req("quantas padarias ativas de sociedade anonima fechada existem em Sorriso").scope.status,
        Some(StatusSet::only(Status::Active))
    );
}

#[test]
fn a_list_period_without_an_event_is_asked() {
    for text in ["liste as padarias de 2025 em Sorriso", "liste as farmacias em Sorriso desde 2020"] {
        assert_eq!(parse(text).decision, Decision::Clarify, "{text}");
    }
    assert!(req("liste as padarias abertas em 2025 em Sorriso").scope.opened.is_some());
}

#[test]
fn explicit_row_counts_are_kept_or_refused_not_truncated() {
    let out = interpreter().interpret("me mande um excel com as 5000 padarias de Sorriso", &mut Conversation::default());
    assert_eq!(out.export.unwrap().rows, Some(5000));
    // A chat list above the plan limit is refused with a clamped option.
    let out = parse("liste 5000 padarias de Sorriso");
    assert!(out.request.is_none());
    assert!(!out.options.is_empty());
}

#[test]
fn a_state_after_do_qualifies_the_city() {
    // "Bom Jesus" is id 3 in PI and id 4 in RS in the test pack.
    let r = req("quantas padarias tem Bom Jesus do Piauí");
    assert_eq!(r.scope.place, PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(3)])));
}

#[test]
fn followup_lists_keep_filters_order_and_explicit_rows() {
    use opencnpj_query::export::ExportFormat;

    let engine = interpreter();
    let mut state = Conversation::default();
    let first = engine.interpret(
        "Liste 5 matrizes de farmácias ativas em Sorriso MT e Sinop MT, de pequeno porte, optantes pelo Simples, sem MEI, com telefone e e-mail e capital social acima de 20 mil reais, ordenadas pelo maior capital.",
        &mut state,
    );
    let mut expected = first.request.expect("initial list").into_request();
    assert_eq!(expected.op, Op::List { order: ListOrder::CapitalDesc, limit: 5 });
    assert_eq!(expected.scope.contact, ContactNeed::Both);
    assert_eq!(expected.scope.porte, PorteSet::only(Porte::Small));
    assert_eq!(expected.scope.simples, Tri::Yes);
    assert_eq!(expected.scope.mei, Tri::No);
    assert_eq!(expected.scope.head_office, Tri::Yes);
    assert_eq!(expected.scope.capital, Some(Range { from: Some(2_000_001), to: None }));
    expected.scope.place = PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)]));

    let mut same_turn = state.clone();
    let list = engine.interpret("dessas só as de Sinop", &mut state);
    assert_eq!(list.request.unwrap().into_request(), expected);
    assert!(list.export.is_none());
    for (question, format) in [("me manda em Excel", ExportFormat::Xlsx), ("agora em CSV", ExportFormat::Csv)] {
        let result = engine.interpret(question, &mut state);
        assert_eq!(result.request.unwrap().into_request(), expected, "{question}");
        let export = result.export.unwrap();
        assert_eq!(export.format, format, "{question}");
        assert_eq!(export.rows, Some(5), "{question}");
    }
    let result = engine.interpret("dessas, só as de Sinop em Excel", &mut same_turn);
    assert_eq!(result.request.unwrap().into_request(), expected);
    assert_eq!(result.export.unwrap().rows, Some(5));

    let mut state = Conversation::default();
    let paged = engine.interpret("Liste 20 padarias em Sorriso com e-mail na segunda página", &mut state);
    let mut expected = paged.request.unwrap().into_request();
    assert_eq!(expected.output.page, 2);
    expected.output.page = 1;
    expected.scope.place = PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)]));
    let next = engine.interpret("só as de Sinop", &mut state);
    assert_eq!(next.request.unwrap().into_request(), expected);
    assert_eq!(engine.interpret("me manda em Excel", &mut state).export.unwrap().rows, Some(20));
    expected.op = Op::List { order: ListOrder::Cnpj, limit: 3 };
    let shorter = engine.interpret("me manda em Excel só 3", &mut state);
    assert_eq!(shorter.request.unwrap().into_request(), expected);
    assert_eq!(shorter.export.unwrap().rows, Some(3));
    assert_eq!(engine.interpret("agora em CSV", &mut state).export.unwrap().rows, Some(3));
}

#[test]
fn followup_contacts_and_dates_keep_the_rest_of_the_query() {
    let engine = interpreter();
    for (start, followup) in [
        ("Liste farmácias ativas em Sorriso MT com telefone.", "dessas, só as que têm e-mail cadastrado"),
        ("Quantas farmácias ativas com telefone em Sorriso MT?", "e com e-mail?"),
    ] {
        let mut state = Conversation::default();
        let mut expected = engine.interpret(start, &mut state).request.unwrap().into_request();
        expected.scope.contact = ContactNeed::Both;
        let added = engine.interpret(followup, &mut state);
        assert_eq!(added.request.unwrap().into_request(), expected, "{followup}");
        expected.scope.place = PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)]));
        let moved = engine.interpret("dessas só as de Sinop", &mut state);
        assert_eq!(moved.request.unwrap().into_request(), expected);
        if matches!(expected.op, Op::List { .. }) {
            expected.scope.opened = Some(year(2025));
            let dated = engine.interpret("dessas só as abertas em 2025", &mut state);
            assert_eq!(dated.request.unwrap().into_request(), expected);
        }
        expected.op = Op::List { order: ListOrder::Cnpj, limit: 20 };
        let file = engine.interpret("me manda em Excel", &mut state);
        assert_eq!(file.request.unwrap().into_request(), expected);
        assert_eq!(file.export.unwrap().rows, None);
    }
}

#[test]
fn followup_export_preserves_columns_after_refinements_and_errors() {
    use opencnpj_query::export::{ExportField, ExportFormat};

    let engine = interpreter();
    let mut state = Conversation::default();
    let first = engine
        .interpret("Me manda um CSV com as colunas nome fantasia e CNPJ de 7 farmácias ativas em Sorriso MT com telefone.", &mut state);
    let mut expected = first.request.unwrap().into_request();
    let original_file = first.export.unwrap();
    for question in ["dessas só as de Atlântida MT", "dessas só as de Atlântida MT em Excel"] {
        let invalid = engine.interpret(question, &mut state);
        assert_eq!(invalid.decision, Decision::Clarify, "{question}: {invalid:?}");
        assert!(invalid.request.is_none() && invalid.export.is_none());
        assert_eq!(state.last.as_ref(), Some(&expected));
        assert_eq!(state.export.as_ref(), Some(&original_file));
    }

    expected.scope.place = PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)]));
    let next = engine.interpret("dessas só as de Sinop em Excel", &mut state);
    assert_eq!(next.request.unwrap().into_request(), expected);
    let export = next.export.unwrap();
    assert_eq!(export.format, ExportFormat::Xlsx);
    assert_eq!(export.rows, Some(7));
    assert_eq!(export.fields, [ExportField::NomeFantasia, ExportField::Cnpj]);

    let columns = engine.interpret("só o CNPJ", &mut state);
    assert_eq!(columns.request.unwrap().into_request(), expected);
    let export = columns.export.unwrap();
    assert_eq!(export.rows, Some(7));
    assert_eq!(export.fields, [ExportField::Cnpj]);
    let new_query = engine.interpret("Nova pesquisa: quantas farmácias ativas existem em Sinop MT?", &mut state);
    assert!(new_query.export.is_none() && state.export.is_none() && state.list_rows.is_none());
    assert_eq!(new_query.request.unwrap().scope.contact, ContactNeed::Any);

    for text in ["dessas só as de Sinop", "me manda em Excel"] {
        let result = engine.interpret(text, &mut Conversation::default());
        assert_eq!(result.decision, Decision::Clarify, "{text}: {result:?}");
        assert!(result.request.is_none() && result.company.is_none(), "{text}");
    }
}

#[test]
fn followup_flow_keeps_status_period_and_corresponding_export_date() {
    let engine = interpreter();
    for (start, metric) in [
        ("Quantas padarias abriram em Sorriso MT em 2025 e continuam ativas?", FlowMetric::Registrations),
        ("Quantas padarias tiveram baixa em Sorriso MT em 2025?", FlowMetric::Closures),
    ] {
        let mut state = Conversation::default();
        let mut expected = engine.interpret(start, &mut state).request.unwrap().into_request();
        assert_eq!(expected.op, Op::Count { metric: Metric::Flow(metric) });
        assert_eq!(expected.time.period, Some(Period::Days(year(2025))));
        let status = if metric == FlowMetric::Registrations { Status::Active } else { Status::Closed };
        assert_eq!(expected.scope.status, Some(StatusSet::only(status)));
        expected.scope.place = PlaceFilter::Municipalities(SmallSet::new([MunicipalityId(2)]));
        let next = engine.interpret("dessas só as de Sinop", &mut state);
        assert_eq!(next.request.unwrap().into_request(), expected, "{start}");
        expected.op = Op::List { order: ListOrder::Cnpj, limit: 20 };
        expected.time.period = None;
        if metric == FlowMetric::Registrations {
            expected.scope.opened = Some(year(2025));
        } else {
            expected.scope.status_date = Some(year(2025));
        }
        let file = engine.interpret("me manda em Excel", &mut state);
        assert_eq!(file.request.unwrap().into_request(), expected, "{start}");
    }
}

#[test]
fn contact_edits_preserve_the_query_and_export_selection() {
    use opencnpj_query::export::ExportFormat;

    let engine = interpreter();
    for (contacts, edit, expected_contact) in [
        ("telefone", "tira o filtro de telefone", ContactNeed::Any),
        ("telefone e e-mail", "tira o filtro de telefone", ContactNeed::Email),
        ("telefone e e-mail", "retire o filtro de e-mail", ContactNeed::Phone),
        ("telefone", "troca telefone por e-mail", ContactNeed::Email),
        ("telefone e e-mail", "Agora, substitua wpp por e-mail", ContactNeed::Email),
        ("telefone e e-mail", "Remove os filtros de telefone e e-mail", ContactNeed::Any),
        ("telefone e e-mail", "Pode tirar o filtro de contato, mantendo os outros filtros?", ContactNeed::Any),
        ("telefone e e-mail", "Retire o filtro de telefone e mantenha o porte", ContactNeed::Email),
        ("e-mail", "tira o filtro de telefone", ContactNeed::Email),
    ] {
        let mut state = Conversation::default();
        let initial = format!(
            "Liste 5 matrizes de farmácias ativas em Sinop MT, abertas em 2025, de pequeno porte, optantes pelo Simples, sem MEI, com {contacts}, capital social acima de 20 mil reais, ordenadas pelo maior capital."
        );
        let first = engine.interpret(&initial, &mut state);
        let mut expected = first.request.unwrap_or_else(|| panic!("{initial}: {}", first.reason)).into_request();
        assert_eq!(expected.op, Op::List { order: ListOrder::CapitalDesc, limit: 5 });
        assert_eq!(expected.scope.head_office, Tri::Yes);
        assert_eq!(expected.scope.simples, Tri::Yes);
        assert_eq!(expected.scope.mei, Tri::No);
        assert_eq!(expected.scope.porte, PorteSet::only(Porte::Small));
        assert_eq!(expected.scope.opened, Some(year(2025)));
        expected.scope.contact = expected_contact;
        let result = engine.interpret(edit, &mut state);
        assert_eq!(result.request.unwrap_or_else(|| panic!("{edit}: {}", result.reason)).into_request(), expected, "{edit}");
        let result = engine.interpret("me manda em Excel", &mut state);
        assert_eq!(result.request.unwrap().into_request(), expected, "{edit}");
        let file = result.export.unwrap();
        assert_eq!(file.format, ExportFormat::Xlsx, "{edit}");
        assert_eq!(file.rows, Some(5), "{edit}");
    }

    let mut state = Conversation::default();
    let first = engine
        .interpret("Quantas matrizes de farmácias ativas em Sinop MT têm telefone e capital social acima de 20 mil reais?", &mut state);
    let mut expected = first.request.unwrap().into_request();
    assert_eq!(expected.op, Op::Count { metric: Metric::Stock });
    expected.scope.contact = ContactNeed::Email;
    let edited = engine.interpret("troca o telefone por e-mail", &mut state);
    assert_eq!(edited.request.unwrap().into_request(), expected);
    expected.op = Op::List { order: ListOrder::Cnpj, limit: 20 };
    let file = engine.interpret("me manda em Excel", &mut state);
    assert_eq!(file.request.unwrap().into_request(), expected);
    assert_eq!(file.export.unwrap().rows, None);
}

#[test]
fn contact_edits_keep_file_columns_and_reject_ambiguous_changes() {
    use opencnpj_query::export::{ExportField, ExportFormat};

    let engine = interpreter();
    let mut state = Conversation::default();
    let first = engine.interpret(
        "Me manda um CSV com as colunas nome fantasia e telefone de 7 farmácias ativas em Sinop MT com telefone e e-mail.",
        &mut state,
    );
    let mut expected = first.request.unwrap().into_request();
    let csv = first.export.unwrap();
    assert_eq!(csv.fields, [ExportField::NomeFantasia, ExportField::Telefone]);
    assert_eq!(csv.rows, Some(7));

    for question in [
        "Troca telefone por",
        "Tire a coluna telefone",
        "dessas, só as sem telefone",
        "tira telefone e mantém o filtro de telefone",
        "tira filtro de telefone ou e-mail",
    ] {
        let result = engine.interpret(question, &mut state);
        assert_eq!(result.decision, Decision::Clarify, "{question}: {result:?}");
        assert!(result.request.is_none() && result.export.is_none(), "{question}");
        assert_eq!(state.last.as_ref(), Some(&expected), "{question}");
        assert_eq!(state.export.as_ref(), Some(&csv), "{question}");
    }
    expected.scope.contact = ContactNeed::Email;
    let edited = engine.interpret("Tira o filtro de telefone e mantém o de e-mail", &mut state);
    assert_eq!(edited.request.unwrap().into_request(), expected);
    assert_eq!(edited.export.as_ref(), Some(&csv));

    expected.scope.contact = ContactNeed::Phone;
    let excel = engine.interpret("Troque e-mail por tel. e me mande em Excel", &mut state);
    assert_eq!(excel.request.unwrap().into_request(), expected);
    let excel = excel.export.unwrap();
    assert_eq!(excel.format, ExportFormat::Xlsx);
    assert_eq!(excel.fields, csv.fields);
    assert_eq!(excel.rows, csv.rows);

    for question in ["tira o filtro de telefone", "troca telefone por e-mail"] {
        let result = engine.interpret(question, &mut Conversation::default());
        assert_eq!(result.decision, Decision::Clarify, "{question}: {result:?}");
        assert!(result.request.is_none() && result.company.is_none());
    }
    let mut state = Conversation::default();
    engine.interpret("Liste farmácias ativas em Sinop", &mut state);
    let result = engine.interpret("troca telefone por e-mail", &mut state);
    assert_eq!(result.decision, Decision::Clarify);
    assert_eq!(state.last.unwrap().scope.contact, ContactNeed::Any);
}

#[test]
fn size_and_unit_edits_preserve_the_remaining_filters_and_export() {
    use opencnpj_query::export::ExportFormat;

    let engine = interpreter();
    for (unit, simples, edit, size, head_office) in [
        ("matrizes", "optantes pelo Simples", "tira o filtro de porte e mantém o Simples", PorteSet::ALL, Tri::Yes),
        ("matrizes", "fora do Simples", "Tira o porte, mantendo o Simples Nacional", PorteSet::ALL, Tri::Yes),
        ("matrizes", "optantes pelo Simples", "Tire o filtro de porte e mantenha o capital social", PorteSet::ALL, Tri::Yes),
        ("matrizes", "optantes pelo Simples", "Pode retirar o porte, mantendo as matrizes?", PorteSet::ALL, Tri::Yes),
        ("matrizes", "optantes pelo Simples", "troca matriz por filial", PorteSet::only(Porte::Small), Tri::No),
        ("filiais", "optantes pelo Simples", "Troque as filiais pelas matrizes", PorteSet::only(Porte::Small), Tri::Yes),
        ("matrizes", "fora do Simples", "Troque matriz por filial, mantendo o Simples", PorteSet::only(Porte::Small), Tri::No),
    ] {
        let initial = format!(
            "Liste 5 {unit} de farmácias ativas em Sinop MT, abertas em 2025, de pequeno porte, {simples}, sem MEI, com telefone e e-mail, capital social acima de 20 mil reais, ordenadas pelo maior capital."
        );
        let mut state = Conversation::default();
        let first = engine.interpret(&initial, &mut state);
        let mut expected = first.request.unwrap_or_else(|| panic!("{initial}: {}", first.reason)).into_request();
        assert_eq!(expected.op, Op::List { order: ListOrder::CapitalDesc, limit: 5 });
        assert_eq!(expected.scope.capital, Some(Range { from: Some(2_000_001), to: None }));
        assert_eq!(expected.scope.porte, PorteSet::only(Porte::Small));
        assert_eq!(expected.scope.simples, if simples == "fora do Simples" { Tri::No } else { Tri::Yes });
        assert_eq!(expected.scope.mei, Tri::No);
        assert_eq!(expected.scope.contact, ContactNeed::Both);
        assert_eq!(expected.scope.opened, Some(year(2025)));
        expected.scope.porte = size;
        expected.scope.head_office = head_office;
        let edited = engine.interpret(edit, &mut state);
        assert_eq!(edited.request.unwrap_or_else(|| panic!("{edit}: {}", edited.reason)).into_request(), expected, "{edit}");
        let file = engine.interpret("me manda em Excel", &mut state);
        assert_eq!(file.request.unwrap().into_request(), expected, "{edit}");
        let export = file.export.unwrap();
        assert_eq!(export.format, ExportFormat::Xlsx, "{edit}");
        assert_eq!(export.rows, Some(5), "{edit}");
    }

    let mut state = Conversation::default();
    let first =
        engine.interpret("Quantas matrizes de farmácias ativas em Sinop MT têm e-mail e capital social acima de 20 mil reais?", &mut state);
    let mut expected = first.request.unwrap().into_request();
    assert_eq!(expected.unit, Unit::HeadOffices);
    expected.unit = Unit::Establishments;
    expected.scope.head_office = Tri::No;
    let result = engine.interpret("troca matriz por filial", &mut state);
    assert_eq!(result.request.unwrap().into_request(), expected);
    expected.op = Op::List { order: ListOrder::Cnpj, limit: 20 };
    let file = engine.interpret("me manda em Excel", &mut state);
    assert_eq!(file.request.unwrap().into_request(), expected);
    assert_eq!(file.export.unwrap().rows, None);
}

#[test]
fn size_and_unit_edits_distinguish_file_columns_and_clarify_conflicts() {
    use opencnpj_query::export::{ExportField, ExportFormat};

    let engine = interpreter();
    let mut state = Conversation::default();
    let first = engine.interpret(
        "Me manda um CSV com as colunas nome fantasia e porte de 7 farmácias ativas de pequeno porte em Sinop MT com telefone e e-mail.",
        &mut state,
    );
    let mut expected = first.request.unwrap().into_request();
    let csv = first.export.unwrap();
    assert_eq!(csv.fields, [ExportField::NomeFantasia, ExportField::Porte]);
    for question in [
        "Troca matriz por",
        "troca matriz por filial",
        "Tira a coluna porte",
        "Tira o filtro de porte e mantém o porte",
        "Tira o filtro de porte e mantém o Simples",
    ] {
        let result = engine.interpret(question, &mut state);
        assert_eq!(result.decision, Decision::Clarify, "{question}: {result:?}");
        assert!(result.request.is_none() && result.export.is_none());
        assert_eq!(state.last.as_ref(), Some(&expected), "{question}");
        assert_eq!(state.export.as_ref(), Some(&csv), "{question}");
    }
    expected.scope.porte = PorteSet::ALL;
    let file = engine.interpret("Remove o porte e mantém o restante e me manda em Excel", &mut state);
    assert_eq!(file.request.unwrap().into_request(), expected);
    let export = file.export.unwrap();
    assert_eq!(export.format, ExportFormat::Xlsx);
    assert_eq!(export.fields, csv.fields);
    assert_eq!(export.rows, Some(7));

    for question in ["tira o filtro de porte e mantém o Simples", "troca matriz por filial"] {
        let result = engine.interpret(question, &mut Conversation::default());
        assert_eq!(result.decision, Decision::Clarify, "{question}: {result:?}");
        assert!(result.request.is_none() && result.company.is_none());
    }

    let mut state = Conversation::default();
    let first = engine.interpret(
        "Liste matrizes de farmácias ativas de pequeno porte em Sinop MT, optantes pelo Simples, sem MEI, com telefone",
        &mut state,
    );
    let mut expected = first.request.unwrap().into_request();
    expected.scope.head_office = Tri::No;
    let file = engine.interpret("Substitua matriz por filial e mantenha o Simples e me mande em Excel", &mut state);
    assert_eq!(file.request.unwrap().into_request(), expected);
    assert_eq!(file.export.unwrap().format, ExportFormat::Xlsx);
    expected.scope.porte = PorteSet::ALL;
    expected.scope.mei = Tri::Any;
    let edited = engine.interpret("Tire o porte e o MEI", &mut state);
    assert_eq!(edited.request.unwrap().into_request(), expected);
}

#[test]
fn capital_and_simples_edits_preserve_the_remaining_query_and_export() {
    use opencnpj_query::export::ExportFormat;

    let engine = interpreter();
    for (regime, edit, capital_removed, simples) in [
        ("optantes pelo Simples", "tira o filtro de capital e mantém o porte", true, Tri::Yes),
        ("optantes pelo Simples", "Retire o limite de capital, mantendo o porte", true, Tri::Yes),
        ("fora do Simples", "Tire o filtro de capital social e mantenha o Simples Nacional", true, Tri::No),
        ("optantes pelo Simples", "troca optantes do Simples por não optantes", false, Tri::No),
        ("fora do Simples", "Troque os não optantes pelo Simples Nacional pelos optantes", false, Tri::Yes),
        (
            "optantes pelo Simples",
            "Substitua optantes do Simples Nacional por quem está fora do Simples, mantendo o capital social",
            false,
            Tri::No,
        ),
    ] {
        let initial = format!(
            "Liste 5 matrizes de farmácias ativas em Sinop MT, abertas em 2025, de pequeno porte, {regime}, sem MEI, com telefone e e-mail, capital social entre 20 mil e 200 mil reais, ordenadas pelo maior capital."
        );
        let mut state = Conversation::default();
        let first = engine.interpret(&initial, &mut state);
        let mut expected = first.request.unwrap_or_else(|| panic!("{initial}: {}", first.reason)).into_request();
        assert_eq!(expected.op, Op::List { order: ListOrder::CapitalDesc, limit: 5 });
        assert_eq!(expected.scope.porte, PorteSet::only(Porte::Small));
        assert_eq!(expected.scope.head_office, Tri::Yes);
        assert_eq!(expected.scope.contact, ContactNeed::Both);
        assert_eq!(expected.scope.opened, Some(year(2025)));
        assert_eq!(expected.scope.capital, Some(Range { from: Some(2_000_000), to: Some(20_000_000) }));
        if capital_removed {
            expected.scope.capital = None;
        }
        expected.scope.simples = simples;
        // Applying the same edit again should leave the selection unchanged.
        for _ in 0..2 {
            let edited = engine.interpret(edit, &mut state);
            assert_eq!(edited.request.unwrap_or_else(|| panic!("{edit}: {}", edited.reason)).into_request(), expected, "{edit}");
        }
        let file = engine.interpret("me manda em Excel", &mut state);
        assert_eq!(file.request.unwrap().into_request(), expected, "{edit}");
        let export = file.export.unwrap();
        assert_eq!(export.format, ExportFormat::Xlsx);
        assert_eq!(export.rows, Some(5));
    }

    let mut state = Conversation::default();
    let first = engine.interpret(
        "Quantas matrizes de farmácias ativas em Sinop MT optantes pelo Simples têm e-mail e capital social acima de 20 mil reais?",
        &mut state,
    );
    let mut expected = first.request.unwrap().into_request();
    assert_eq!(expected.unit, Unit::HeadOffices);
    assert_eq!(expected.scope.simples, Tri::Yes);
    expected.scope.simples = Tri::No;
    let edited = engine.interpret("troca optantes do Simples por não optantes", &mut state);
    assert_eq!(edited.request.unwrap().into_request(), expected);
    expected.op = Op::List { order: ListOrder::Cnpj, limit: 20 };
    let file = engine.interpret("me manda em Excel", &mut state);
    assert_eq!(file.request.unwrap().into_request(), expected);
    assert_eq!(file.export.unwrap().rows, None);
}

#[test]
fn capital_and_simples_edits_keep_file_columns_and_clarify_conflicts() {
    use opencnpj_query::export::{ExportField, ExportFormat};

    let engine = interpreter();
    let mut state = Conversation::default();
    let first = engine.interpret(
        "Me manda um CSV com as colunas nome fantasia e capital social de 7 farmácias ativas de pequeno porte em Sinop MT, optantes pelo Simples, com e-mail e capital social acima de 20 mil reais.",
        &mut state,
    );
    let mut expected = first.request.unwrap().into_request();
    let csv = first.export.unwrap();
    assert_eq!(csv.fields, [ExportField::NomeFantasia, ExportField::CapitalSocial]);
    for question in [
        "Troca optantes do Simples por",
        "Tira o filtro de capital e mantém o capital",
        "Tira a coluna capital social",
        "Troque optantes por não optantes",
        "Troque optantes do Simples por não optantes e mantenha o Simples",
    ] {
        let result = engine.interpret(question, &mut state);
        assert_eq!(result.decision, Decision::Clarify, "{question}: {result:?}");
        assert!(result.request.is_none() && result.export.is_none());
        assert_eq!(state.last.as_ref(), Some(&expected), "{question}");
        assert_eq!(state.export.as_ref(), Some(&csv), "{question}");
    }
    expected.scope.capital = None;
    let edited = engine.interpret("Remove o filtro de capital social e mantém o porte e me manda em Excel", &mut state);
    assert_eq!(edited.request.unwrap().into_request(), expected);
    let file = edited.export.unwrap();
    assert_eq!(file.format, ExportFormat::Xlsx);
    assert_eq!(file.fields, csv.fields);
    assert_eq!(file.rows, Some(7));
    expected.scope.simples = Tri::No;
    let edited = engine.interpret("Troque optantes pelo Simples por não optantes e mantenha o porte e me mande em Excel", &mut state);
    assert_eq!(edited.request.unwrap().into_request(), expected);
    let file = edited.export.unwrap();
    assert_eq!(file.format, ExportFormat::Xlsx);
    assert_eq!(file.fields, csv.fields);
    assert_eq!(file.rows, Some(7));

    for question in ["tira o filtro de capital e mantém o porte", "troca optantes do Simples por não optantes"] {
        let result = engine.interpret(question, &mut Conversation::default());
        assert_eq!(result.decision, Decision::Clarify, "{question}: {result:?}");
        assert!(result.request.is_none() && result.company.is_none());
    }
    let mut state = Conversation::default();
    let first = engine.interpret("Liste farmácias de pequeno porte em Sinop MT com telefone", &mut state);
    let expected = first.request.unwrap().into_request();
    let result = engine.interpret("troca optantes do Simples por não optantes", &mut state);
    assert_eq!(result.decision, Decision::Clarify);
    assert_eq!(state.last.as_ref(), Some(&expected));
}
