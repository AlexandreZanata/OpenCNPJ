import { cx, h } from "../core/dom.js";
import { Icon } from "./icon.js";
/** `1 2 3 4 5 … 97` while near the start, `1 … 48 49 50 … 97` in the middle, etc. */
export function pageItems(page, pageCount) {
    if (pageCount <= 7)
        return Array.from({ length: pageCount }, (_, i) => i + 1);
    if (page <= 3)
        return [1, 2, 3, 4, 5, "gap", pageCount];
    if (page >= pageCount - 2)
        return [1, "gap", ...Array.from({ length: 5 }, (_, i) => pageCount - 4 + i)];
    return [1, "gap", page - 1, page, page + 1, "gap", pageCount];
}
/** Numbered pager with previous/next arrows. */
export function Pagination(onChange) {
    const el = h("nav", { class: "pagination", "aria-label": "Paginação" });
    const update = ({ page, pageCount }) => {
        const go = (target) => () => {
            if (target >= 1 && target <= pageCount && target !== page)
                onChange(target);
        };
        el.replaceChildren(h("button", { class: "pagination__btn", type: "button", "aria-label": "Página anterior", disabled: page <= 1, on: { click: go(page - 1) } }, Icon({ name: "chevron-left", size: 16 })), ...pageItems(page, pageCount).map((item) => item === "gap"
            ? h("span", { class: "pagination__gap", "aria-hidden": "true" }, "…")
            : h("button", {
                class: cx("pagination__btn", item === page && "is-active"),
                type: "button",
                "aria-label": `Página ${item}`,
                "aria-current": item === page ? "page" : undefined,
                on: { click: go(item) },
            }, item)), h("button", { class: "pagination__btn", type: "button", "aria-label": "Próxima página", disabled: page >= pageCount, on: { click: go(page + 1) } }, Icon({ name: "chevron-right", size: 16 })));
    };
    return { el, update };
}
//# sourceMappingURL=pagination.js.map