/** Application frame: a slim top bar with the three tools, the plan's usage and the account. */
import { logout } from "../core/api.js";
import { h } from "../core/dom.js";
import { formatInteger } from "../core/format.js";
import { href } from "../core/router.js";
import { accountsEnabled, onSession, session } from "../core/session.js";
import { Icon } from "../ui/icon.js";
import { Logo } from "./logo.js";
import { ThemeToggle } from "./theme-toggle.js";
const NAV = [
    ["assistente", "Assistente", "message"],
    ["busca", "Busca", "filter"],
    ["cnpj", "CNPJ", "building"],
];
export function AppShell() {
    const links = new Map();
    const nav = h("nav", { class: "topbar__nav", "aria-label": "Ferramentas" }, NAV.filter(([name]) => name === "assistente" || session().platform?.search !== false).map(([name, label, icon]) => {
        const link = h("a", { class: "nav-link", href: href(name) }, Icon({ name: icon, size: 16 }), h("span", null, label));
        links.set(name, link);
        return link;
    }));
    const side = h("div", { class: "topbar__account" });
    const notice = h("div", { class: "access-notice", hidden: true, role: "status" });
    const outlet = h("main", { class: "outlet", id: "conteudo" });
    let current = "assistente";
    const mark = () => {
        links.forEach((link, name) => {
            if (name === current)
                link.setAttribute("aria-current", "page");
            else
                link.removeAttribute("aria-current");
        });
    };
    const renderSide = (state) => {
        const items = [];
        const quota = state.quota;
        if (state.token && quota && quota.dailyLimit !== null && quota.dailyRemaining !== null) {
            const used = quota.dailyLimit - quota.dailyRemaining;
            const low = quota.dailyRemaining <= quota.dailyLimit * 0.1;
            items.push(h("a", { class: `usage${low ? " usage--low" : ""}`, href: href("conta"), title: "Consultas usadas hoje no seu plano" }, h("span", { class: "usage__bar" }, h("i", { style: `width:${Math.min(100, (used / Math.max(1, quota.dailyLimit)) * 100).toFixed(1)}%` })), h("span", null, `${formatInteger(used)} / ${formatInteger(quota.dailyLimit)} hoje`)));
        }
        if (state.staff) {
            const admin = h("a", { class: "nav-link", href: href("admin") }, Icon({ name: "shield", size: 16 }), h("span", null, "Administração"));
            links.set("admin", admin);
            items.push(admin);
        }
        if (accountsEnabled()) {
            if (state.token) {
                const account = h("a", { class: "nav-link", href: href("conta"), title: "Minha conta" }, h("span", { class: "avatar", "aria-hidden": "true" }, (state.account?.user.username ?? "?").slice(0, 1).toUpperCase()), h("span", null, state.account?.user.username ?? "Conta"));
                links.set("conta", account);
                items.push(account, h("button", { class: "nav-link nav-link--icon", type: "button", title: "Sair", "aria-label": "Sair", on: { click: () => void logout() } }, Icon({ name: "log-out", size: 16 })));
            }
            else {
                items.push(h("a", { class: "btn btn--primary btn--sm", href: href("conta") }, "Entrar"));
            }
        }
        if (!state.token && state.trialQuota && state.trialQuota.remaining > 0) {
            const { remaining } = state.trialQuota;
            const label = remaining === 1 ? "1 consulta grátis" : `${remaining} consultas grátis`;
            items.unshift(h("a", { class: "usage trial-usage", href: href("conta", "criar") }, label));
        }
        const free = state.account?.plan.name === "free";
        const exhausted = !state.token && state.trialQuota?.remaining === 0;
        const upgrade = free && (state.upgradeNeeded || quota?.dailyRemaining === 0 || quota?.monthlyRemaining === 0);
        if (exhausted) {
            notice.replaceChildren(h("span", null, "Sua amostra grátis terminou."), ...(state.platform?.registration === "closed" ? [] : [h("a", { href: href("conta", "criar") }, "Criar conta grátis")]), h("a", { href: href("conta") }, "Entrar"));
        }
        else if (upgrade) {
            notice.replaceChildren(h("span", null, "Você atingiu um limite do plano gratuito."), h("a", { href: href("conta") }, "Ver opções de plano"));
        }
        notice.hidden = !exhausted && !upgrade;
        side.replaceChildren(...items.filter((item) => item !== null));
        mark();
    };
    renderSide(session());
    const unsubscribe = onSession(renderSide);
    return {
        el: h("div", { class: "app" }, h("a", { class: "sr-only", href: "#conteudo" }, "Pular para o conteúdo"), h("header", { class: "topbar" }, h("a", { class: "topbar__brand", href: href("assistente"), "aria-label": "OpenCNPJ — início" }, Logo()), nav, h("div", { class: "topbar__side" }, ThemeToggle(), side)), notice, outlet),
        outlet,
        setRoute: (route) => {
            current = route;
            mark();
        },
        destroy: unsubscribe,
    };
}
//# sourceMappingURL=shell.js.map