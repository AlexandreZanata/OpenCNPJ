# Data Formats (Receita Federal CNPJ)

Padrao dos arquivos:
- Separador `;`
- Campos entre aspas duplas
- Encoding de origem comum: `ISO-8859-1` (converter para UTF-8 na leitura)
- Datas no formato `YYYYMMDD`
- Data `"00000000"` deve ser tratada como `NULL`

## EMPRESAS (`*.EMPRECSV`)

- Regex de nome: `^K3241\.K03200Y0\.D\d{5}\.EMPRECSV$`
- Colunas: `7`

| idx | campo                        | Go              | SQL             | validacao/regras                                  | exemplo |
|-----|------------------------------|-----------------|-----------------|---------------------------------------------------|---------|
| 0   | cnpj_basico                  | string          | char(8)         | 8 digitos                                         | 41273590 |
| 1   | razao_social                 | string          | text            | texto livre                                       | MARIA DAS MERCES SOARES LEMOS |
| 2   | natureza_juridica            | string          | varchar(4)      | lookup em `naturezas`                             | 4014 |
| 3   | qualificacao_responsavel     | string          | varchar(2)      | lookup em `qualificacoes`                         | 34 |
| 4   | capital_social               | decimal.Decimal | numeric(20,2)   | formato BR (`1.000,00` / `0,00`)                 | 1000,00 |
| 5   | porte_empresa                | string          | varchar(2)      | dominio RFB                                       | 01 |
| 6   | ente_federativo_responsavel  | string          | text            | opcional (pode vir vazio)                         | (vazio) |

## ESTABELECIMENTOS (`*.ESTABELE`)

- Regex de nome: `^K3241\.K03200Y0\.D\d{5}\.ESTABELE$`
- Colunas: `30`

| idx | campo                          | Go     | SQL           | validacao/regras                           | exemplo |
|-----|--------------------------------|--------|---------------|--------------------------------------------|---------|
| 0   | cnpj_basico                    | string | char(8)       | 8 digitos                                  | 43217850 |
| 1   | cnpj_ordem                     | string | char(4)       | 4 digitos                                  | 0051 |
| 2   | cnpj_dv                        | string | char(2)       | 2 digitos                                  | 18 |
| 3   | id_matriz_filial               | int16  | smallint      | dominio RFB (1=matriz,2=filial)            | 2 |
| 4   | nome_fantasia                  | string | text          | opcional                                   | SC SPORT'S |
| 5   | situacao_cadastral             | int16  | smallint      | dominio RFB                                | 08 |
| 6   | data_situacao_cadastral        | *Date  | date          | `YYYYMMDD`; `00000000` => NULL             | 20070402 |
| 7   | motivo_situacao_cadastral      | string | varchar(2)    | lookup em `motivos`                        | 01 |
| 8   | nome_cidade_exterior           | string | text          | opcional                                   | (vazio) |
| 9   | pais                           | string | varchar(3)    | lookup em `paises`                         | 000 |
| 10  | data_inicio_atividade          | *Date  | date          | `YYYYMMDD`; `00000000` => NULL             | 20050815 |
| 11  | cnae_fiscal_principal          | string | varchar(7)    | lookup em `cnaes`                          | 4761001 |
| 12  | cnae_fiscal_secundaria         | string | text          | lista separada por virgula                 | 7020400,7490105 |
| 13  | tipo_logradouro                | string | varchar(20)   | opcional                                   | RUA |
| 14  | logradouro                     | string | text          | opcional                                   | DO BISPO |
| 15  | numero                         | string | varchar(20)   | opcional; pode vir `S N`                   | 83 |
| 16  | complemento                    | string | text          | opcional                                   | APT 707 BLC A |
| 17  | bairro                         | string | text          | opcional                                   | RIO COMPRIDO |
| 18  | cep                            | string | char(8)       | 8 digitos quando preenchido                | 20261063 |
| 19  | uf                             | string | char(2)       | UF valida                                  | RJ |
| 20  | municipio                      | string | varchar(4)    | lookup em `municipios`                     | 6001 |
| 21  | ddd_1                          | string | varchar(4)    | opcional                                   | 011 |
| 22  | telefone_1                     | string | varchar(20)   | opcional                                   | 21887350 |
| 23  | ddd_2                          | string | varchar(4)    | opcional                                   | (vazio) |
| 24  | telefone_2                     | string | varchar(20)   | opcional                                   | (vazio) |
| 25  | ddd_fax                        | string | varchar(4)    | opcional                                   | 011 |
| 26  | fax                            | string | varchar(20)   | opcional                                   | 21887205 |
| 27  | correio_eletronico             | string | text          | opcional                                   | rosania.oliveira@iob.com.br |
| 28  | situacao_especial              | string | text          | opcional                                   | (vazio) |
| 29  | data_situacao_especial         | *Date  | date          | `YYYYMMDD`; `00000000` => NULL             | (vazio) |

## SOCIOS (`*.SOCIOCSV`)

- Regex de nome: `^K3241\.K03200Y0\.D\d{5}\.SOCIOCSV$`
- Colunas: `11`

