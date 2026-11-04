//! RFB open-data layouts ("Metadados CNPJ", layout in force since 2023, unchanged by the
//! alphanumeric CNPJ). Column 0 of every fact table is `cnpj_basico`; it becomes the
//! company key and is not repeated inside rows.

use serde::{Deserialize, Serialize};

/// How a column is encoded. Every typed encoding is verified to round-trip to the exact
/// source string; values that do not round-trip are stored verbatim ("escaped").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Free text, stored as UTF-8.
    Text,
    /// Low-cardinality text: global dictionary id, rare values escaped.
    Dict,
    /// `YYYYMMDD` calendar date, or the literal `00000000`.
    Date,
    /// Brazilian decimal with two fraction digits, e.g. `1000,00`.
    Money,
    /// Fixed-width ASCII digits (width given by the column).
    Digits(u8),
    /// Comma-separated list of 7-digit CNAE codes.
    CnaeList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Table {
    Empresas,
    Estabelecimentos,
    Socios,
    Simples,
}

impl Table {
    pub const ALL: [Table; 4] = [Table::Empresas, Table::Estabelecimentos, Table::Socios, Table::Simples];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Table::Empresas => "empresas",
            Table::Estabelecimentos => "estabelecimentos",
            Table::Socios => "socios",
            Table::Simples => "simples",
        }
    }

    /// All columns in source order, including `cnpj_basico` at index 0.
    pub fn columns(self) -> &'static [Column] {
        match self {
            Table::Empresas => EMPRESAS,
            Table::Estabelecimentos => ESTABELECIMENTOS,
            Table::Socios => SOCIOS,
            Table::Simples => SIMPLES,
        }
    }

    /// Columns stored in rows (source columns without `cnpj_basico`).
    pub fn row_columns(self) -> &'static [Column] {
        &self.columns()[1..]
    }

    /// Source file prefix inside the monthly share (`Empresas0.zip`, ...).
    pub fn zip_prefix(self) -> &'static str {
        match self {
            Table::Empresas => "Empresas",
            Table::Estabelecimentos => "Estabelecimentos",
            Table::Socios => "Socios",
            Table::Simples => "Simples",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Column {
    pub name: &'static str,
    pub kind: Kind,
}

const fn c(name: &'static str, kind: Kind) -> Column {
    Column { name, kind }
}

use Kind::*;

pub const EMPRESAS: &[Column] = &[
    c("cnpj_basico", Text),
    c("razao_social", Text),
    c("natureza_juridica", Dict),
    c("qualificacao_responsavel", Dict),
    c("capital_social", Money),
    c("porte_empresa", Dict),
    c("ente_federativo_responsavel", Dict),
];

pub const ESTABELECIMENTOS: &[Column] = &[
    c("cnpj_basico", Text),
    c("cnpj_ordem", Text),
    c("cnpj_dv", Digits(2)),
    c("identificador_matriz_filial", Dict),
    c("nome_fantasia", Text),
    c("situacao_cadastral", Dict),
    c("data_situacao_cadastral", Date),
    c("motivo_situacao_cadastral", Dict),
    c("nome_cidade_exterior", Dict),
    c("pais", Dict),
    c("data_inicio_atividade", Date),
    c("cnae_fiscal_principal", Dict),
    c("cnae_fiscal_secundaria", CnaeList),
    c("tipo_logradouro", Dict),
    c("logradouro", Text),
    c("numero", Dict),
    c("complemento", Text),
    c("bairro", Dict),
    c("cep", Digits(8)),
    c("uf", Dict),
    c("municipio", Dict),
    c("ddd_1", Dict),
    c("telefone_1", Text),
    c("ddd_2", Dict),
    c("telefone_2", Text),
    c("ddd_fax", Dict),
    c("fax", Text),
    c("correio_eletronico", Text),
    c("situacao_especial", Dict),
    c("data_situacao_especial", Date),
];

pub const SOCIOS: &[Column] = &[
    c("cnpj_basico", Text),
    c("identificador_socio", Dict),
    c("nome_socio_razao_social", Text),
    c("cpf_cnpj_socio", Text),
    c("qualificacao_socio", Dict),
    c("data_entrada_sociedade", Date),
    c("pais", Dict),
    c("representante_legal", Dict),
    c("nome_do_representante", Text),
    c("qualificacao_representante_legal", Dict),
    c("faixa_etaria", Dict),
];

pub const SIMPLES: &[Column] = &[
    c("cnpj_basico", Text),
    c("opcao_pelo_simples", Dict),
    c("data_opcao_simples", Date),
    c("data_exclusao_simples", Date),
    c("opcao_mei", Dict),
    c("data_opcao_mei", Date),
    c("data_exclusao_mei", Date),
];

/// Reference (domain) tables: two columns, `codigo;descricao`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefTable {
    Cnaes,
    Motivos,
    Municipios,
    Naturezas,
    Paises,
    Qualificacoes,
}

