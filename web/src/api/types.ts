/** Domain types aligned with docs/GLOSSARY.md and internal/models. */

export type SqlNullString = { String: string; Valid: boolean } | string | null
export type SqlNullFloat = { Float64: number; Valid: boolean } | number | null

export interface SearchResponse<T> {
  data: T[]
  total: number
  limit: number
  offset: number
  has_more: boolean
}

export interface ErrorResponse {
  error: string
  message?: string
  code: number
}

export interface Empresa {
  uuid_id: string
  cnpj_basico: string
  razao_social: string
  natureza_juridica?: SqlNullString
  capital_social?: SqlNullFloat
  porte_empresa?: SqlNullString
  ente_federativo_responsavel?: SqlNullString
}

export interface Estabelecimento {
  cnpj_completo: string
  cnpj_basico: string
  nome_fantasia?: SqlNullString
  razao_social?: SqlNullString
  situacao_cadastral?: SqlNullString
  cnae_fiscal_principal?: SqlNullString
  cnae_descricao?: SqlNullString
  uf?: SqlNullString
  municipio?: SqlNullString
  municipio_nome?: SqlNullString
  logradouro?: SqlNullString
  numero?: SqlNullString
  bairro?: SqlNullString
  cep?: SqlNullString
  email?: SqlNullString
  ddd_1?: SqlNullString
  telefone_1?: SqlNullString
}

export interface StatsRow {
  cnae?: string
  uf?: string
  count: number
}

export interface EmpresaSearchParams {
  cnpj_basico?: string
  razao_social?: string
  natureza_juridica?: string
  porte_empresa?: string
  limit?: number
  offset?: number
}

export interface EstabelecimentoSearchParams {
  cnpj?: string
  cnpj_basico?: string
  nome_fantasia?: string
  cnae?: string
  uf?: string
  municipio?: string
  situacao?: string
  cep?: string
  limit?: number
  offset?: number
}

export interface ExportRequest {
  filters: Record<string, string | number>
  selected_columns: string[]
  format: 'csv'
}
