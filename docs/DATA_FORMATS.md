# Data Formats (Receita Federal CNPJ)

File conventions:

- Separator: `;`
- Fields wrapped in double quotes
- Source encoding: `ISO-8859-1` (convert to UTF-8 on read)
- Dates: `YYYYMMDD` format
- Date `"00000000"` must be treated as `NULL`

## EMPRESAS (`*.EMPRECSV`)

- Filename regex: `^K3241\.K03200Y0\.D\d{5}\.EMPRECSV$`
- Columns: `7`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | cnpj_basico | string | char(8) | 8 digits | 41273590 |
| 1 | razao_social | string | text | free text | MARIA DAS MERCES SOARES LEMOS |
| 2 | natureza_juridica | string | varchar(4) | lookup in `naturezas` | 4014 |
| 3 | qualificacao_responsavel | string | varchar(2) | lookup in `qualificacoes` | 34 |
| 4 | capital_social | decimal.Decimal | numeric(20,2) | BR format (`1.000,00` / `0,00`) | 1000,00 |
| 5 | porte_empresa | string | varchar(2) | RFB domain | 01 |
| 6 | ente_federativo_responsavel | string | text | optional (may be empty) | (empty) |

## ESTABELECIMENTOS (`*.ESTABELE`)

- Filename regex: `^K3241\.K03200Y0\.D\d{5}\.ESTABELE$`
- Columns: `30`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | cnpj_basico | string | char(8) | 8 digits | 43217850 |
| 1 | cnpj_ordem | string | char(4) | 4 digits | 0051 |
| 2 | cnpj_dv | string | char(2) | 2 digits | 18 |
| 3 | id_matriz_filial | int16 | smallint | RFB domain (1=hq, 2=branch) | 2 |
| 4 | nome_fantasia | string | text | optional | SC SPORT'S |
| 5 | situacao_cadastral | int16 | smallint | RFB domain | 08 |
| 6 | data_situacao_cadastral | *Date | date | `YYYYMMDD`; `00000000` => NULL | 20070402 |
| 7 | motivo_situacao_cadastral | string | varchar(2) | lookup in `motivos` | 01 |
| 8 | nome_cidade_exterior | string | text | optional | (empty) |
| 9 | pais | string | varchar(3) | lookup in `paises` | 000 |
| 10 | data_inicio_atividade | *Date | date | `YYYYMMDD`; `00000000` => NULL | 20050815 |
| 11 | cnae_fiscal_principal | string | varchar(7) | lookup in `cnaes` | 4761001 |
| 12 | cnae_fiscal_secundaria | string | text | comma-separated list | 7020400,7490105 |
| 13 | tipo_logradouro | string | varchar(20) | optional | RUA |
| 14 | logradouro | string | text | optional | DO BISPO |
| 15 | numero | string | varchar(20) | optional; may be `S N` | 83 |
| 16 | complemento | string | text | optional | APT 707 BLC A |
| 17 | bairro | string | text | optional | RIO COMPRIDO |
| 18 | cep | string | char(8) | 8 digits when present | 20261063 |
| 19 | uf | string | char(2) | valid state code | RJ |
| 20 | municipio | string | varchar(4) | lookup in `municipios` | 6001 |
| 21 | ddd_1 | string | varchar(4) | optional | 011 |
| 22 | telefone_1 | string | varchar(20) | optional | 21887350 |
| 23 | ddd_2 | string | varchar(4) | optional | (empty) |
| 24 | telefone_2 | string | varchar(20) | optional | (empty) |
| 25 | ddd_fax | string | varchar(4) | optional | 011 |
| 26 | fax | string | varchar(20) | optional | 21887205 |
| 27 | correio_eletronico | string | text | optional | rosania.oliveira@iob.com.br |
| 28 | situacao_especial | string | text | optional | (empty) |
| 29 | data_situacao_especial | *Date | date | `YYYYMMDD`; `00000000` => NULL | (empty) |

## SOCIOS (`*.SOCIOCSV`)

