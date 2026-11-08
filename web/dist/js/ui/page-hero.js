import { h } from "../core/dom.js";
/** Page header: green eyebrow, serif headline and lead paragraph. */
export function PageHero({ eyebrow, title, lead }) {
    return h("header", { class: "page-hero" }, h("p", { class: "page-hero__eyebrow" }, eyebrow), h("h1", { class: "page-hero__title" }, title.flatMap((line, i) => (i === 0 ? [line] : [h("br"), line]))), h("p", { class: "page-hero__lead" }, lead));
}
//# sourceMappingURL=page-hero.js.map