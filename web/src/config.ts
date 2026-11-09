export const config = {
  /** Base URL of the OpenCNPJ HTTP API ("" = same origin). */
  apiBase: "",
  pageSize: 5,
} as const;

export function apiUrl(path: string): string {
  return `${config.apiBase}${path}`;
}
