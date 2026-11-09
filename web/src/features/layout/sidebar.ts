import { h, type View } from "../../core/dom.js";
import { routeHref, type RouteId } from "../../core/router.js";
import { Brand } from "../../ui/logo.js";
import { NavLink } from "../../ui/nav-link.js";
import type { IconName } from "../../ui/icons.js";
import { Icon } from "../../ui/icon.js";

interface NavEntry {
  id: RouteId;
  label: string;
  icon: IconName;
  trailingIcon?: IconName;
}

const PRIMARY: readonly NavEntry[] = [
  { id: "consulta", label: "Consulta com IA", icon: "sparkle" },
  { id: "explorar", label: "Explorar Dados", icon: "layers", trailingIcon: "chevron-right" },
  { id: "analises", label: "Análises", icon: "chart" },
  { id: "relatorios", label: "Relatórios", icon: "file" },
  { id: "exportacoes", label: "Exportações", icon: "download" },
  { id: "favoritos", label: "Favoritos", icon: "star" },
];

const SECONDARY: readonly NavEntry[] = [
  { id: "api", label: "API e Integrações", icon: "code" },
  { id: "documentacao", label: "Documentação", icon: "book" },
  { id: "configuracoes", label: "Configurações", icon: "settings" },
];

function DataSourceCard(): HTMLElement {
  return h(
    "aside",
    { class: "source-card", "aria-label": "Fonte dos dados" },
    Icon({ name: "database", size: 28, strokeWidth: 1.6, class: "source-card__icon" }),
    h("p", { class: "source-card__title" }, "Dados oficiais", h("br"), "da Receita Federal"),
    h("p", { class: "source-card__text" }, "CNPJ · CNAE · Porte · Situação e muito mais"),
  );
}

/** Left navigation rail. */
export function Sidebar(): View<{ route: RouteId }> {
  const nav = (entries: readonly NavEntry[], route: RouteId): HTMLElement =>
    h(
      "ul",
      { class: "sidebar__list" },
      entries.map((entry) =>
        h("li", null, NavLink({ label: entry.label, icon: entry.icon, trailingIcon: entry.trailingIcon, href: routeHref(entry.id), active: entry.id === route })),
      ),
    );

  const links = h("div", { class: "sidebar__links" });
  const el = h(
    "aside",
    { class: "sidebar" },
    h("a", { class: "sidebar__brand", href: routeHref("consulta"), "aria-label": "OpenCNPJ — início" }, Brand()),
    h("nav", { class: "sidebar__nav", "aria-label": "Principal" }, links),
    DataSourceCard(),
  );

  return {
    el,
    update: ({ route }) => {
      links.replaceChildren(nav(PRIMARY, route), h("hr", { class: "sidebar__divider" }), nav(SECONDARY, route));
    },
  };
}
