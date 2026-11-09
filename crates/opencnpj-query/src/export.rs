//! File delivery requested together with a query ("me mande um Excel com …").
//!
//! An export is a delivery instruction, not part of the query: the same canonical
//! request answers the chat and fills the file, so it is kept outside the wire codec
//! and the cache key.

use serde::{Deserialize, Serialize};

use crate::ir::{ContactNeed, FlowMetric, Metric, Op, Request, Status, StatusSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Xlsx,
    Csv,
    Txt,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
            Self::Txt => "txt",
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Txt => "text/plain; charset=utf-8",
        }
    }

    /// Name shown to the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Xlsx => "Excel (.xlsx)",
            Self::Csv => "CSV (.csv)",
            Self::Txt => "texto (.txt)",
        }
    }
}

/// Columns of an exported establishment record, in the RFB vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportField {
    Cnpj,
    RazaoSocial,
    NomeFantasia,
    MatrizFilial,
    Situacao,
    DataSituacao,
    MotivoSituacao,
    DataInicio,
    CnaePrincipal,
    CnaeDescricao,
    NaturezaJuridica,
    Porte,
    CapitalSocial,
    Simples,
    Mei,
    Endereco,
    Bairro,
    Cep,
    Municipio,
    Uf,
    Telefone,
    Email,
}

impl ExportField {
    pub const ALL: [Self; 22] = [
        Self::Cnpj,
        Self::RazaoSocial,
        Self::NomeFantasia,
        Self::MatrizFilial,
        Self::Situacao,
        Self::DataSituacao,
        Self::MotivoSituacao,
        Self::DataInicio,
        Self::CnaePrincipal,
        Self::CnaeDescricao,
        Self::NaturezaJuridica,
        Self::Porte,
        Self::CapitalSocial,
        Self::Simples,
        Self::Mei,
        Self::Endereco,
        Self::Bairro,
        Self::Cep,
        Self::Municipio,
        Self::Uf,
        Self::Telefone,
        Self::Email,
    ];

    pub fn header(self) -> &'static str {
        match self {
            Self::Cnpj => "CNPJ",
            Self::RazaoSocial => "Razão social",
            Self::NomeFantasia => "Nome fantasia",
            Self::MatrizFilial => "Matriz/filial",
            Self::Situacao => "Situação cadastral",
            Self::DataSituacao => "Data da situação",
            Self::MotivoSituacao => "Motivo da situação",
            Self::DataInicio => "Início de atividade",
            Self::CnaePrincipal => "CNAE principal",
            Self::CnaeDescricao => "Atividade principal",
            Self::NaturezaJuridica => "Natureza jurídica",
            Self::Porte => "Porte",
            Self::CapitalSocial => "Capital social (R$)",
            Self::Simples => "Simples Nacional",
            Self::Mei => "MEI",
            Self::Endereco => "Endereço",
            Self::Bairro => "Bairro",
            Self::Cep => "CEP",
            Self::Municipio => "Município",
            Self::Uf => "UF",
            Self::Telefone => "Telefone",
            Self::Email => "E-mail",
        }
    }

    /// Phone and e-mail columns, removed when the server hides contacts.
    pub fn is_contact(self) -> bool {
        matches!(self, Self::Telefone | Self::Email)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub format: ExportFormat,
    /// Columns in the order the user named them; empty selects [`default_fields`].
    #[serde(default)]
    pub fields: Vec<ExportField>,
    /// Explicit row count from the question ("as 50 primeiras"). `None` exports every
    /// match up to the server cap.
    #[serde(default)]
    pub rows: Option<u32>,
}

impl Export {
    /// Columns written for `request`: the requested ones, or the default set.
    pub fn columns(&self, request: &Request) -> Vec<ExportField> {
        if self.fields.is_empty() { default_fields(request) } else { self.fields.clone() }
    }
}

/// Cadastral columns that answer a records export. Closed and suspended records also
/// carry the date and reason of their status; contacts appear only when the question
/// filtered by them ("com telefone").
pub fn default_fields(request: &Request) -> Vec<ExportField> {
    use ExportField::*;
    let status = request.scope.status.unwrap_or_else(|| request.op.default_status());
    let closures = matches!(request.op, Op::Count { metric: Metric::Flow(FlowMetric::Closures) });
    let mut fields = vec![Cnpj, RazaoSocial, NomeFantasia, MatrizFilial, Situacao];
    if closures || status != StatusSet::only(Status::Active) {
        fields.extend([DataSituacao, MotivoSituacao]);
    }
    fields.extend([DataInicio, CnaePrincipal, CnaeDescricao, Porte, CapitalSocial, Municipio, Uf]);
    match request.scope.contact {
        ContactNeed::Any => {}
        ContactNeed::Phone => fields.push(Telefone),
        ContactNeed::Email => fields.push(Email),
        ContactNeed::Both => fields.extend([Telefone, Email]),
    }
    fields
}