- Filename regex: `^K3241\.K03200Y0\.D\d{5}\.SOCIOCSV$`
- Columns: `11`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | cnpj_basico | string | char(8) | 8 digits | 41481283 |
| 1 | identificador_socio | string | varchar(1) | RFB domain (PF/PJ/foreign) | 2 |
| 2 | nome_socio_razao_social | string | text | free text | LEONARDO FISTAROL PEDOTTI |
| 3 | cpf_cnpj_socio | string | varchar(14) | masked; keep literal | ***261720** |
| 4 | qualificacao_socio | string | varchar(2) | lookup in `qualificacoes` | 49 |
| 5 | data_entrada_sociedade | *Date | date | `YYYYMMDD`; `00000000` => NULL | 20210407 |
| 6 | pais | string | varchar(3) | lookup in `paises`; may be empty | (empty) |
| 7 | representante_legal | string | varchar(14) | masked; keep literal | ***000000** |
| 8 | nome_representante | string | text | optional | (empty) |
| 9 | qualificacao_representante_legal | string | varchar(2) | lookup in `qualificacoes` | 00 |
| 10 | faixa_etaria | string | varchar(1) | RFB domain | 3 |

## SIMPLES / MEI (`*.SIMPLES`)

- Filename regex: `^K3241\.K03200Y0\.D\d{5}\.SIMPLES$`
- Columns: `7`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | cnpj_basico | string | char(8) | 8 digits | 00000011 |
| 1 | opcao_simples | string | char(1) | `S`/`N` | S |
| 2 | data_opcao_simples | *Date | date | `YYYYMMDD`; `00000000` => NULL | 20070701 |
| 3 | data_exclusao_simples | *Date | date | `YYYYMMDD`; `00000000` => NULL | 00000000 |
| 4 | opcao_mei | string | char(1) | `S`/`N` | N |
| 5 | data_opcao_mei | *Date | date | `YYYYMMDD`; `00000000` => NULL | 00000000 |
| 6 | data_exclusao_mei | *Date | date | `YYYYMMDD`; `00000000` => NULL | 00000000 |

## REFERENCE TABLES (lookups)

### CNAES (`*.CNAECSV`)

- Columns: `2`
- Example: `"0111301";"Cultivo de arroz"`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | codigo | string | varchar(7) | lookup key | 0111301 |
| 1 | descricao | string | text | UTF-8 | Cultivo de arroz |

### NATUREZAS (`*.NATJUCSV`)

- Columns: `2`
- Example: `"4014";"Empresa Individual Imobiliária"`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | codigo | string | varchar(4) | lookup key | 4014 |
| 1 | descricao | string | text | UTF-8 | Empresa Individual Imobiliaria |

### QUALIFICACOES (`*.QUALSCSV`)

- Columns: `2`
- Example: `"22";"Sócio"`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | codigo | string | varchar(2) | lookup key | 22 |
| 1 | descricao | string | text | UTF-8 | Socio |

### MUNICIPIOS (`*.MUNICCSV`)

- Columns: `2`
- Expected example: `"6001";"RIO DE JANEIRO"` (IBGE/RFB code)

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | codigo | string | varchar(4) | lookup key | 6001 |
| 1 | descricao | string | text | UTF-8 | RIO DE JANEIRO |

### PAISES (`*.PAISCSV`)

- Columns: `2`
- Example: `"013";"AFEGANISTAO"`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | codigo | string | varchar(3) | lookup key | 013 |
| 1 | descricao | string | text | UTF-8 | AFEGANISTAO |

### MOTIVOS (`*.MOTICSV`)

- Columns: `2`
- Example: `"01";"EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA"`

| idx | field | Go | SQL | validation/rules | example |
|-----|-------|-----|-----|------------------|---------|
| 0 | codigo | string | varchar(2) | lookup key | 01 |
| 1 | descricao | string | text | UTF-8 | EXTINCAO POR ENCERRAMENTO LIQUIDACAO VOLUNTARIA |

## Mandatory parser rules

- `cnpj_basico`: exactly 8 digits.
- Dates: `YYYYMMDD`; `"00000000"` and empty => `NULL`.
- Capital social: convert BR format to `decimal.Decimal`.
- Lookup fields must be loaded in memory before main fact-table import.
- Masked fields (partner/representative CPF/CNPJ): do not normalize; keep literal value.
