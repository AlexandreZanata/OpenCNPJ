# Official open-data expansion for commercial intelligence

Researched: 2026-11-08. Priorities are product judgments for the MT pilot,
not measured ROI. These are candidates; this research did not integrate or
download national datasets. Some IBGE geography/population and CNAE resources
are already part of the project: extend them rather than counting them as new.

## Commercial reuse and admission criteria

Official open publication supports commercial analytical services, subject to
the specific resource licence, attribution, personal-data obligations and any
redistribution conditions. Public search access alone is not an open bulk licence.
Do not label every source below legally cleared for unrestricted resale.

The consolidated federal open-data decree defines open licensing and reuse and
encourages private-sector innovation:
https://www2.camara.leg.br/legin/fed/decret/2016/decreto-8777-11-maio-2016-783082-normaatualizada-pe.pdf
Federal implementation guidance:
https://www.gov.br/governodigital/pt-br/estrategias-e-governanca-digital/sisp/guia-do-gestor/pda

Before commercial deployment preserve the exact dataset/resource licence, URL,
terms version, attribution and restrictions. Several resources use open licences,
but this review does not establish a nominal licence for every IBGE/MTE/MT
resource. Verify those resource-level terms rather than inventing CC BY labels.
Examples verified here: ANTT vehicle resource declares Creative Commons
Attribution; the ANEEL distributed-generation dataset declares ODbL.
ODbL requires attention to attribution and share-alike obligations for adapted
databases/public use; it does not automatically mean all application code must
be open. Assess the actual database architecture before distributing a combined
proprietary dataset:
https://opendatacommons.org/licenses/odbl/summary/

Public business contacts are not consent to outreach. Do not expose natural-person
records or treat debt/sanctions as an unqualified credit score. Source status,
scope, effective dates and correction/removal procedures matter.

## Core information: maintain and deepen

### 1. RFB CNPJ plus CNAE/CONCLA and historical releases

Maintain as the identity/establishment spine already implemented. Use full CNPJ
versus root deliberately; maintain source changes over time and versioned sector
definitions. Do not infer revenue from capital or physical opening from a
registration date. Use the project source manifests and data policy for refresh
and access details; this review does not claim to have tested a new RFB endpoint.

### 2. IBGE population, Census, geography and municipal economy

Priority: essential foundation. Join through versioned IBGE municipality codes;
the RFB municipality code is a different identifier. Census aggregates can support
smaller-area analyses only where the released variable/geography supports them.
Population estimates are annual; Census observations remain anchored to their
reference year despite later publication; municipal GDP is annual with lag.
Do not turn GDP per capita into household purchasing power, or a municipal
average into a company's or neighbourhood's income.

Sell: comparable territory indicators, sector establishments per population,
demographic catchments, business expansion scenarios and regional context.
Example: municipalities with growing population and low registered density of
a chosen service, described as an opportunity hypothesis, not proven demand.

Sources:
https://www.ibge.gov.br/estatisticas/sociais/populacao/9103
https://www.ibge.gov.br/estatisticas/sociais/populacao/22827-censo-demografico-2022.html?edicao=39499&t=resultados
https://www.ibge.gov.br/estatisticas/Sociais/populacao/22827-censo-demografico-2022.html?edicao=44663
https://www.ibge.gov.br/estatisticas/economicas/contas-nacionais/9088-produto-interno-bruto-dos-municipios.html?c=3509502&t=pib-por-municipio

Availability differs by edition: the GDP page specifically notes missing
traditional sector detail for its 2022–2023 edition. Check variable metadata
before promising identical columns across years.

CEMPRE is another annual structural benchmark for organizations, local units,
employment and remuneration at released statistical geographies. Its active-unit
definition and coverage must be reconciled with the RFB administrative status;
do not assume the counts are interchangeable or obtain individual company
payroll from aggregate tables:
https://www.ibge.gov.br/estatisticas/economicas/comercio/9016-cadastro-central-de-empresas.html

### 3. MTE/PDET: Novo Caged and RAIS

Priority: essential economic context. Novo Caged provides monthly formal-labour
movement; RAIS supplies annual structural information. Public microdata are
non-identified: do not promise an exact current workforce for every CNPJ.
Join territorial/sectoral statistics using compatible municipality/CNAE/period
definitions, without attempting reidentification. Account for revisions.

Sell: territory/sector momentum, hiring-market analysis, opportunities for HR,
training, meals, uniforms and other employer services.
Example: MT municipalities where formal employment in a selected sector is growing.
Sources:
https://www.gov.br/trabalho-e-emprego/pt-br/assuntos/estatisticas-trabalho
https://www.gov.br/pt-br/servicos/solicitar-consulta-as-estatisticas-da-rais-e-do-caged?id=7839&origem=servico

