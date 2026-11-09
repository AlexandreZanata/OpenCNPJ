import { h, s } from "../core/dom.js";
/** The hexagonal cube mark. */
export function LogoMark(size = 42) {
    const svg = s("svg", { class: "logo__mark", width: size, height: size, viewBox: "0 0 40 40", "aria-hidden": "true", focusable: "false" }, s("defs", null, s("linearGradient", { id: "logo-top", x1: "0", y1: "0", x2: "1", y2: "1" }, s("stop", { offset: "0", "stop-color": "#2fbf78" }), s("stop", { offset: "1", "stop-color": "#1e9d62" })), s("linearGradient", { id: "logo-left", x1: "0", y1: "0", x2: "0", y2: "1" }, s("stop", { offset: "0", "stop-color": "#17895a" }), s("stop", { offset: "1", "stop-color": "#0f6e48" })), s("linearGradient", { id: "logo-right", x1: "0", y1: "0", x2: "0", y2: "1" }, s("stop", { offset: "0", "stop-color": "#35c985" }), s("stop", { offset: "1", "stop-color": "#22a56a" }))), 
    // Outer hexagon faces (left / top / right) around a hollow core.
    s("polygon", { points: "4,11 20,2 20,13 12,18 12,27 4,29", fill: "url(#logo-left)" }), s("polygon", { points: "20,2 36,11 28,15.5 20,11", fill: "url(#logo-top)" }), s("polygon", { points: "36,11 36,29 20,38 20,29 28,24.5 28,15.5", fill: "url(#logo-right)" }), s("polygon", { points: "4,29 20,38 20,29 12,24.5", fill: "#0f6e48" }), 
    // Dark core.
    s("polygon", { points: "20,13 28,17.5 28,26.5 20,31 12,26.5 12,17.5", fill: "#0e2a27" }));
    return svg;
}
/** Wordmark: mark, product name and tagline. */
export function Brand() {
    return h("div", { class: "logo" }, LogoMark(), h("div", { class: "logo__text" }, h("span", { class: "logo__name" }, "OpenCNPJ"), h("span", { class: "logo__tagline" }, "Inteligência empresarial")));
}
//# sourceMappingURL=logo.js.map