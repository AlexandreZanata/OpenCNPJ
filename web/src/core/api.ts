/** HTTP client: sends the session token, reads quota headers and turns failures into messages. */

import { apiUrl } from "../config.js";
import { session, updateSession, type Account, type Platform } from "./session.js";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    /** Seconds until the request may be retried (rate limit, quota). */
    readonly retryAfter: number | null = null,
  ) {
    super(message);
  }
}

const MESSAGES: Record<string, string> = {
  unauthenticated: "Entre com a sua chave para continuar.",
  invalid_credentials: "Sua sessão expirou. Entre novamente.",
  invalid_key: "Chave inválida. Confira e tente de novo.",
  account_suspended: "Esta conta está suspensa. Fale com o administrador.",
  account_banned: "Esta conta foi bloqueada.",
  rate_limited: "Muitas consultas em pouco tempo. Aguarde alguns segundos.",
  quota_exceeded: "Você usou todas as consultas do seu plano neste período.",
  plan_forbids_export: "Seu plano não inclui exportação de arquivos.",
  plan_forbids_downloads: "Seu plano não inclui downloads.",
  limit_exceeds_plan: "Esse tamanho de página é maior do que o seu plano permite.",
  too_many_failures: "Muitas tentativas com chave inválida. Aguarde um minuto.",
  too_many_requests: "Muitas tentativas a partir deste endereço. Tente mais tarde.",
  registration_closed: "O cadastro de novas contas está fechado.",
  username_taken: "Esse nome de usuário já existe.",
  invalid_username: "Use de 3 a 32 letras minúsculas, números, ponto, hífen ou sublinhado.",
  invalid_email: "Informe um e-mail válido.",
  cnpj_not_found: "CNPJ não encontrado na base.",
  invalid_document: "CNPJ inválido. Confira os dígitos.",
  assistant_unavailable: "O assistente não está habilitado neste servidor.",
  overloaded: "O servidor está ocupado. Tente novamente em instantes.",
  timeout: "A consulta demorou demais. Tente restringir os filtros.",
  forbidden: "Sua conta não tem permissão para isso.",
};

function quotaFrom(headers: Headers): void {
  const number = (name: string): number | null => {
    const value = headers.get(name);
    return value === null ? null : Number(value);
  };
  const dailyLimit = number("x-quota-daily-limit");
  const monthlyLimit = number("x-quota-monthly-limit");
  if (dailyLimit === null && monthlyLimit === null) return;
  updateSession({
    quota: {
      dailyLimit,
      dailyRemaining: number("x-quota-daily-remaining"),
      monthlyLimit,
      monthlyRemaining: number("x-quota-monthly-remaining"),
    },
  });
}

async function failure(response: Response): Promise<ApiError> {
  let code = "error";
  let message = "";
  try {
    const body = (await response.json()) as { error?: string; message?: string };
    code = body.error ?? code;
    message = body.message ?? "";
  } catch {
    /* Not JSON: fall back to the status. */
  }
  const retry = response.headers.get("retry-after");
  return new ApiError(response.status, code, MESSAGES[code] ?? (message || `Erro ${response.status}.`), retry ? Number(retry) : null);
}

export interface RequestOptions {
  method?: string;
  body?: unknown;
  signal?: AbortSignal;
}

/** Performs a request and returns the raw response; a 401 with a stored session ends it. */
export async function request(path: string, options: RequestOptions = {}): Promise<Response> {
  const headers: Record<string, string> = {};
  const token = session().token;
  if (token) headers["Authorization"] = `Bearer ${token}`;
  if (options.body !== undefined) headers["Content-Type"] = "application/json";
  const init: RequestInit = { method: options.method ?? "GET", headers };
  if (options.body !== undefined) init.body = JSON.stringify(options.body);
  if (options.signal) init.signal = options.signal;
  const response = await fetch(apiUrl(path), init);
  quotaFrom(response.headers);
  if (!response.ok) {
    const error = await failure(response);
    if (response.status === 401 && token && error.code !== "invalid_key") {
      updateSession({ token: null, account: null, quota: null, staff: false });
    }
    throw error;
  }
  return response;
}

export async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  const options: RequestOptions = {};
  if (signal) options.signal = signal;
  return (await (await request(path, options)).json()) as T;
}

export async function sendJson<T>(method: string, path: string, body?: unknown): Promise<T> {
  const options: RequestOptions = { method };
  if (body !== undefined) options.body = body;
  const response = await request(path, options);
  if (response.status === 204) return undefined as T;
  const text = await response.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

export function errorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof DOMException && error.name === "AbortError") return "";
  return "Não consegui falar com o servidor. Verifique a conexão.";
}

/** Hands a response body to the browser as a download. */
export async function saveAs(response: Response, filename: string): Promise<void> {
  const url = URL.createObjectURL(await response.blob());
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 30_000);
}

// ----- session lifecycle -------------------------------------------------------------------

export async function loadPlatform(): Promise<Platform> {
  const platform = await getJson<Platform>("/api/v1/platform");
  updateSession({ platform });
  return platform;
}

/** Refreshes the profile, plan and usage of the logged-in user. */
export async function loadAccount(): Promise<Account | null> {
  if (!session().token) return null;
  try {
    const account = await getJson<Account>("/api/v1/auth/me");
    // Only staff accounts can use the admin API, and only where the admin listener answers.
    const staff =
      account.permissions.length > 0 &&
      (await fetch(apiUrl("/admin/api/whoami"), { headers: { Authorization: `Bearer ${session().token}` } })
        .then((response) => response.ok)
        .catch(() => false));
    updateSession({
      account,
      staff,
      quota: {
        dailyLimit: account.limits.daily_limit,
        dailyRemaining: account.limits.daily_limit === null ? null : Math.max(0, account.limits.daily_limit - account.usage.today),
        monthlyLimit: account.limits.monthly_limit,
        monthlyRemaining: account.limits.monthly_limit === null ? null : Math.max(0, account.limits.monthly_limit - account.usage.month),
      },
    });
    return account;
  } catch {
    return null;
  }
}

export async function login(key: string): Promise<void> {
  const reply = await sendJson<{ token: string }>("POST", "/api/v1/auth/login", { key: key.trim() });
  updateSession({ token: reply.token });
  await loadAccount();
}

export async function logout(): Promise<void> {
  try {
    await sendJson("POST", "/api/v1/auth/logout");
  } catch {
    /* The session is dropped locally either way. */
  }
  updateSession({ token: null, account: null, quota: null, staff: false });
}
