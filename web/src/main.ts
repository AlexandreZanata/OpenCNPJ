import { currentRoute, onRouteChange, type RouteId } from "./core/router.js";
import { AppShell } from "./features/layout/app-shell.js";
import { ConsultaPage, type Page } from "./pages/consulta-page.js";
import { PlaceholderPage } from "./pages/placeholder-page.js";
import { config } from "./config.js";
import { MockQueryService } from "./services/mock-query-service.js";
import { DEMO_QUERY, EXAMPLE_QUERIES } from "./services/fixtures.js";
import { ConsultaController } from "./state/consulta-controller.js";

function bootstrap(): void {
  const root = document.getElementById("app");
  if (!root) throw new Error("Missing #app root element");

  // Swap MockQueryService for an HTTP implementation to talk to the real API.
  const controller = new ConsultaController(new MockQueryService(), DEMO_QUERY, config.pageSize);
  const shell = AppShell();
  root.replaceChildren(shell.el);

  let page: Page | null = null;
  let started = false;

  const show = (route: RouteId): void => {
    page?.destroy();
    shell.setRoute(route);
    page = route === "consulta" ? ConsultaPage({ controller, examples: EXAMPLE_QUERIES }) : PlaceholderPage(route);
    shell.outlet.replaceChildren(page.el);
    if (route === "consulta" && !started) {
      started = true;
      void controller.start();
    }
  };

  onRouteChange(show);
  show(currentRoute());
}

bootstrap();
