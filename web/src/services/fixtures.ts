import type {
  Company,
  InterpretedFilter,
  MonthlyOpening,
  PorteSlice,
  Summary,
} from "../domain/models.js";

/**
 * Demo fixtures. They mirror the product mock-up and are only used by
 * `MockQueryService`; nothing here is real Receita Federal data.
 */

export const DEMO_QUERY =
  "Encontre empresas de tecnologia abertas em Mato Grosso nos últimos 12 meses, com capital social acima de R$ 100 mil.";

export const EXAMPLE_QUERIES: readonly string[] = [
  "Empresas por setor e estado",
  "Novas empresas (últimos 12 meses)",
  "Maiores capitais sociais",
  "Empresas ativas por CNAE",
];

export const DEMO_FILTERS: InterpretedFilter[] = [
  { label: "Atividade", value: "Tecnologia (CNAEs relacionados)" },
  { label: "UF", value: "MT" },
  { label: "Abertura", value: "últimos 12 meses" },
  { label: "Capital social", value: "> R$ 100.000,00" },
];

export const DEMO_SUMMARY: Summary = {
  companiesFound: 482,
  companiesDelta: 28.4,
  totalCapital: 318_600_000,
  capitalDelta: 41.7,
  newCompanies: 482,
};

export const DEMO_OPENINGS: MonthlyOpening[] = [
  { label: "Fev", value: 17 },
  { label: "Mar", value: 21 },
  { label: "Abr", value: 25 },
  { label: "Mai", value: 27 },
  { label: "Jun", value: 30 },
  { label: "Jul", value: 34 },
  { label: "Ago", value: 41 },
  { label: "Set", value: 42 },
  { label: "Out", value: 45 },
  { label: "Nov", value: 56 },
  { label: "Dez", value: 52 },
  { label: "Jan", value: 70 },
];

export const DEMO_PORTE: PorteSlice[] = [
  { id: "ME", label: "ME", share: 58.7 },
  { id: "EPP", label: "EPP", share: 27.8 },
  { id: "DEMAIS", label: "Demais", share: 13.5 },
];

const TOTAL = 482;

const FEATURED: Company[] = [
  company("56418975000123", "MT Tech Soluções em Software Ltda", "6201-5/01", "Desenvolvimento de programas", "Cuiabá", 500_000, "2025-01-15"),
  company("56392771000109", "Pantanal Tecnologia e Dados Ltda", "6204-0/00", "Consultoria em TI", "Várzea Grande", 250_000, "2025-01-10"),
  company("56351908000147", "AgroTech MT Sistemas Inteligentes Ltda", "6209-1/00", "Suporte técnico em TI", "Rondonópolis", 300_000, "2025-01-08"),
  company("56309447000182", "Vórtice Tecnologia e Inovação Ltda", "6311-9/00", "Tratamento de dados", "Sinop", 1_000_000, "2025-01-06"),
  company("56270114000135", "Cerrado Sistemas e Consultoria Ltda", "6202-3/00", "Desenvolvimento e licenciamento", "Cuiabá", 150_000, "2025-01-03"),
];

function company(
  cnpj: string,
  razaoSocial: string,
  cnaeCodigo: string,
  cnaeDescricao: string,
  municipio: string,
  capitalSocial: number,
  abertura: string,
): Company {
  return { cnpj, razaoSocial, cnaeCodigo, cnaeDescricao, municipio, uf: "MT", capitalSocial, abertura, situacao: "Ativa" };
}

