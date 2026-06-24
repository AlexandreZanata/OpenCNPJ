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
  qualificacao_responsavel?: SqlNullString
  capital_social?: SqlNullFloat
  porte_empresa?: SqlNullString
  ente_federativo_responsavel?: SqlNullString
  created_at?: string
  updated_at?: string
}

export interface EmpresaFull extends Empresa {
  natureza_descricao?: SqlNullString
  qualificacao_descricao?: SqlNullString
}

export interface Socio {
  id?: number
  uuid_id?: string
  cnpj_basico: string
  identificador_socio?: SqlNullString
  nome_socio: string
  cpf_cnpj_socio?: SqlNullString
  qualificacao_socio?: SqlNullString
  data_entrada_sociedade?: string | SqlNullString
  pais?: SqlNullString
  representante_legal?: SqlNullString
  nome_representante?: SqlNullString
  qualificacao_representante?: SqlNullString
  faixa_etaria?: SqlNullString
  created_at?: string
}

export interface Simples {
  uuid_id?: string
  cnpj_basico: string
  opcao_simples?: SqlNullString
  data_opcao_simples?: string | SqlNullString
  data_exclusao_simples?: string | SqlNullString
  opcao_mei?: SqlNullString
  data_opcao_mei?: string | SqlNullString
  data_exclusao_mei?: string | SqlNullString
}

export interface EstabelecimentoFull {
  id?: number
  uuid_id?: string
  cnpj_completo: string
  cnpj_basico: string
  cnpj_ordem?: SqlNullString
  cnpj_dv?: SqlNullString
  identificador_matriz_filial?: SqlNullString
  nome_fantasia?: SqlNullString
  razao_social?: SqlNullString
  capital_social?: SqlNullFloat
  situacao_cadastral?: SqlNullString
  data_situacao_cadastral?: string | SqlNullString
  motivo_situacao_cadastral?: SqlNullString
  motivo_descricao?: SqlNullString
  nome_cidade_exterior?: SqlNullString
  pais?: SqlNullString
  pais_descricao?: SqlNullString
  data_inicio_atividade?: string | SqlNullString
  cnae_fiscal_principal?: SqlNullString
  cnae_fiscal_secundaria?: SqlNullString
  cnae_descricao?: SqlNullString
  tipo_logradouro?: SqlNullString
  logradouro?: SqlNullString
  numero?: SqlNullString
  complemento?: SqlNullString
  bairro?: SqlNullString
  cep?: SqlNullString
  uf?: SqlNullString
  municipio?: SqlNullString
  municipio_nome?: SqlNullString
  ddd_1?: SqlNullString
  telefone_1?: SqlNullString
  ddd_2?: SqlNullString
  telefone_2?: SqlNullString
  ddd_fax?: SqlNullString
  fax?: SqlNullString
  email?: SqlNullString
  situacao_especial?: SqlNullString
  data_situacao_especial?: string | SqlNullString
  created_at?: string
}

export interface EmpresaAggregate extends EmpresaFull {
  estabelecimentos: EstabelecimentoFull[]
  socios: Socio[]
  simples?: Simples | null
}

export interface EstabelecimentoSearchResult extends EstabelecimentoFull {
  empresa: EmpresaFull
  socios: Socio[]
  simples?: Simples | null
}

/** @deprecated use EstabelecimentoFull */
export type Estabelecimento = EstabelecimentoFull

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

export interface PhoneExportRequest {
  category: string
  cnae?: string
  uf?: string
  municipio?: string
  municipio_nome?: string
  nome_fantasia?: string
  created_from?: string
  created_to?: string
  only_active?: boolean
  export_all?: boolean
  limit?: number
  format: 'csv' | 'txt'
}

export interface LookupItem {
  type: string
  code: string
  label: string
  description?: string
  uf?: string
}

export interface ExportCategory {
  key: string
  label: string
  description: string
  cnae_codes: string[]
}
