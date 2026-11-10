/** Hash router: `#/`, `#/busca`, `#/cnpj/33000167000101`, `#/conta`, `#/admin`. */
const NAMES = ["assistente", "busca", "cnpj", "conta", "admin"];
export function currentRoute() {
    const [name = "", ...rest] = window.location.hash.replace(/^#\/?/, "").split("/");
    const known = NAMES.includes(name) ? name : "assistente";
    return { name: known, param: decodeURIComponent(rest.join("/")) };
}
export function href(name, param = "") {
    return name === "assistente" && !param ? "#/" : `#/${name}${param ? `/${encodeURIComponent(param)}` : ""}`;
}
export function navigate(name, param = "") {
    window.location.hash = href(name, param);
}
export function onRouteChange(listener) {
    const handler = () => listener(currentRoute());
    window.addEventListener("hashchange", handler);
    return () => window.removeEventListener("hashchange", handler);
}
//# sourceMappingURL=router.js.map