| idx | campo                              | Go     | SQL         | validacao/regras                              | exemplo |
|-----|------------------------------------|--------|-------------|-----------------------------------------------|---------|
| 0   | cnpj_basico                        | string | char(8)     | 8 digitos                                     | 41481283 |
| 1   | identificador_socio                | string | varchar(1)  | dominio RFB (PF/PJ/estrangeiro)              | 2 |
| 2   | nome_socio_razao_social            | string | text        | texto livre                                   | LEONARDO FISTAROL PEDOTTI |
| 3   | cpf_cnpj_socio                     | string | varchar(14) | mascarado; manter literal                     | ***261720** |
| 4   | qualificacao_socio                 | string | varchar(2)  | lookup em `qualificacoes`                     | 49 |
| 5   | data_entrada_sociedade             | *Date  | date        | `YYYYMMDD`; `00000000` => NULL                | 20210407 |
| 6   | pais                               | string | varchar(3)  | lookup em `paises`; pode vazio                | (vazio) |
| 7   | representante_legal                | string | varchar(14) | mascarado; manter literal                     | ***000000** |
| 8   | nome_representante                 | string | text        | opcional                                      | (vazio) |
| 9   | qualificacao_representante_legal   | string | varchar(2)  | lookup em `qualificacoes`                     | 00 |
| 10  | faixa_etaria                       | string | varchar(1)  | dominio RFB                                   | 3 |

## SIMPLES / MEI (`*.SIMPLES`)

- Regex de nome: `^K3241\.K03200Y0\.D\d{5}\.SIMPLES$`
- Colunas: `7`

| idx | campo                      | Go    | SQL         | validacao/regras                      | exemplo |
|-----|----------------------------|-------|-------------|---------------------------------------|---------|
| 0   | cnpj_basico                | string| char(8)     | 8 digitos                             | 00000011 |
| 1   | opcao_simples              | string| char(1)     | `S`/`N`                               | S |
| 2   | data_opcao_simples         | *Date | date        | `YYYYMMDD`; `00000000` => NULL        | 20070701 |
| 3   | data_exclusao_simples      | *Date | date        | `YYYYMMDD`; `00000000` => NULL        | 00000000 |
| 4   | opcao_mei                  | string| char(1)     | `S`/`N`                               | N |
| 5   | data_opcao_mei             | *Date | date        | `YYYYMMDD`; `00000000` => NULL        | 00000000 |
| 6   | data_exclusao_mei          | *Date | date        | `YYYYMMDD`; `00000000` => NULL        | 00000000 |

## TABELAS DE REFERENCIA (lookups)

### CNAES (`*.CNAECSV`)

- Colunas: `2`
- Exemplo: `"0111301";"Cultivo de arroz"`

| idx | campo      | Go     | SQL          | validacao/regras | exemplo |
|-----|------------|--------|--------------|------------------|---------|
| 0   | codigo     | string | varchar(7)   | chave lookup     | 0111301 |
| 1   | descricao  | string | text         | UTF-8            | Cultivo de arroz |

### NATUREZAS (`*.NATJUCSV`)

- Colunas: `2`
- Exemplo: `"4014";"Empresa Individual Imobiliária"`

| idx | campo      | Go     | SQL          | validacao/regras | exemplo |
|-----|------------|--------|--------------|------------------|---------|
| 0   | codigo     | string | varchar(4)   | chave lookup     | 4014 |
| 1   | descricao  | string | text         | UTF-8            | Empresa Individual Imobiliaria |

### QUALIFICACOES (`*.QUALSCSV`)

- Colunas: `2`
- Exemplo: `"22";"Sócio"`

| idx | campo      | Go     | SQL          | validacao/regras | exemplo |
|-----|------------|--------|--------------|------------------|---------|
| 0   | codigo     | string | varchar(2)   | chave lookup     | 22 |
| 1   | descricao  | string | text         | UTF-8            | Socio |

### MUNICIPIOS (`*.MUNICCSV`)

- Colunas: `2`
- Exemplo esperado: `"6001";"RIO DE JANEIRO"` (codigo IBGE/RFB)

| idx | campo      | Go     | SQL          | validacao/regras | exemplo |
|-----|------------|--------|--------------|------------------|---------|
| 0   | codigo     | string | varchar(4)   | chave lookup     | 6001 |
| 1   | descricao  | string | text         | UTF-8            | RIO DE JANEIRO |

### PAISES (`*.PAISCSV`)

- Colunas: `2`
- Exemplo: `"013";"AFEGANISTAO"`

| idx | campo      | Go     | SQL          | validacao/regras | exemplo |
|-----|------------|--------|--------------|------------------|---------|
| 0   | codigo     | string | varchar(3)   | chave lookup     | 013 |
| 1   | descricao  | string | text         | UTF-8            | AFEGANISTAO |

### MOTIVOS (`*.MOTICSV`)

- Colunas: `2`
- Exemplo: `"01";"EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA"`

| idx | campo      | Go     | SQL          | validacao/regras | exemplo |
|-----|------------|--------|--------------|------------------|---------|
| 0   | codigo     | string | varchar(2)   | chave lookup     | 01 |
| 1   | descricao  | string | text         | UTF-8            | EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA |

## Regras obrigatorias do parser

- `cnpj_basico`: exatamente 8 digitos.
- Datas: `YYYYMMDD`; `"00000000"` e vazio => `NULL`.
- Capital social: converter formato BR para `decimal.Decimal`.
- Campos de lookup devem existir em memoria antes da carga principal.
- Campos mascarados (CPF/CNPJ de socio e representante): nao normalizar, manter literal.