/** Deterministic PRNG so the demo data is identical on every load. */
function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Numeric CNPJ check digits (modulo 11). */
function withCheckDigits(base12: string): string {
  const digit = (digits: string, weights: readonly number[]): number => {
    const sum = weights.reduce((acc, weight, i) => acc + weight * Number(digits[i]), 0);
    const rest = sum % 11;
    return rest < 2 ? 0 : 11 - rest;
  };
  const first = digit(base12, [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
  const second = digit(base12 + first, [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
  return `${base12}${first}${second}`;
}

const PREFIXES = ["Cerrado", "Pantanal", "Araguaia", "Vórtice", "Nexo", "Aurora", "Tapajós", "Planalto", "Veredas", "Guará", "Cuiabana", "Ipê", "Jaguar", "Horizonte", "Alvorada", "Matrinxã"];
const KINDS = ["Tecnologia", "Sistemas", "Soluções Digitais", "Software", "Dados", "Cloud", "Informática", "Inovação", "Digital", "Automação"];
const SUFFIXES = ["Ltda", "Ltda", "Ltda", "ME", "EIRELI", "S.A."];

const CITIES: ReadonlyArray<readonly [string, number]> = [
  ["Cuiabá", 38], ["Várzea Grande", 14], ["Rondonópolis", 11], ["Sinop", 9], ["Tangará da Serra", 6],
  ["Lucas do Rio Verde", 6], ["Sorriso", 5], ["Primavera do Leste", 4], ["Cáceres", 4], ["Barra do Garças", 3],
];

const CNAES: ReadonlyArray<readonly [string, string]> = [
  ["6201-5/01", "Desenvolvimento de programas"],
  ["6201-5/02", "Web design"],
  ["6202-3/00", "Desenvolvimento e licenciamento"],
  ["6203-1/00", "Desenvolvimento de software"],
  ["6204-0/00", "Consultoria em TI"],
  ["6209-1/00", "Suporte técnico em TI"],
  ["6311-9/00", "Tratamento de dados"],
  ["6319-4/00", "Portais e provedores"],
];

const CAPITALS = [110_000, 150_000, 200_000, 250_000, 300_000, 500_000, 750_000, 1_000_000, 2_000_000, 5_000_000];
const SITUACOES: ReadonlyArray<readonly [Company["situacao"], number]> = [["Ativa", 94], ["Suspensa", 2], ["Inapta", 3], ["Baixada", 1]];

function pickWeighted<T>(items: ReadonlyArray<readonly [T, number]>, random: () => number): T {
  const total = items.reduce((acc, [, weight]) => acc + weight, 0);
  let cursor = random() * total;
  for (const [item, weight] of items) {
    cursor -= weight;
    if (cursor < 0) return item;
  }
  return items[items.length - 1]![0];
}

function pick<T>(items: readonly T[], random: () => number): T {
  return items[Math.floor(random() * items.length)]!;
}

function generated(count: number): Company[] {
  const random = mulberry32(20_251_108);
  const seen = new Set(FEATURED.map((c) => c.cnpj));
  const rows: Company[] = [];

  while (rows.length < count) {
    const root = String(Math.floor(40_000_000 + random() * 16_000_000));
    const cnpj = withCheckDigits(`${root}0001`);
    if (seen.has(cnpj)) continue;
    seen.add(cnpj);

    // Feb 2024 … Jan 2025; January stays before the featured rows (days 1–2).
    const slot = Math.floor(random() * 12);
    const year = slot === 11 ? 2025 : 2024;
    const month = slot === 11 ? 1 : slot + 2;
    const day = slot === 11 ? 1 + Math.floor(random() * 2) : 1 + Math.floor(random() * 28);
    const [cnaeCodigo, cnaeDescricao] = pick(CNAES, random);

    rows.push({
      cnpj,
      razaoSocial: `${pick(PREFIXES, random)} ${pick(KINDS, random)} ${pick(SUFFIXES, random)}`,
      cnaeCodigo,
      cnaeDescricao,
      municipio: pickWeighted(CITIES, random),
      uf: "MT",
      capitalSocial: pick(CAPITALS, random),
      abertura: `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`,
      situacao: pickWeighted(SITUACOES, random),
    });
  }
  return rows;
}

export const DEMO_COMPANIES: readonly Company[] = [...FEATURED, ...generated(TOTAL - FEATURED.length)];
