import { h } from "../core/dom.js";
import { EmptyState } from "../ui/empty-state.js";
import { PageHero } from "../ui/page-hero.js";
const COPY = {
    explorar: { title: ["Explorar dados"], lead: "Navegue pelas tabelas oficiais: empresas, estabelecimentos, sócios e domínios." },
    analises: { title: ["Análises"], lead: "Cruze setores, regiões e portes para entender o mercado." },
    relatorios: { title: ["Relatórios"], lead: "Gere relatórios reutilizáveis a partir das suas consultas." },
    exportacoes: { title: ["Exportações"], lead: "Acompanhe e baixe as exportações em CSV e NDJSON." },
    favoritos: { title: ["Favoritos"], lead: "Consultas e empresas que você salvou." },
    api: { title: ["API e Integrações"], lead: "Chaves, limites do plano e exemplos de integração." },
    documentacao: { title: ["Documentação"], lead: "Guias, formato do snapshot e referência da API." },
    configuracoes: { title: ["Configurações"], lead: "Conta, plano e preferências da plataforma." },
};
/** Placeholder for areas that are not built yet; keeps navigation honest. */
export function PlaceholderPage(route) {
    const { title, lead } = COPY[route];
    const el = h("div", { class: "page page--placeholder" }, PageHero({ eyebrow: "Plataforma de inteligência empresarial", title, lead }), EmptyState({ title: "Em construção", description: "Esta área ainda não está disponível. Volte para a Consulta com IA para continuar." }));
    return { el, destroy: () => undefined };
}
//# sourceMappingURL=placeholder-page.js.map