## Highest-value MT differentiation

### 4. IBGE PAM, PPM and LSPA

Priority: first MT vertical. PAM/PPM offer annual municipal agricultural/livestock
information; LSPA provides monthly crop estimates at its available published
geographies, not a universal monthly municipal production feed.

Sell: agri-input, maintenance, veterinary, machinery and logistics territory
analysis. Compare crop area or livestock indicators with registered related
businesses. An association is not proof that a CNPJ owns a farm, fleet or herd,
nor that a named producer purchases from a particular supplier.

Sources:
https://www.ibge.gov.br/estatisticas/economicas/agricultura-e-pecuaria/9117-pesquisa-da-pecuaria-municipal.html
https://www.ibge.gov.br/estatisticas/economicas/agricultura-e-pecuaria/2041-np-producao-da-pecuaria-municipal.html
https://ftp.ibge.gov.br/Producao_Agricola/Levantamento_Sistematico_da_Producao_Agricola_%5Bmensal%5D/Fasciculo_Indicadores_IBGE/2026/

The first URL's title/path is historically inconsistent; the accessed content
describes PAM. Select the actual published dataset and metadata, not the URL slug.

### 5. Senatran registered fleet

Priority: first MT vertical. Monthly files, aggregated by municipality/type and
other dimensions. The checked 2026 page contains data through September 2026,
published/modified in October: publication is not real-time circulation.

Sell: workshops, tyres, parts, insurance-market territory analysis and vehicle-age
profiles. Example: registered heavy vehicles relative to related establishments.
A municipal fleet total does not identify vehicles owned by each CNPJ.

Source:
https://www.gov.br/transportes/pt-br/assuntos/transito/conteudo-Senatran/frota-de-veiculos-2026

### 6. ANTT RNTRC

Priority: transport specialization after fleet. Distinguish aggregate monthly
transport/fleet history from individually identified resources. Inspect each
resource schema before promising a CNPJ/fleet linkage. A public consultation is
not automatically an unrestricted bulk endpoint. Natural-person carriers need
separate treatment. Operational registration is not service quality or a client list.

Verified resource and licence:
https://dados.antt.gov.br/pt_BR/dataset/rntrc-veiculos/resource/ecd5e2fb-7e9d-46d4-9cda-fa23ab80ae85
Monthly historical-series dictionary:
https://dados.antt.gov.br/dataset/f5a84126-be26-4052-8ca4-9d5694b8d372/resource/71188e11-a531-43fa-b0de-f6cc580efeeb/download/dicionario-de-dados-historico-rntrc.pdf

## Transactional opportunity and qualification

### 7. PNCP plus Compras.gov.br

Priority: high if customers sell to government. APIs expose procurement plans,
notices, contracts, items, results and supplier information depending on endpoint.
Publication follows responsible organizations; no guaranteed universal real-time
coverage or fixed SLA was verified. Polling daily is a proposed ingestion policy,
not an official universal refresh claim.

Sell: bid/opportunity alerts, expiring-contract monitoring, supplier profiles,
public procurement market sizing and awarded-item comparisons.
Join full CNPJ where supplied and procurement identifiers; prevent duplication
between PNCP and Compras. Estimated, awarded, contracted and actually paid values
are different. Procurement awards do not prove private B2B relationships.

Sources:
https://www.gov.br/pncp/pt-br/acesso-a-informacao/dados-abertos
https://dadosabertos.compras.gov.br/swagger-ui/index.html

### 8. CGU CEIS/CNEP

Priority: qualification/compliance module, especially with public procurement.
The checked origin page declares updates every four hours for CEIS/CNEP;
source date and ingestion date must still be displayed because publication lags
and reporting gaps exist. Match CNPJ, actual sanction scope and effective dates.
Absence is not a universal certificate; sanctions are not general credit scores.

Sources:
https://portaldatransparencia.gov.br/origem-dos-dados
https://portaldatransparencia.gov.br/download-de-dados/ceis

### 9. PGFN public active debt

Priority: optional fiscal context, not first demand-discovery feature. Officially
quarterly, with federal/FGTS scope. Preserve debt status, date and source;
do not equate listed debt with insolvency, all tax debt, current delinquency or
lack of payment ability. Do not republish personal debt data in prospect lists.
The dataset's commercial/redistribution terms must be recorded before release.

Source:
https://www.gov.br/pgfn/pt-br/assuntos/divida-ativa-da-uniao/transparencia-fiscal-1/dados-abertos

