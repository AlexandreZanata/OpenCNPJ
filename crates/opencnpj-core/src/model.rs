//! Semantic model: one company (`cnpj_basico`) with every row linked to it.

use serde::Serialize;

use crate::schema::Table;

/// A source row without its `cnpj_basico` column, values in layout order.
pub type Row = Vec<String>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Company {
    pub basico: String,
    pub empresas: Vec<Row>,
    pub estabelecimentos: Vec<Row>,
    pub socios: Vec<Row>,
    pub simples: Vec<Row>,
}

impl Company {
    pub fn rows(&self, t: Table) -> &[Row] {
        match t {
            Table::Empresas => &self.empresas,
            Table::Estabelecimentos => &self.estabelecimentos,
            Table::Socios => &self.socios,
            Table::Simples => &self.simples,
        }
    }

    pub fn rows_mut(&mut self, t: Table) -> &mut Vec<Row> {
        match t {
            Table::Empresas => &mut self.empresas,
            Table::Estabelecimentos => &mut self.estabelecimentos,
            Table::Socios => &mut self.socios,
            Table::Simples => &mut self.simples,
        }
    }

    /// Search documents: one per establishment, or one for a company without any.
    pub fn doc_count(&self) -> u32 {
        self.estabelecimentos.len().max(1) as u32
    }
}

/// Domains documented in the RFB layout but not shipped as reference files.
pub fn fixed_label(column: &str, code: &str) -> Option<&'static str> {
    Some(match (column, code) {
        ("identificador_matriz_filial", "1") => "MATRIZ",
        ("identificador_matriz_filial", "2") => "FILIAL",
        ("situacao_cadastral", "01" | "1") => "NULA",
        ("situacao_cadastral", "02" | "2") => "ATIVA",
        ("situacao_cadastral", "03" | "3") => "SUSPENSA",
        ("situacao_cadastral", "04" | "4") => "INAPTA",
        ("situacao_cadastral", "08" | "8") => "BAIXADA",
        ("porte_empresa", "00") => "NAO INFORMADO",
        ("porte_empresa", "01") => "MICRO EMPRESA",
        ("porte_empresa", "03") => "EMPRESA DE PEQUENO PORTE",
        ("porte_empresa", "05") => "DEMAIS",
        ("identificador_socio", "1") => "PESSOA JURIDICA",
        ("identificador_socio", "2") => "PESSOA FISICA",
        ("identificador_socio", "3") => "ESTRANGEIRO",
        ("faixa_etaria", "0") => "NAO SE APLICA",
        ("faixa_etaria", "1") => "0 A 12 ANOS",
        ("faixa_etaria", "2") => "13 A 20 ANOS",
        ("faixa_etaria", "3") => "21 A 30 ANOS",
        ("faixa_etaria", "4") => "31 A 40 ANOS",
        ("faixa_etaria", "5") => "41 A 50 ANOS",
        ("faixa_etaria", "6") => "51 A 60 ANOS",
        ("faixa_etaria", "7") => "61 A 70 ANOS",
        ("faixa_etaria", "8") => "71 A 80 ANOS",
        ("faixa_etaria", "9") => "MAIOR DE 80 ANOS",
        _ => return None,
    })
}
