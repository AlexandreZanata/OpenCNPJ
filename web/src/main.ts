import { loadAccount, loadPlatform, loadTrial } from "./core/api.js";
import { h } from "./core/dom.js";
import { currentRoute, onRouteChange, type Route } from "./core/router.js";
import { needsLogin, onSession, session } from "./core/session.js";
import { AppShell, type Shell } from "./layout/shell.js";
import { AccountPage } from "./pages/account-page.js";
import { AdminPage } from "./pages/admin-page.js";
import { AssistantPage } from "./pages/assistant-page.js";
import { CnpjPage } from "./pages/cnpj-page.js";
import { LoginPage } from "./pages/login-page.js";
import type { Page } from "./pages/page.js";
import { SearchPage } from "./pages/search-page.js";
import { EmptyState } from "./ui/kit.js";

async function bootstrap(): Promise<void> {
  const root = document.getElementById("app");
  if (!root) throw new Error("Missing #app root element");

  try {
    await loadPlatform();
  } catch {
    root.replaceChildren(h("div", { class: "auth" }, EmptyState("info", "Não consegui falar com o servidor.", "Recarregue a página em instantes.")));
    return;
  }
  if (session().token) await loadAccount();
  else await loadTrial();

  let shell: Shell | null = null;
  let page: Page | null = null;
  // The conversation survives a visit to the other tools.
  let assistant: Page | null = null;
  let signedIn = Boolean(session().token);

  const show = (route: Route): void => {
    if (!shell) return;
    if (page && page !== assistant) page.destroy?.();
    const name = route.name === "admin" && !session().staff ? "assistente" : route.name;
    shell.setRoute(name);
    switch (name) {
      case "assistente":
        assistant ??= AssistantPage();
        page = assistant;
        break;
      case "busca":
        page = SearchPage(route.param);
        break;
      case "cnpj":
        page = CnpjPage(route.param);
        break;
      case "conta":
        page = session().token ? AccountPage() : LoginPage(route.param === "criar" ? "criar" : "entrar");
        break;
      case "admin":
        page = AdminPage();
        break;
    }
    shell.outlet.replaceChildren(page.el);
    if (page === assistant) assistant.resume?.();
    else window.scrollTo(0, 0);
  };

  const render = (): void => {
    page?.destroy?.();
    assistant?.destroy?.();
    page = assistant = null;
    shell?.destroy();
    if (needsLogin()) {
      shell = null;
      root.replaceChildren(LoginPage(currentRoute().param === "criar" ? "criar" : "entrar", true).el);
      return;
    }
    shell = AppShell();
    root.replaceChildren(shell.el);
    show(currentRoute());
  };

  onRouteChange(show);
  // Logging in or out changes what the whole application may show.
  onSession((state) => {
    const now = Boolean(state.token);
    if (now !== signedIn) {
      signedIn = now;
      render();
    }
  });
  render();
}

void bootstrap();