## Additional verticals

### 10. CNES / Ministry of Health

Health-provider specialization: establishments, services and capacity rather
than CNAE alone; CNES identifier and CNPJ linkage where present. Establishments
must update at least monthly, but national publication and completeness can lag.
Use organization/service fields rather than unnecessary personal-professional data.
Sell qualified healthcare territory and supplier-market analysis.
https://dadosabertos.saude.gov.br/dataset/cnes-cadastro-nacional-de-estabelecimentos-de-saude/resource/4600a142-12c4-4c1c-ae5c-ffcb0cc71e59
https://webatendimento.saude.gov.br/faq/cnes

### 11. INEP Censo Escolar

Annual school/infrastructure/enrolment information. Sell education-supplier,
connectivity and related service territory analysis. School codes are not CNPJ;
verify available linkage fields per edition. Do not infer school-level purchases
or expose student information.
https://www.gov.br/inep/pt-br/acesso-a-informacao/dados-abertos/microdados/censo-escolar

### 12. ANEEL distributed generation

Solar/energy specialization: registered generation projects and capacity.
The catalogue was updated on 2026-11-07; fixed publication cadence was not
confirmed in this review. A generator register does not identify every solar
installer, prove energy consumption or estimate an individual electricity bill.
ODbL is explicitly declared: review adapted-database sharing obligations.
https://dadosabertos.aneel.gov.br/dataset/relacao-de-empreendimentos-de-geracao-distribuida

### 13. ANP fuel prices

Weekly sampled prices and historical series. Sell fuel-market comparisons and
regional operating-cost scenarios. Coverage is a sample, not every station/city
or a live pump-price feed. Station-level and geographic fields depend on resource.
https://www.gov.br/anp/pt-br/assuntos/precos-e-defesa-da-concorrencia/precos/precos-revenda-e-de-distribuicao-combustiveis/informacoes-levantamento-de-precos-de-combustiveis

### 14. MDIC Comex Stat

Monthly trade statistics; October 2026 data updated 2026-11-06 in the checked
portal. Useful for product/territory market analysis and trade-related logistics.
Municipal detail uses SH4 and the declarant's fiscal domicile, which may differ
from the production location. The official FAQ explicitly rules out publishing
company-identifying trade statistics/CNPJ lists. Do not sell a fictional list of
exporting CNPJs derived from municipal totals.
https://comexstat.mdic.gov.br/pt/home
https://www.gov.br/mdic/pt-br/assuntos/comercio-exterior/estatisticas/base-de-dados-bruta
https://www.gov.br/mdic/pt-br/assuntos/comercio-exterior/estatisticas/perguntas-frequentes-faq

### 15. BCB open economic series

Context layer: interest, exchange and other economic series, each with its own
frequency/metadata. Useful for dated scenarios and proper comparisons, not
company-level banking balances, credit limits or account access.
https://www.bcb.gov.br/acessoinformacao/dadosabertos

### 16. MT official data and geography portals

Local complement: inspect the actual datasets, licences, owners, schemas and
latest resources. The official portal exposes economic, environmental and
infrastructure groups. The existence of a group or dashboard does not establish
a usable API, a reliable refresh schedule or a bulk list of state registrations,
licences, invoices or all municipal procurement. Those remain integration
candidates until verified individually.
https://dadosabertos.mt.gov.br/
https://www.seplag.mt.gov.br/portais

## Recommended implementation order

1. Preserve the CNPJ/CNAE/history spine; deepen existing IBGE socioeconomic context.
2. Add Novo Caged/RAIS aggregates for dated local economic momentum.
3. For MT, launch agricultural and vehicle-fleet analytical packs.
4. Add PNCP/Compras when targeting government suppliers, with CEIS/CNEP context.
5. Add specialized CNES, education, energy, fuel and trade packs by actual demand.
6. Add PGFN only with careful fiscal semantics and reviewed resource terms.

## Integration contract for the assistant

Use data as versioned queryable evidence, not facts memorized through question
training. Record business unit, join key, source/competence/publication/ingestion
dates, licence, hashes, coverage, null rules, currency/price basis and revisions.
Preserve point-in-time snapshots to avoid future-data leakage in history.
Maintain CNPJ as a string, explicit root versus establishment and versioned
municipality bridges. Geography-only statistics cannot become CNPJ-level facts.

Every new capability needs typed operations and semantic tests. Validate changed
analytical requests against actual artifacts; do not use plausible answers when
integration is incomplete. Reuse the round-09 corpus to test these capability
boundaries and clearly explain unavailable combinations.
