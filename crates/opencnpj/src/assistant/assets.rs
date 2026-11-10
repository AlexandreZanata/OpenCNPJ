//! Assets of the assistant: catalogues, interpreter, aggregates and snapshot, loaded once.

use super::engine as query;
use anyhow::{Context, Result, ensure};
use opencnpj_ingest::source::sha256_file;
use opencnpj_insights::{
    agg,
    catalog::municipalities::{MunicipalityCatalogue, Place},
    execute::{self, AggregateSet, Value},
    render::SectorLabel,
};
use opencnpj_nlu::{Interpreter, context::Conversation, lang::LanguagePack, policy::Interpretation};
use opencnpj_query::{ir::*, validate::ValidRequest};
use opencnpj_store::Snapshot;
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(clap::Args)]
pub struct DataArgs {
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/municipalities.tsv")]
    pub catalog: PathBuf,
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub sectors: PathBuf,
    #[arg(long, default_value = "2026-10-11")]
    pub anchor: String,
    #[arg(long)]
    pub agg: Option<PathBuf>,
    #[arg(long)]
    pub history: Option<PathBuf>,
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
}

pub struct Assets {
    pub catalogue: MunicipalityCatalogue,
    pub sectors: execute::SectorMap,
    /// Display names by sector id, for rendered answers.
    pub sector_labels: BTreeMap<u16, SectorLabel>,
    pub interpreter: Interpreter,
    pub aggregates: Option<AggregateSet>,
    pub snapshot: Option<Snapshot>,
    pub anchor: Day,
    pub snapshot_sha256: Option<String>,
}

#[derive(Deserialize)]
struct SectorFile {
    sector: Vec<Sector>,
}
#[derive(Deserialize)]
struct Sector {
    aliases: Vec<String>,
    display: Display,
    cnae_core: Vec<String>,
}
#[derive(Deserialize)]
struct Display {
    sing: String,
    plural: String,
    #[serde(default)]
    gender: String,
}

impl Assets {
    pub fn open(args: &DataArgs) -> Result<Self> {
        let parts = args.anchor.split('-').map(str::parse::<u16>).collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(parts.len() == 3, "anchor must be YYYY-MM-DD");
        let anchor = Day::from_ymd(parts[0], u8::try_from(parts[1])?, u8::try_from(parts[2])?).context("invalid data anchor")?;
        let catalogue = MunicipalityCatalogue::load(&args.catalog)?;
        let sectors = query::load_sectors(&args.sectors)?;
        let mut pack = LanguagePack::new();
        for row in catalogue.rows() {
            if let Place::Municipality(geo) = &row.place {
                pack.municipality(MunicipalityId(row.id), agg::uf_id(&geo.uf).context("UF")?, &row.name, &row.variants);
            }
        }
        let file: SectorFile = toml::from_str(&fs::read_to_string(&args.sectors)?)?;
        let mut sector_labels = BTreeMap::new();
        for (i, s) in file.sector.into_iter().enumerate() {
            sector_labels.insert(
                u16::try_from(i)?,
                SectorLabel { singular: s.display.sing.clone(), plural: s.display.plural.clone(), feminine: s.display.gender == "f" },
            );
            let mut aliases = s.aliases;
            aliases.extend([s.display.sing, s.display.plural]);
            pack.sector(
                SectorId(u16::try_from(i)?),
                &aliases,
                SmallSet::new(
                    s.cnae_core.iter().map(|c| c.parse::<u32>().map(CnaeCode::Subclass)).collect::<std::result::Result<Vec<_>, _>>()?,
                ),
            );
        }
        let aggregates = args.agg.as_ref().map(|p| AggregateSet::open(p, args.history.as_deref())).transpose()?;
        if let Some(agg) = &aggregates {
            ensure!(sha256_file(&args.catalog)? == agg.current().manifest().municipality_sha256, "aggregate/catalogue identity mismatch");
        }
        let snapshot = args.snapshot.as_ref().map(|path| Snapshot::open(path)).transpose()?;
        let snapshot_sha256 = if let Some(snapshot) = &snapshot {
            snapshot.set_postings_cache_bytes(512 << 20);
            // The sidecar written with the snapshot spares hashing the whole file at start-up.
            let sidecar = fs::read_to_string(snapshot.path().with_extension("ocnpj.sha256"))
                .ok()
                .and_then(|text| text.split_whitespace().next().map(str::to_owned))
                .filter(|sha| sha.len() == 64);
            let sha = match sidecar {
                Some(sha) => sha,
                None => sha256_file(snapshot.path())?,
            };
            if let Some(agg) = &aggregates {
                ensure!(sha == agg.current().manifest().snapshot_sha256, "snapshot/aggregate identity mismatch");
            }
            Some(sha)
        } else {
            None
        };
        let mut caps = query::validate_capabilities(&catalogue, &sectors, anchor);
        caps.current = Competence::from_year_month(parts[0], parts[1] as u8).context("competence")?;
        caps.competences = if let Some(aggregates) = &aggregates {
            ensure!(aggregates.current().manifest().competence == caps.current, "anchor and aggregate competence differ");
            (0..=caps.current.0).map(Competence).filter(|c| aggregates.get(*c).is_some()).collect()
        } else {
            vec![caps.current]
        };
        let interpreter = Interpreter::new(pack, caps, anchor);
        Ok(Self { catalogue, sectors, sector_labels, interpreter, aggregates, snapshot, anchor, snapshot_sha256 })
    }

