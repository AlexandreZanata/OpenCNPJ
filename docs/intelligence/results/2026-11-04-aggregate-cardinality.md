# 2026-11-04 — Cardinality of candidate aggregate tables [MEASURED]

Purpose: size the local aggregates ("cube") that would let a 1 vCPU / 1 GB chat node answer
counts, rankings, densities and time series without the 8.3 GB national snapshot.

* Data: national release 2026-10, Parquet baseline built by `scripts/baselines.sh`
  (`/data/opencnpj-data/baselines/parquet/*.parquet`, DuckDB CSV parse of the official ZIPs).
* Host: i7-13620H, 31 GiB, NVMe; DuckDB CLI, 8 threads, 8 GB memory limit.
* Caveat: the Parquet baseline is **not** the ground truth. About ten source rows are
  shifted in it (municipality column holding a UF or CEP, see the last query), while the
  `.ocnpj` snapshot parses the same rows correctly (`verify --sources` passed). Counts below
  are therefore exact to within a handful of rows; the real aggregates must be built from
  the snapshot (WP-1.4).
* Column legend: `m` município (RFB/TOM code), `c` main CNAE subclass, `s` situação
  cadastral, `mf` matriz/filial, `y` year and `ym` year-month of `data_inicio_atividade`,
  `ds` `data_situacao_cadastral`, `cs` secondary CNAE list.

## Commands

```sql
-- cube.sql, run as: duckdb -csv < cube.sql
SET memory_limit='8GB'; SET threads=8;
.timer on
CREATE TEMP VIEW e AS SELECT municipio m, uf, cnae_fiscal_principal c, situacao_cadastral s,
  identificador_matriz_filial mf, substr(data_inicio_atividade,1,4) y,
  substr(data_inicio_atividade,1,6) ym, cnpj_basico b
  FROM '/data/opencnpj-data/baselines/parquet/estabelecimentos.parquet';
SELECT count(*) estabs, count(*) FILTER (WHERE s='02') active, count(DISTINCT m) municipios, count(DISTINCT c) cnaes FROM e;
SELECT s, count(*) FROM e GROUP BY s ORDER BY 2 DESC;
SELECT count(*) AS municipio_codes_with_multiple_uf FROM (SELECT m FROM e WHERE uf<>'EX' GROUP BY m HAVING count(DISTINCT uf)>1);
SELECT count(*) pairs_all FROM (SELECT DISTINCT m,c FROM e);
SELECT count(*) pairs_active FROM (SELECT DISTINCT m,c FROM e WHERE s='02');
SELECT count(*) cells_m_c_s_mf FROM (SELECT DISTINCT m,c,s,mf FROM e);
SELECT count(*) cells_m_c_s_mf_year FROM (SELECT DISTINCT m,c,s,mf,y FROM e);
SELECT count(*) cells_active_m_c_mf_ym FROM (SELECT DISTINCT m,c,mf,ym FROM e WHERE s='02');
SELECT count(*) cells_active_m_c_mf_porte_mei FROM (SELECT DISTINCT e.m,e.c,e.mf,em.porte_empresa,coalesce(si.opcao_mei,'')
  FROM e JOIN '/data/opencnpj-data/baselines/parquet/empresas.parquet' em ON em.cnpj_basico=e.b
  LEFT JOIN '/data/opencnpj-data/baselines/parquet/simples.parquet' si ON si.cnpj_basico=e.b WHERE e.s='02');
```

```sql
-- flows.sql (same settings; view e also exposes cs, di = data_inicio_atividade, ds)
SELECT count(*) open_cells_m_c_year_all FROM (SELECT DISTINCT m,c,substr(di,1,4) FROM e);
SELECT count(*) open_cells_m_c_ym_since2021 FROM (SELECT DISTINCT m,c,substr(di,1,6) FROM e WHERE di>='20210101');
SELECT count(*) open_rows_since2021 FROM e WHERE di>='20210101';
SELECT count(*) close_cells_m_c_ym_since2021 FROM (SELECT DISTINCT m,c,substr(ds,1,6) FROM e WHERE s='08' AND ds>='20210101');
SELECT count(*) status_cells_m_c_s_ym_since2021 FROM (SELECT DISTINCT m,c,s,substr(ds,1,6) FROM e WHERE ds>='20210101');
SELECT count(*) sec_pairs_active FROM (SELECT DISTINCT m, unnest(string_split(cs, ',')) x FROM e WHERE s='02' AND cs<>'');
SELECT count(*) sec_pairs_rows_active FROM (SELECT m, unnest(string_split(cs, ',')) x FROM e WHERE s='02' AND cs<>'');
SELECT min(di), max(di) FROM e WHERE di>'19000101' AND di<'20990101';
SELECT substr(di,1,4) y, count(*) FROM e WHERE di>='20220101' GROUP BY 1 ORDER BY 1;
```

