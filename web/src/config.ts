export const config = {
  /** Base URL of the OpenCNPJ HTTP API ("" = same origin). */
  apiBase: "",
  /** Rows per page of the search table. */
  pageSize: 25,
} as const;

export function apiUrl(path: string): string {
  return `${config.apiBase}${path}`;
}
