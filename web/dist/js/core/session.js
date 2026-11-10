/**
 * Session state shared by every page: what the server offers, who is logged in and how
 * much of the plan was used. Pages subscribe; the API client keeps the quota current from
 * the headers of each answer.
 */
const TOKEN_KEY = "opencnpj.session";
const listeners = new Set();
function storedToken() {
    try {
        return window.localStorage.getItem(TOKEN_KEY);
    }
    catch {
        return null;
    }
}
let state = { platform: null, token: storedToken(), account: null, quota: null, trialQuota: null, upgradeNeeded: false, staff: false };
export function session() {
    return state;
}
export function updateSession(patch) {
    state = { ...state, ...patch };
    if ("token" in patch) {
        try {
            if (patch.token)
                window.localStorage.setItem(TOKEN_KEY, patch.token);
            else
                window.localStorage.removeItem(TOKEN_KEY);
        }
        catch {
            /* Private browsing: the session lasts for this tab only. */
        }
    }
    listeners.forEach((listener) => listener(state));
}
export function onSession(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
}
/** Accounts are in use and nobody is logged in. */
export function needsLogin() {
    return state.platform?.auth === "required" && !state.token;
}
export function accountsEnabled() {
    return state.platform !== null && state.platform.auth !== "off";
}
//# sourceMappingURL=session.js.map