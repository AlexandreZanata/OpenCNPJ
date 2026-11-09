import { h } from "../core/dom.js";
import { CompaniesSection } from "../features/companies/companies-section.js";
import { InsightsGrid } from "../features/insights/insights-grid.js";
import { InterpretationBanner } from "../features/query/interpretation-banner.js";
import { QueryBox } from "../features/query/query-box.js";
import { ConsultaController } from "../state/consulta-controller.js";
import { PageHero } from "../ui/page-hero.js";
/** "Consulta com IA": natural-language search, indicators and results table. */
export function ConsultaPage({ controller, examples }) {
    const queryBox = QueryBox({
        examples,
        onSubmit: (text) => void controller.submit(text),
        onExample: (text) => void controller.submit(text),
        onClear: () => controller.clear(),
    });
    const banner = InterpretationBanner();
    const insights = InsightsGrid();
    const companies = CompaniesSection({
        onSort: (key) => void controller.sortBy(key),
        onPage: (page) => void controller.goToPage(page),
        onToggleColumn: (id) => controller.toggleColumn(id),
        onExport: (format) => void controller.export(format),
        onToggleFilters: () => controller.toggleFilters(),
    });
    const alert = h("p", { class: "alert", role: "alert", hidden: true });
    const render = () => {
        const state = controller.store.get();
        queryBox.update({ value: state.draft, loading: state.overviewLoading });
        banner.update({
            filters: state.overview?.filters ?? [],
            visible: state.filtersVisible && state.overview !== null,
            loading: state.overviewLoading,
        });
        insights.update({ overview: state.overview });
        companies.update({
            rows: state.rows,
            total: state.total,
            page: state.page,
            pageSize: state.pageSize,
            sort: state.sort,
            loading: state.rowsLoading,
            hiddenColumns: state.hiddenColumns,
            filterCount: state.overview?.filters.length ?? 0,
        });
        alert.hidden = state.error === null;
        alert.textContent = state.error ?? "";
    };
    const el = h("div", { class: "page page--consulta" }, PageHero({
        eyebrow: "Plataforma de inteligência empresarial",
        title: ["Inteligência empresarial", "com dados oficiais do CNPJ"],
        lead: "Use linguagem natural para encontrar empresas, analisar mercados e extrair insights com os dados da Receita Federal.",
    }), queryBox.el, banner.el, alert, insights.el, companies.el);
    const unsubscribe = controller.store.subscribe(render);
    render();
    return { el, destroy: unsubscribe };
}
//# sourceMappingURL=consulta-page.js.map