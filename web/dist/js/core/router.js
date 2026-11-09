/** Hash router: `#/consulta`, `#/analises`, … */
export const ROUTES = [
    "consulta",
    "explorar",
    "analises",
    "relatorios",
    "exportacoes",
    "favoritos",
    "api",
    "documentacao",
    "configuracoes",
];
export const DEFAULT_ROUTE = "consulta";
export function currentRoute() {
    const id = window.location.hash.replace(/^#\/?/, "");
    return ROUTES.includes(id) ? id : DEFAULT_ROUTE;
}
export function routeHref(id) {
    return `#/${id}`;
}
export function onRouteChange(listener) {
    const handler = () => listener(currentRoute());
    window.addEventListener("hashchange", handler);
    return () => window.removeEventListener("hashchange", handler);
}
//# sourceMappingURL=router.js.map