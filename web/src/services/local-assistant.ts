import type { AskReply, Context, ExportOffer } from "../features/chat/types.js";

async function failure(response: Response, fallback: string): Promise<Error> {
  try {
    const body = (await response.json()) as { error?: string };
    return new Error(body.error ?? fallback);
  } catch {
    return new Error(fallback);
  }
}

/** One conversation turn. The context of the previous reply makes follow-ups work. */
export async function ask(question: string, context: Context | null, signal: AbortSignal): Promise<AskReply> {
  const response = await fetch("/api/local/ask", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(context ? { question, context } : { question }),
    signal,
  });
  if (!response.ok) throw await failure(response, "O motor local não conseguiu concluir a consulta.");
  return (await response.json()) as AskReply;
}

/** Builds the offered file on the server and hands it to the browser as a download. */
export async function download(offer: ExportOffer, question: string): Promise<number> {
  const response = await fetch("/api/local/export", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ request: offer.request, export: offer.export, question }),
  });
  if (!response.ok) throw await failure(response, "Não consegui gerar o arquivo.");
  const blob = await response.blob();
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = offer.filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 30_000);
  return Number(response.headers.get("x-opencnpj-rows") ?? offer.rows);
}
