/** Application frame: a slim top bar with the three tools, the plan's usage and the account. */

import { logout } from "../core/api.js";
import { h } from "../core/dom.js";
import { formatInteger } from "../core/format.js";
import { href, type RouteName } from "../core/router.js";
import { accountsEnabled, onSession, session, type SessionState } from "../core/session.js";
import { Icon } from "../ui/icon.js";
import type { IconName } from "../ui/icons.js";
import { Logo } from "./logo.js";

const NAV: readonly (readonly [RouteName, string, IconName])[] = [
  ["assistente", "Assistente", "message"],
  ["busca", "Busca", "filter"],
  ["cnpj", "CNPJ", "building"],
];

export interface Shell {
  readonly el: HTMLElement;
  readonly outlet: HTMLElement;
  setRoute(route: RouteName): void;
  /** Stops following the session (the frame is being replaced). */
  destroy(): void;
}

export function AppShell(): Shell {
  const links = new Map<RouteName, HTMLAnchorElement>();
  const nav = h(
    "nav",
    { class: "topbar__nav", "aria-label": "Ferramentas" },
    NAV.filter(([name]) => name === "assistente" || session().platform?.search !== false).map(([name, label, icon]) => {
      const link = h("a", { class: "nav-link", href: href(name) }, Icon({ name: icon, size: 16 }), h("span", null, label));
      links.set(name, link);
      return link;
    }),
  );
  const side = h("div", { class: "topbar__side" });
  const outlet = h("main", { class: "outlet", id: "conteudo" });
  let current: RouteName = "assistente";
  const mark = (): void => {
    links.forEach((link, name) => {
      if (name === current) link.setAttribute("aria-current", "page");
      else link.removeAttribute("aria-current");
    });
  };

  const renderSide = (state: SessionState): void => {
    const items: (HTMLElement | null)[] = [];
    const quota = state.quota;
    if (state.token && quota && quota.dailyLimit !== null && quota.dailyRemaining !== null) {
      const used = quota.dailyLimit - quota.dailyRemaining;
      const low = quota.dailyRemaining <= quota.dailyLimit * 0.1;
      items.push(
        h(
          "a",
          { class: `usage${low ? " usage--low" : ""}`, href: href("conta"), title: "Consultas usadas hoje no seu plano" },
          h("span", { class: "usage__bar" }, h("i", { style: `width:${Math.min(100, (used / Math.max(1, quota.dailyLimit)) * 100).toFixed(1)}%` })),
          h("span", null, `${formatInteger(used)} / ${formatInteger(quota.dailyLimit)} hoje`),
        ),
      );
    }
    if (state.staff) {
      const admin = h("a", { class: "nav-link", href: href("admin") }, Icon({ name: "shield", size: 16 }), h("span", null, "Administração"));
      links.set("admin", admin);
      items.push(admin);
    }
    if (accountsEnabled()) {
      if (state.token) {
        const account = h(
          "a",
          { class: "nav-link", href: href("conta"), title: "Minha conta" },
          h("span", { class: "avatar", "aria-hidden": "true" }, (state.account?.user.username ?? "?").slice(0, 1).toUpperCase()),
          h("span", null, state.account?.user.username ?? "Conta"),
        );
        links.set("conta", account);
        items.push(account, h("button", { class: "nav-link nav-link--icon", type: "button", title: "Sair", "aria-label": "Sair", on: { click: () => void logout() } }, Icon({ name: "log-out", size: 16 })));
      } else {
        items.push(h("a", { class: "btn btn--primary btn--sm", href: href("conta") }, "Entrar"));
      }
    }
    side.replaceChildren(...items.filter((item): item is HTMLElement => item !== null));
    mark();
  };
  renderSide(session());
  const unsubscribe = onSession(renderSide);

  return {
    el: h(
      "div",
      { class: "app" },
      h("a", { class: "sr-only", href: "#conteudo" }, "Pular para o conteúdo"),
      h("header", { class: "topbar" }, h("a", { class: "topbar__brand", href: href("assistente"), "aria-label": "OpenCNPJ — início" }, Logo()), nav, side),
      outlet,
    ),
    outlet,
    setRoute: (route) => {
      current = route;
      mark();
    },
    destroy: unsubscribe,
  };
}
