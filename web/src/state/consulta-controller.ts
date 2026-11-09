import { Store } from "../core/store.js";
import type { Company, QueryOverview, SortKey, SortState } from "../domain/models.js";
import { download, serialize, type ExportFormat } from "../services/export.js";
import type { QueryService } from "../services/query-service.js";

export interface ConsultaState {
  /** Text currently shown in the search box. */
  draft: string;
  overview: QueryOverview | null;
  overviewLoading: boolean;
  rows: readonly Company[];
  total: number;
  page: number;
  pageSize: number;
  sort: SortState;
  rowsLoading: boolean;
  hiddenColumns: ReadonlySet<string>;
  filtersVisible: boolean;
  error: string | null;
}

const DEFAULT_SORT: SortState = { key: "abertura", direction: "desc" };
/** Keys whose natural first order is descending (newest / largest first). */
const DESC_FIRST: ReadonlySet<SortKey> = new Set<SortKey>(["abertura", "capitalSocial"]);

/** Orchestrates the "Consulta com IA" screen: owns state, talks to the service. */
export class ConsultaController {
  readonly store: Store<ConsultaState>;
  /** Guards against out-of-order responses. */
  private overviewSeq = 0;
  private rowsSeq = 0;
  /** Query the current results belong to. */
  private activeQuery = "";

  constructor(
    private readonly service: QueryService,
    initialQuery: string,
    pageSize: number,
  ) {
    this.store = new Store<ConsultaState>({
      draft: initialQuery,
      overview: null,
      overviewLoading: false,
      rows: [],
      total: 0,
      page: 1,
      pageSize,
      sort: DEFAULT_SORT,
      rowsLoading: false,
      hiddenColumns: new Set(),
      filtersVisible: true,
      error: null,
    });
  }

  start(): Promise<void> {
    return this.submit(this.store.get().draft);
  }

  async submit(text: string): Promise<void> {
    const query = text.trim();
    if (!query) return;
    this.activeQuery = query;
    const seq = ++this.overviewSeq;
    this.store.set({ draft: query, overviewLoading: true, error: null, page: 1, sort: DEFAULT_SORT });

    try {
      const [overview] = await Promise.all([this.service.interpret(query), this.loadRows(1, DEFAULT_SORT)]);
      if (seq !== this.overviewSeq) return;
      this.store.set({ overview, overviewLoading: false });
    } catch (error) {
      if (seq !== this.overviewSeq) return;
      this.store.set({ overviewLoading: false, error: describe(error) });
    }
  }

  clear(): void {
    this.store.set({ draft: "" });
  }

  async sortBy(key: SortKey): Promise<void> {
    const { sort } = this.store.get();
    const next: SortState =
      sort.key === key
        ? { key, direction: sort.direction === "asc" ? "desc" : "asc" }
        : { key, direction: DESC_FIRST.has(key) ? "desc" : "asc" };
    this.store.set({ sort: next, page: 1 });
    await this.loadRows(1, next);
  }

  async goToPage(page: number): Promise<void> {
    this.store.set({ page });
    await this.loadRows(page, this.store.get().sort);
  }

  toggleColumn(id: string): void {
    const hidden = new Set(this.store.get().hiddenColumns);
    if (!hidden.delete(id)) hidden.add(id);
    this.store.set({ hiddenColumns: hidden });
  }

  toggleFilters(): void {
    this.store.set({ filtersVisible: !this.store.get().filtersVisible });
  }

  async export(format: ExportFormat): Promise<void> {
    try {
      const rows = await this.service.allCompanies({ query: this.activeQuery, sort: this.store.get().sort });
      const mime = format === "csv" ? "text/csv" : "application/x-ndjson";
      download(`opencnpj-empresas.${format}`, serialize(rows, format), mime);
    } catch (error) {
      this.store.set({ error: describe(error) });
    }
  }

  private async loadRows(page: number, sort: SortState): Promise<void> {
    const seq = ++this.rowsSeq;
    this.store.set({ rowsLoading: true });
    try {
      const result = await this.service.companies({
        query: this.activeQuery,
        sort,
        page,
        pageSize: this.store.get().pageSize,
      });
      if (seq !== this.rowsSeq) return;
      this.store.set({ rows: result.rows, total: result.total, rowsLoading: false });
    } catch (error) {
      if (seq !== this.rowsSeq) return;
      this.store.set({ rowsLoading: false, error: describe(error) });
    }
  }
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : "Não foi possível concluir a consulta.";
}
