/**
 * Session state shared by every page: what the server offers, who is logged in and how
 * much of the plan was used. Pages subscribe; the API client keeps the quota current from
 * the headers of each answer.
 */

export interface Platform {
  auth: "off" | "optional" | "required";
  registration: "open" | "closed" | null;
  assistant: boolean;
  /** Filter search and CNPJ lookup are served (false on the local assistant-only service). */
  search: boolean;
  export: boolean;
  contacts: boolean;
  competence: string;
  documents: number;
  version: string;
}

export interface Plan {
  name: string;
  daily_limit: number | null;
  monthly_limit: number | null;
  per_minute: number | null;
  max_page_size: number;
  export: boolean;
  max_export_rows: number | null;
  downloads: boolean;
}

export interface Account {
  user: { id: number; username: string; email: string; role: string; status: string; plan: string; key_prefix?: string };
  plan: Plan;
  limits: { daily_limit: number | null; monthly_limit: number | null; per_minute: number | null };
  usage: { today: number; month: number };
  permissions: string[];
}

export interface Quota {
  dailyLimit: number | null;
  dailyRemaining: number | null;
  monthlyLimit: number | null;
  monthlyRemaining: number | null;
}

export interface SessionState {
  platform: Platform | null;
  token: string | null;
  account: Account | null;
  quota: Quota | null;
  /** The admin API answers on this origin for this user (admin listener, staff login). */
  staff: boolean;
}

const TOKEN_KEY = "opencnpj.session";
const listeners = new Set<(state: SessionState) => void>();

function storedToken(): string | null {
  try {
    return window.localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

let state: SessionState = { platform: null, token: storedToken(), account: null, quota: null, staff: false };

export function session(): Readonly<SessionState> {
  return state;
}

export function updateSession(patch: Partial<SessionState>): void {
  state = { ...state, ...patch };
  if ("token" in patch) {
    try {
      if (patch.token) window.localStorage.setItem(TOKEN_KEY, patch.token);
      else window.localStorage.removeItem(TOKEN_KEY);
    } catch {
      /* Private browsing: the session lasts for this tab only. */
    }
  }
  listeners.forEach((listener) => listener(state));
}

export function onSession(listener: (state: SessionState) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Accounts are in use and nobody is logged in. */
export function needsLogin(): boolean {
  return state.platform?.auth === "required" && !state.token;
}

export function accountsEnabled(): boolean {
  return state.platform !== null && state.platform.auth !== "off";
}
