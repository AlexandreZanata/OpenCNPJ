import { DEMO_COMPANIES, DEMO_FILTERS, DEMO_OPENINGS, DEMO_PORTE, DEMO_QUERY, DEMO_SUMMARY, } from "./fixtures.js";
const LATENCY_MS = 280;
const collator = new Intl.Collator("pt-BR", { sensitivity: "base", numeric: true });
function compareBy(sort) {
    const factor = sort.direction === "asc" ? 1 : -1;
    const value = (company) => {
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
        const order = typeof left === "number" && typeof right === "number"
            ? left - right
            : collator.compare(String(left), String(right));
        return order * factor;
    };
}
function wait(ms) {
    return new Promise((resolve) => window.setTimeout(resolve, ms));
}
/**
 * In-memory implementation backed by demo fixtures. Every query returns the
 * same canned dataset; sorting and pagination are real.
 */
export class MockQueryService {
    async interpret(query) {
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
    async companies(request) {
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
    async allCompanies(request) {
        return [...DEMO_COMPANIES].sort(compareBy(request.sort));
    }
}
//# sourceMappingURL=mock-query-service.js.map