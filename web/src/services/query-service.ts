import type { CompaniesPage, CompaniesRequest, QueryOverview } from "../domain/models.js";

/**
 * Boundary between the UI and the data source. The UI only depends on this
 * interface; swap `MockQueryService` for an HTTP implementation against the
 * OpenCNPJ API without touching any component.
 */
export interface QueryService {
  /** Interprets a natural-language request and returns filters plus aggregates. */
  interpret(query: string): Promise<QueryOverview>;
  /** Returns one sorted page of matching companies. */
  companies(request: CompaniesRequest): Promise<CompaniesPage>;
  /** Returns every matching company (used by exports). */
  allCompanies(request: Omit<CompaniesRequest, "page" | "pageSize">): Promise<CompaniesPage["rows"]>;
}
