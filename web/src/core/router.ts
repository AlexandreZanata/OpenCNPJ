/** Hash router: `#/`, `#/busca`, `#/cnpj/33000167000101`, `#/conta`, `#/admin`. */

export type RouteName = "assistente" | "busca" | "cnpj" | "conta" | "admin";

export interface Route {
  name: RouteName;
  /** Path segment after the route name (`#/cnpj/<param>`). */
  param: string;
}

const NAMES: readonly RouteName[] = ["assistente", "busca", "cnpj", "conta", "admin"];

export function currentRoute(): Route {
  const [name = "", ...rest] = window.location.hash.replace(/^#\/?/, "").split("/");
  const known = (NAMES as readonly string[]).includes(name) ? (name as RouteName) : "assistente";
  return { name: known, param: decodeURIComponent(rest.join("/")) };
}

export function href(name: RouteName, param = ""): string {
  return name === "assistente" && !param ? "#/" : `#/${name}${param ? `/${encodeURIComponent(param)}` : ""}`;
}

export function navigate(name: RouteName, param = ""): void {
  window.location.hash = href(name, param);
}

export function onRouteChange(listener: (route: Route) => void): () => void {
  const handler = (): void => listener(currentRoute());
  window.addEventListener("hashchange", handler);
  return () => window.removeEventListener("hashchange", handler);
}
