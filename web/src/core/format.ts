/** pt-BR formatters shared by every component. */

const integerFormat = new Intl.NumberFormat("pt-BR");
const currencyFormat = new Intl.NumberFormat("pt-BR", {
  style: "currency",
  currency: "BRL",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});
const percentFormat = new Intl.NumberFormat("pt-BR", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});

/** `482`, `1.234` */
export function formatInteger(value: number): string {
  return integerFormat.format(value);
}

/** `R$ 500.000,00` (regular space instead of the NBSP emitted by Intl). */
export function formatBRL(value: number): string {
  return currencyFormat.format(value).replace(/\u00a0/g, " ");
}

/** `R$ 318,6 milhões`, `R$ 1,2 bilhão`, `R$ 950 mil` */
export function formatBRLCompact(value: number): string {
  const abs = Math.abs(value);
  const fraction = (n: number): string => percentFormat.format(n);
  if (abs >= 1e9) {
    const n = value / 1e9;
    return `R$ ${fraction(n)} ${Math.abs(n) >= 2 ? "bilhões" : "bilhão"}`;
  }
  if (abs >= 1e6) {
    const n = value / 1e6;
    return `R$ ${fraction(n)} ${Math.abs(n) >= 2 ? "milhões" : "milhão"}`;
  }
  if (abs >= 1e3) return `R$ ${integerFormat.format(Math.round(value / 1e3))} mil`;
  return formatBRL(value);
}

/** `58,7%` */
export function formatPercent(value: number): string {
  return `${percentFormat.format(value)}%`;
}

/** `+28,4%` / `-3,0%` */
export function formatSignedPercent(value: number): string {
  return `${value >= 0 ? "+" : "-"}${percentFormat.format(Math.abs(value))}%`;
}

/** `2025-01-15` → `15/01/2025` (no Date object: avoids time-zone shifts). */
export function formatDate(iso: string): string {
  const [year, month, day] = iso.split("-");
  return `${day}/${month}/${year}`;
}

/** `56418975000123` → `56.418.975/0001-23`; other inputs are returned unchanged. */
export function formatCnpj(raw: string): string {
  const digits = raw.replace(/[^0-9A-Za-z]/g, "").toUpperCase();
  if (digits.length !== 14) return raw;
  return `${digits.slice(0, 2)}.${digits.slice(2, 5)}.${digits.slice(5, 8)}/${digits.slice(8, 12)}-${digits.slice(12)}`;
}