impl RefTable {
    pub const ALL: [RefTable; 6] =
        [RefTable::Cnaes, RefTable::Motivos, RefTable::Municipios, RefTable::Naturezas, RefTable::Paises, RefTable::Qualificacoes];

    pub fn name(self) -> &'static str {
        match self {
            RefTable::Cnaes => "cnaes",
            RefTable::Motivos => "motivos",
            RefTable::Municipios => "municipios",
            RefTable::Naturezas => "naturezas",
            RefTable::Paises => "paises",
            RefTable::Qualificacoes => "qualificacoes",
        }
    }

    pub fn zip_name(self) -> &'static str {
        match self {
            RefTable::Cnaes => "Cnaes",
            RefTable::Motivos => "Motivos",
            RefTable::Municipios => "Municipios",
            RefTable::Naturezas => "Naturezas",
            RefTable::Paises => "Paises",
            RefTable::Qualificacoes => "Qualificacoes",
        }
    }
}

/// Column index helpers (row index = source index - 1).
pub mod col {
    pub mod emp {
        pub const RAZAO_SOCIAL: usize = 0;
        pub const NATUREZA: usize = 1;
        pub const QUALIFICACAO: usize = 2;
        pub const CAPITAL: usize = 3;
        pub const PORTE: usize = 4;
    }
    pub mod est {
        pub const ORDEM: usize = 0;
        pub const DV: usize = 1;
        pub const MATRIZ_FILIAL: usize = 2;
        pub const NOME_FANTASIA: usize = 3;
        pub const SITUACAO: usize = 4;
        pub const DATA_SITUACAO: usize = 5;
        pub const MOTIVO: usize = 6;
        pub const PAIS: usize = 8;
        pub const DATA_INICIO: usize = 9;
        pub const CNAE_PRINCIPAL: usize = 10;
        pub const CNAE_SECUNDARIA: usize = 11;
        pub const CEP: usize = 17;
        pub const UF: usize = 18;
        pub const MUNICIPIO: usize = 19;
        pub const DDD_1: usize = 20;
        pub const TELEFONE_1: usize = 21;
        pub const TELEFONE_2: usize = 23;
        pub const EMAIL: usize = 26;
    }
    pub mod soc {
        pub const IDENTIFICADOR: usize = 0;
        pub const NOME: usize = 1;
        pub const DOCUMENTO: usize = 2;
        pub const QUALIFICACAO: usize = 3;
        pub const PAIS: usize = 5;
        pub const REPRESENTANTE: usize = 6;
        pub const QUALIFICACAO_REP: usize = 8;
    }
    pub mod sim {
        pub const OPCAO_SIMPLES: usize = 0;
        pub const DATA_OPCAO_SIMPLES: usize = 1;
        pub const DATA_EXCLUSAO_SIMPLES: usize = 2;
        pub const OPCAO_MEI: usize = 3;
        pub const DATA_OPCAO_MEI: usize = 4;
        pub const DATA_EXCLUSAO_MEI: usize = 5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_counts_match_layout() {
        assert_eq!(EMPRESAS.len(), 7);
        assert_eq!(ESTABELECIMENTOS.len(), 30);
        assert_eq!(SOCIOS.len(), 11);
        assert_eq!(SIMPLES.len(), 7);
        let est = Table::Estabelecimentos.row_columns();
        assert_eq!(est[col::est::UF].name, "uf");
        assert_eq!(est[col::est::MUNICIPIO].name, "municipio");
        assert_eq!(est[col::est::CNAE_SECUNDARIA].name, "cnae_fiscal_secundaria");
        assert_eq!(est[col::est::NOME_FANTASIA].name, "nome_fantasia");
        assert_eq!(Table::Empresas.row_columns()[col::emp::PORTE].name, "porte_empresa");
        assert_eq!(Table::Simples.row_columns()[col::sim::OPCAO_MEI].name, "opcao_mei");
        assert_eq!(Table::Simples.row_columns()[col::sim::DATA_EXCLUSAO_MEI].name, "data_exclusao_mei");
        assert_eq!(Table::Empresas.row_columns()[col::emp::CAPITAL].name, "capital_social");
        assert_eq!(est[col::est::DATA_INICIO].name, "data_inicio_atividade");
        assert_eq!(est[col::est::DATA_SITUACAO].name, "data_situacao_cadastral");
        assert_eq!(est[col::est::CEP].name, "cep");
        assert_eq!(est[col::est::DDD_1].name, "ddd_1");
        assert_eq!(est[col::est::TELEFONE_2].name, "telefone_2");
        assert_eq!(est[col::est::EMAIL].name, "correio_eletronico");
        let soc = Table::Socios.row_columns();
        assert_eq!(soc[col::soc::DOCUMENTO].name, "cpf_cnpj_socio");
        assert_eq!(soc[col::soc::REPRESENTANTE].name, "representante_legal");
    }
}
