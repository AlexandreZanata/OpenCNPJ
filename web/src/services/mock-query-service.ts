import type {
  Company,
  CompaniesPage,
  CompaniesRequest,
  QueryOverview,
  SortState,
} from "../domain/models.js";
import {
  DEMO_COMPANIES,
  DEMO_FILTERS,
  DEMO_OPENINGS,
  DEMO_PORTE,
  DEMO_QUERY,
  DEMO_SUMMARY,
} from "./fixtures.js";
import type { QueryService } from "./query-service.js";

const LATENCY_MS = 280;

const collator = new Intl.Collator("pt-BR", { sensitivity: "base", numeric: true });

function compareBy(sort: SortState): (a: Company, b: Company) => number {
  const factor = sort.direction === "asc" ? 1 : -1;
  const value = (company: Company): string | number => {
    switch (sort.key) {
      case "cnpj": return company.cnpj;
      case "razaoSocial": return company.razaoSocial;
      case "cnae": return company.cnaeCodigo;
      case "municipio": return `${company.municipio}/${company.uf}`;
      case "capitalSocial": return company.capitalSocial;
      case "abertura": return company.abertura;
      case "situacao": return company.situacao;
    }
  };
  return (a, b) => {
    const left = value(a);
    const right = value(b);
    const order =
      typeof left === "number" && typeof right === "number"
        ? left - right
        : collator.compare(String(left), String(right));
    return order * factor;
  };
}

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

/**
 * In-memory implementation backed by demo fixtures. Every query returns the
 * same canned dataset; sorting and pagination are real.
 */
export class MockQueryService implements QueryService {
  async interpret(query: string): Promise<QueryOverview> {
    await wait(LATENCY_MS);
    return {
      query: query.trim() || DEMO_QUERY,
      filters: DEMO_FILTERS,
      summary: DEMO_SUMMARY,
      openings: DEMO_OPENINGS,
      porte: DEMO_PORTE,
      total: DEMO_COMPANIES.length,
    };
  }

  async companies(request: CompaniesRequest): Promise<CompaniesPage> {
    await wait(LATENCY_MS / 2);
    const sorted = [...DEMO_COMPANIES].sort(compareBy(request.sort));
    const start = (request.page - 1) * request.pageSize;
    return {
      total: sorted.length,
      page: request.page,
      pageSize: request.pageSize,
      rows: sorted.slice(start, start + request.pageSize),
    };
  }

  async allCompanies(request: Omit<CompaniesRequest, "page" | "pageSize">): Promise<Company[]> {
    return [...DEMO_COMPANIES].sort(compareBy(request.sort));
  }
}
