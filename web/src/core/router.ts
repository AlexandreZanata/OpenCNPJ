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
] as const;

export type RouteId = (typeof ROUTES)[number];

export const DEFAULT_ROUTE: RouteId = "consulta";

export function currentRoute(): RouteId {
  const id = window.location.hash.replace(/^#\/?/, "");
  return (ROUTES as readonly string[]).includes(id) ? (id as RouteId) : DEFAULT_ROUTE;
}

export function routeHref(id: RouteId): string {
  return `#/${id}`;
}

export function onRouteChange(listener: (route: RouteId) => void): () => void {
  const handler = (): void => listener(currentRoute());
  window.addEventListener("hashchange", handler);
  return () => window.removeEventListener("hashchange", handler);
}
