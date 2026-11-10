/**
 * Starting points shown on the empty assistant page, grouped by what the person wants to
 * do. Every question is part of a versioned evaluation set
 * (`crates/opencnpj-lab/eval/prospecting-01`, `skills-export-01`, `skills-company-01`), so
 * the page only offers questions the interpreter is tested to answer.
 */
export interface ExampleGroup {
  title: string;
  questions: readonly string[];
}

export const EXAMPLE_GROUPS: readonly ExampleGroup[] = [
  {
    title: "Encontrar clientes",
    questions: [
      "dentistas em Cuiabá/MT com telefone",
      "novas empresas em Sinop/MT",
      "liste os maiores restaurantes de Goiânia/GO",
      "telefones de imobiliárias em Londrina/PR",
    ],
  },
  {
    title: "Entender o mercado",
    questions: [
      "Quantas farmácias ativas existem em Sinop/MT?",
      "Quais as 10 cidades de MT com mais padarias ativas?",
      "academias abertas em 2024 em Goiânia/GO",
      "Compare a densidade por 10 mil habitantes de farmácias ativas entre Cuiabá/MT e Várzea Grande/MT.",
    ],
  },
  {
    title: "Consultar uma empresa",
    questions: ["33.000.167/0001-01", "filiais do Bradesco em MT", "Encontre a empresa Zaflas"],
  },
  {
    title: "Levar para a planilha",
    questions: [
      "Me mande um Excel com nome, CNPJ e telefone das padarias de Cuiabá MT",
      "Quero um CSV das padarias que fecharam em 2025 em Cuiabá/MT",
    ],
  },
];
