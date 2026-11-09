/** Contract of the local chat service (`opencnpj-intel serve`). */

export type Decision = "execute" | "execute_with_note" | "clarify" | "reject";

export interface ChartPoint {
  label: string;
  value: number;
}

export type Block =
  | { type: "text"; text: string }
  | { type: "kpi"; label: string; value: string }
  | { type: "comparison"; members: { label: string; value: string; base: string | null }[]; difference_abs: string; difference_pct: string | null }
  | { type: "table"; columns: string[]; rows: string[][]; total: string | null }
  | { type: "chart"; kind: "bar" | "line"; series: ChartPoint[] }
  | { type: "records"; columns: string[]; rows: string[][]; total: number; page: number }
  | { type: "company"; fields: [string, string][] }
  | { type: "methodology"; items: string[] }
  | { type: "sources"; items: { name: string; competence: string; snapshot_sha256: string }[] }
  | { type: "caveats"; items: { id: string; text: string }[] }
  | { type: "notice"; text: string }
  | { type: "clarify"; question: string; options: { label: string; reply: string }[] }
  | { type: "reject"; reason: string };

export interface Answer {
  decision: Decision;
  understood: string;
  blocks: Block[];
  suggestions: string[];
}

export type Extra =
  | { type: "series"; title: string; series: { name: string; points: ChartPoint[] }[] }
  | { type: "records"; title: string; columns: string[]; rows: string[][]; total: number };

export interface ExportOffer {
  format: "xlsx" | "csv" | "txt";
  label: string;
  filename: string;
  rows: number;
  total: number;
  columns: string[];
  notes: string[];
  /** Canonical request and export spec, posted back verbatim to build the file. */
  request: unknown;
  export: unknown;
}

/** Opaque conversation state: returned by each reply and sent with the next question. */
export type Context = Record<string, unknown>;

export interface AskReply {
  interpretation: { decision: Decision; act: string; reason: string; notes: string[] };
  answer: Answer;
  extras: Extra[];
  export: ExportOffer | null;
  context: Context;
  timing_us: Record<string, number>;
}
