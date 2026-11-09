import { h } from "../../core/dom.js";
import type { RouteId } from "../../core/router.js";
import { HeroArt } from "./hero-art.js";
import { Sidebar } from "./sidebar.js";
import { Topbar } from "./topbar.js";

export interface AppShell {
  readonly el: HTMLElement;
  /** Container the active page is rendered into. */
  readonly outlet: HTMLElement;
  setRoute(route: RouteId): void;
}

/** Application frame: sidebar + workspace (backdrop art, top bar, page outlet). */
export function AppShell(): AppShell {
  const sidebar = Sidebar();
  const outlet = h("main", { class: "workspace__main", id: "main", tabindex: -1 });

  const el = h(
    "div",
    { class: "app" },
    h("a", { class: "skip-link", href: "#main" }, "Ir para o conteúdo"),
    sidebar.el,
    h(
      "div",
      { class: "workspace" },
      HeroArt(),
      Topbar({
        planName: "Plano Profissional",
        hasNotifications: true,
        onNavigate: (hash) => {
          window.location.hash = hash;
        },
      }),
      outlet,
    ),
  );

  return { el, outlet, setRoute: (route) => sidebar.update({ route }) };
}