## Raw output

```
estabs,active,municipios,cnaes
73809379,28380653,5583,1343
s,count_star()
08,34829938
02,28380653
04,10165173
03,323068
01,110547
municipio_codes_with_multiple_uf
6
pairs_all
2030950
pairs_active
1483948
cells_m_c_s_mf
4567317
cells_m_c_s_mf_year
19388497
cells_active_m_c_mf_ym
14165496
cells_active_m_c_mf_porte_mei
2825808            (Run Time real 8.467 s — the only query with joins)

open_cells_m_c_year_all
13130956
open_cells_m_c_ym_since2021
8717966
open_rows_since2021
26891437
close_cells_m_c_ym_since2021
5422112
status_cells_m_c_s_ym_since2021
14380535
sec_pairs_active
2793162
sec_pairs_rows_active
70509026
min(di),max(di)
19010101,20261011
y,count_star()
2022,3977694
2023,3985063
2024,4844307
2025,5195568
2026,4749185
```

Every query except the join ran in 0.02–2.6 s.

## Municipality code anomalies

```sql
-- municipios.csv = Municipios.zip member transcoded with iconv -f latin1 -t utf8
SELECT 'multi_uf' k, e.m, any_value(r.nm) nm, string_agg(DISTINCT e.uf, ',') ufs, count(*) n
  FROM e LEFT JOIN r ON r.code=e.m
  WHERE e.m IN (SELECT m FROM e WHERE uf<>'EX' GROUP BY m HAVING count(DISTINCT uf)>1) GROUP BY e.m;
```

```
k,m,nm,ufs,n
multi_uf,ES,NULL,"29175584,29190014",2
multi_uf,DF,NULL,"70307900,72309200",2
multi_uf,SP,NULL,"07190100,16402320",2
multi_uf,6969,RIBEIRAO PRETO,"SP,PA",386871
multi_uf,0403,ACARA,"SC,PA",4679
multi_uf,GO,NULL,"75095795,76155000",2
uf,n
EX,173410
table_codes_without_estabs
0
```

Rows with `m` = a UF and `ufs` = CEPs are the shifted Parquet rows; `EX` = establishments
abroad. Codes 6969 and 0403 are real municipalities with some rows in another UF.

Reference tables of the release (`wc -l` after `iconv`): Municipios 5,572 rows (4-digit
code; name in uppercase without accents; **no UF column**; 241 names occur more than once,
e.g. "SAO DOMINGOS" ×5, "BOM JESUS" ×5), Cnaes 1,359 subclasses (no section/division/group/
class names), Naturezas 92, Motivos 63.

## What this means [ESTIMATE]

* An active-stock table keyed by (município, main CNAE, matriz/filial, porte, MEI) has
  **2.83 M cells**. Packed key (13 + 11 + 1 + 2 + 1 = 28 bits) + `u32` count = 8 B/cell →
  **22.6 MB raw**; sorted keys delta-encode well, so 6–12 MB on disk is expected (to be
  measured in WP-1.4).
* Monthly registration flows since 2021 by (município, CNAE): **8.7 M cells**; closures
  (baixas) by month: **5.4 M cells**. Raw ≈ 6 B/cell → ~85 MB raw for both, plausibly
  25–40 MB compressed.
* Secondary-CNAE presence for active establishments: **2.79 M** (município, CNAE) cells.
* Conclusion: one competence of aggregates fits in tens of MB; 24 monthly stock tables
  (history) add ~24 × 10 MB. All of it fits a 1 GB node as memory-mapped read-only files.
* UF must come from the official TOM ↔ IBGE table, not from the data (two real codes have
  rows in another UF).
