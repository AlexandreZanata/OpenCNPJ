import { h } from "../../core/dom.js";
import { FilterTag } from "../../ui/filter-tag.js";
import { Icon } from "../../ui/icon.js";
/** Shows the verifiable filters the AI derived from the request. */
export function InterpretationBanner() {
    const tags = h("div", { class: "interpretation__tags", role: "list" });
    const el = h("section", { class: "interpretation", "aria-label": "Consulta interpretada pela IA" }, Icon({ name: "sparkles", size: 28, strokeWidth: 1.8, class: "interpretation__icon" }), h("div", { class: "interpretation__text" }, h("h2", { class: "interpretation__title" }, "Consulta interpretada pela IA"), h("p", { class: "interpretation__description" }, "Transformamos seu pedido em filtros verificáveis e consultamos a base oficial do CNPJ.")), tags);
    return {
        el,
        update: ({ filters, visible, loading }) => {
            el.hidden = !visible;
            el.classList.toggle("is-loading", loading);
            tags.replaceChildren(...filters.map((filter) => {
                const tag = FilterTag(filter);
                tag.setAttribute("role", "listitem");
                return tag;
            }));
        },
    };
}
//# sourceMappingURL=interpretation-banner.js.map