    pub fn canonical(&self, request: Request) -> Result<ValidRequest> {
        query::validate_request(request, &self.catalogue, &self.sectors, self.anchor)
    }
}

/// Answer to a question about the service itself.
pub fn meta_text(request: &ValidRequest) -> String {
    match request.op {
        Op::Meta { topic: MetaTopic::DataDate } => format!("Competência {}; referência {}.", request.time.competence, request.time.anchor),
        Op::Meta { topic: MetaTopic::Sources } => {
            "Receita Federal: dados cadastrais; IBGE: população e geografia. Veja os manifestos dos catálogos e agregados.".into()
        }
        Op::Meta { topic: MetaTopic::Cnaes } => format!("CNAEs da consulta: {:?}", request.scope.activity),
        Op::Meta { topic: MetaTopic::Methodology } => "A unidade padrão é o estabelecimento. Estoque ativo usa situação 02. Registros usam início de atividade; baixas usam a última data da situação 08. Capital social não é faturamento.".into(),
        _ => "Posso contar e listar estabelecimentos, comparar locais e setores, mostrar distribuições, registros, baixas, estoque histórico e densidade por habitante. Também entrego os resultados em Excel, CSV ou TXT: peça \"me mande um Excel com…\".".into(),
    }
}

pub fn value_json(value: &Value) -> Json {
    match value {
        Value::Scalar { value } => json!({"scalar":value}),
        Value::Ratio { num, den } => json!({"numerator":num,"denominator":den}),
        Value::Rows(rows) => json!({"rows":rows.iter().map(|r|json!({"key":r.key,"value":r.value,"base":r.base})).collect::<Vec<_>>()}),
    }
}

pub fn remember_execution(state: &mut Conversation, interpretation: &Interpretation, execution: &Json) {
    if execution["status"] != "ok" {
        return;
    }
    let Some(request) = &interpretation.request else {
        return;
    };
    let Some(rows) = execution.pointer("/value/rows").and_then(Json::as_array) else {
        return;
    };
    let members = rows
        .iter()
        .filter_map(|row| {
            let key = row.get("key")?.as_u64()?;
            match request.op.dimension()? {
                Dim::Municipality => u16::try_from(key).ok().map(|id| Member::Municipality(MunicipalityId(id))),
                Dim::Uf => u8::try_from(key).ok().map(|id| Member::Uf(UfId(id))),
                Dim::Sector => u16::try_from(key).ok().map(|id| Member::Sector(SectorId(id))),
                _ => None,
            }
        })
        .collect();
    state.remember_result(members);
}
