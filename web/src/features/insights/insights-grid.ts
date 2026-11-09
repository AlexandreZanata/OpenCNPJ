import { h, type View } from "../../core/dom.js";
import { formatBRLCompact, formatInteger, formatSignedPercent } from "../../core/format.js";
import type { QueryOverview } from "../../domain/models.js";
import { BarChart } from "../../ui/bar-chart.js";
import { DonutChart } from "../../ui/donut-chart.js";
import { MetricCard } from "../../ui/metric-card.js";

export interface InsightsProps {
  overview: QueryOverview | null;
}

/** KPI cards plus the openings-per-month and company-size charts. */
export function InsightsGrid(): View<InsightsProps> {
  const el = h("section", { class: "insights", "aria-label": "Indicadores da consulta" });

  const skeleton = (): HTMLElement[] =>
    Array.from({ length: 5 }, () => h("div", { class: "card skeleton", "aria-hidden": "true" }));

  return {
    el,
    update: ({ overview }) => {
      el.setAttribute("aria-busy", String(overview === null));
      if (overview === null) {
        el.replaceChildren(...skeleton());
        return;
      }
      const { summary, openings, porte, total } = overview;
      el.replaceChildren(
        MetricCard({
          icon: "building",
          label: "Empresas encontradas",
          value: formatInteger(summary.companiesFound),
          delta: formatSignedPercent(summary.companiesDelta),
          caption: "vs. 12 meses anteriores",
        }),
        MetricCard({
          icon: "coins",
          label: "Capital social total",
          value: formatBRLCompact(summary.totalCapital),
          compact: true,
          delta: formatSignedPercent(summary.capitalDelta),
        }),
        MetricCard({
          icon: "calendar",
          label: "Novas empresas",
          value: formatInteger(summary.newCompanies),
          caption: "(últimos 12 meses)",
        }),
        BarChart({ title: "Aberturas por mês", data: openings }),
        DonutChart({
          title: "Por porte (nº de empresas)",
          slices: porte,
          centerValue: formatInteger(total),
          centerLabel: "empresas",
        }),
      );
    },
  };
}
