export const config = {
    /** Base URL of the OpenCNPJ HTTP API ("" = same origin). */
    apiBase: "",
    pageSize: 5,
};
export function apiUrl(path) {
    return `${config.apiBase}${path}`;
}
//# sourceMappingURL=config.js.map