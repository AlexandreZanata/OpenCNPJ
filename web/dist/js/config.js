export const config = {
    /** Base URL of the OpenCNPJ HTTP API ("" = same origin). */
    apiBase: "",
    /** Rows per page of the search table. */
    pageSize: 25,
};
export function apiUrl(path) {
    return `${config.apiBase}${path}`;
}
//# sourceMappingURL=config.js.map