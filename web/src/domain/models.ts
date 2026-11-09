/** Domain types: what the UI knows about the platform, independent of any transport. */

export type Situacao = "Ativa" | "Suspensa" | "Inapta" | "Baixada" | "Nula";

export interface Company {
  /** 14 characters, no punctuation. */
  cnpj: string;
  razaoSocial: string;
  cnaeCodigo: string;
  cnaeDescricao: string;
  municipio: string;
  uf: string;
  capitalSocial: number;
  /** ISO date (`YYYY-MM-DD`). */
  abertura: string;
  situacao: Situacao;
}

/** One verifiable filter the AI derived from the natural-language request. */
export interface InterpretedFilter {
  label: string;
  value: string;
}

export interface Summary {
  companiesFound: number;
  /** Percent change versus the previous 12 months. */
  companiesDelta: number;
  totalCapital: number;
  capitalDelta: number;
  newCompanies: number;
}

export interface MonthlyOpening {
  label: string;
  value: number;
}

export type PorteId = "ME" | "EPP" | "DEMAIS";

export interface PorteSlice {
  id: PorteId;
  label: string;
  /** Percent of the total (0–100). */
  share: number;
}

/** Everything the dashboard needs for one natural-language query. */
export interface QueryOverview {
  query: string;
  filters: InterpretedFilter[];
  summary: Summary;
  openings: MonthlyOpening[];
  porte: PorteSlice[];
  total: number;
}

export type SortKey =
  | "cnpj"
  | "razaoSocial"
  | "cnae"
  | "municipio"
  | "capitalSocial"
  | "abertura"
  | "situacao";

export type SortDirection = "asc" | "desc";

export interface SortState {
  key: SortKey;
  direction: SortDirection;
}

export interface CompaniesRequest {
  query: string;
  sort: SortState;
  /** 1-based. */
  page: number;
  pageSize: number;
}

export interface CompaniesPage {
  total: number;
  page: number;
  pageSize: number;
  rows: Company[];
}
