#!/usr/bin/env python3
"""Build WP-1.3 catalogue artifacts from local, versioned source files."""

import argparse
from collections import defaultdict
import csv
import hashlib
import io
import json
from pathlib import Path
import re
import tempfile
import tomllib
import unicodedata
import xml.etree.ElementTree as ET
import zipfile


CATALOG = Path(__file__).resolve().parents[2] / "opencnpj-insights" / "catalog"
TABLE = "{urn:oasis:names:tc:opendocument:xmlns:table:1.0}"
FORMATTED_CODE = re.compile(r"(\d{4})-(\d)/(\d{2})(?:\s*\*)?", re.ASCII)
EXACT_CODE = re.compile(r"[0-9]{7}")
SOURCES = {
    "ibge-subclasses.json": "https://servicodados.ibge.gov.br/api/v2/cnae/subclasses",
    "concla-2.3.zip": "https://ftp.ibge.gov.br/Informacoes_Gerais_e_Referencia/Classificacoes/CNAE/cnae2_3/cnae_subclasses_2_3_ods.zip",
    "concla-2.2.zip": "https://ftp.ibge.gov.br/Informacoes_Gerais_e_Referencia/Classificacoes/CNAE/cnae2_2/cnae_2_2_subclasses_ods_20150609.zip",
}
EXCEPTION_SOURCES = {
    "concla-2010": "https://www.gov.br/mme/pt-br/arquivos/do-29-06-2010-s1.pdf",
    "concla-2007": "https://cnae.ibge.gov.br/images/concla/resolucoes_e_atas/ataconcla20_22032007.pdf",
    "concla-2.2-notes": "https://cnae.ibge.gov.br/images/concla/downloads/cnae-subclasses-2-2-notas-explicativas.pdf",
    "rfb-reference": "rfb_cnaes.tsv",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def tsv_rows(path):
    with path.open(encoding="utf-8", newline="") as stream:
        yield from csv.DictReader(stream, delimiter="\t")


def write_tsv(path, header, rows):
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(header)
        writer.writerows(rows)


def code_from_cell(text):
    match = FORMATTED_CODE.fullmatch(text)
    return "".join(match.groups()) if match else None


def ods_rows(path, member, width):
    with zipfile.ZipFile(path) as archive:
        names = [name for name in archive.namelist() if member in name]
        require(len(names) == 1, f"{path}: expected exactly one {member!r} workbook")
        with zipfile.ZipFile(io.BytesIO(archive.read(names[0]))) as workbook:
            root = ET.fromstring(workbook.read("content.xml"))
    sheets = root.findall(f".//{TABLE}table")
    require(len(sheets) == 1, f"{path}: expected one worksheet in {member}")
    for row in sheets[0].findall(f"{TABLE}table-row"):
        cells = []
        for cell in row:
            if cell.tag not in (f"{TABLE}table-cell", f"{TABLE}covered-table-cell"):
                continue
            text = "".join(cell.itertext()).strip()
            count = int(cell.get(f"{TABLE}number-columns-repeated", "1"))
            cells.extend([text] * min(count, width - len(cells)))
            if len(cells) == width:
                break
        yield cells + [""] * (width - len(cells))


def correspondences(path, member):
    mappings = defaultdict(dict)
    previous = None
    for left, right, flag in ods_rows(path, member, 3):
        origin = code_from_cell(left)
        target = code_from_cell(right)
        if origin or left:
            previous = origin
        if target:
            require(previous is not None, f"{path}: destination {target} has no source code")
            partial = "*" in right or flag == "*"
            mappings[previous][target] = partial or mappings[previous].get(target, False)
    require(bool(mappings), f"{path}: empty correspondence sheet")
    return mappings


def read_hierarchy(path):
    nodes = {}
    subclasses = {}
    phrases = 0
    for subclass in json.loads(path.read_text(encoding="utf-8")):
        code = subclass["id"]
        require(EXACT_CODE.fullmatch(code) and code not in subclasses, f"invalid or repeated subclass {code}")
        subclasses[code] = subclass
        phrases += len(subclass["atividades"])
        c = subclass["classe"]
        g = c["grupo"]
        d = g["divisao"]
        s = d["secao"]
        for row, level, parent in (
            (subclass, "subclass", c["id"]), (c, "class", g["id"]), (g, "group", d["id"]),
            (d, "division", s["id"]), (s, "section", ""),
        ):
            key = row["id"]
            node = (level, parent, " ".join(row["descricao"].split()))
            require(nodes.setdefault(key, node) == node, f"inconsistent hierarchy for {key}")
    require(bool(subclasses), "IBGE API has no subclasses")
    return nodes, subclasses, phrases


def legacy_map(raw, reference, official):
    two_to_three = correspondences(raw / "concla-2.3.zip", "2_2 x")
    one_to_two = correspondences(raw / "concla-2.2.zip", "correspondencia2.ods")
    overrides = {}
    for row in tsv_rows(CATALOG / "cnae_legacy_exceptions.tsv"):
        code = row["code"]
        targets = tuple(filter(None, row["targets"].split(",")))
        require(code not in overrides and code in reference and code not in official, f"unexpected exception {code}")
        require(set(targets) <= official and (bool(targets) or row["relation"] == "unreported"), f"invalid exception targets for {code}")
        require(set(row["source"].split(";")) <= EXCEPTION_SOURCES.keys() and row["note"], f"missing exception provenance for {code}")
        overrides[code] = row["relation"], targets, row["source"], row["note"]
    mapped = {}
    for code in sorted(reference.keys() - official):
        if code in overrides:
            mapped[code] = overrides[code]
            continue
        direct = two_to_three.get(code, {})
        if direct and direct.keys() <= official:
            targets = direct
            source = "concla-2.3"
        else:
            targets = {}
            for interim, first_partial in one_to_two.get(code, {}).items():
                links = two_to_three.get(interim, {interim: False})
                require(links.keys() <= official, f"obsolete intermediate target for RFB CNAE {code}: {interim}")
                for target, second_partial in links.items():
                    targets[target] = first_partial or second_partial or targets.get(target, False)
            source = "concla-2.2;concla-2.3"
        require(bool(targets), f"RFB CNAE {code} has no documented mapping")
        relation = "partial" if any(targets.values()) else "split" if len(targets) > 1 else "successor"
        mapped[code] = relation, tuple(sorted(targets)), source, "CONCLA correspondence. Retain the original RFB code for execution."
    return mapped


def fold(text):
    folded = unicodedata.normalize("NFKD", text.casefold())
    plain = "".join(ch for ch in folded if not unicodedata.combining(ch))
    return " ".join(re.findall(r"[a-z0-9]+", plain))


def sectors_and_aliases(path, valid):
    with path.open("rb") as file:
        sectors = tomllib.load(file)["sector"]
    ids = set()
    aliases = defaultdict(set)
    scopes = {}
    for sector in sectors:
        name = sector["id"]
        require(re.fullmatch(r"[a-z][a-z0-9_]*", name) and name not in ids, f"invalid or duplicate sector {name}")
        ids.add(name)
        core = sector["cnae_core"]
        extended = sector["cnae_extended"]
        require(core and core == sorted(set(core)) and extended == sorted(set(extended)), f"invalid CNAE sets in {name}")
        require(not (set(core) & set(extended)), f"core and extension overlap in {name}")
        scopes[name] = set(core) | set(extended)
        require(scopes[name] <= valid, f"invalid sector codes in {name}: {sorted(scopes[name] - valid)}")
        display = sector["display"]
        require(display["gender"] in {"m", "f"} and sector["scope"] == "main" and sector["since"] == "v1", f"invalid display or scope for {name}")
        require(bool(sector["aliases"]), f"no aliases for {name}")
        for label in [display["sing"], display["plural"], *sector["aliases"]]:
            alias = fold(label)
            require(bool(alias), f"empty normalized alias in {name}")
            aliases[alias].add(name)
    require(len(sectors) >= 200, "sector catalogue requires at least 200 curated entries")
    return sectors, aliases, scopes


def automatic_aliases(subclasses):
    candidates = set()
    for code, subclass in subclasses.items():
        for text, source in [(subclass["descricao"], "description"), *((text, "activity") for text in subclass["atividades"])]:
            words = fold(text)
            if words:
                candidates.add((words, code, source))
            if ";" in text:
                subject, action = text.rsplit(";", 1)
                if fold(action) and fold(subject):
                    candidates.add((fold(f"{action} {subject}"), code, "reordered_activity"))
            head = re.sub(r"^(comercio (?:varejista|atacadista)(?: especializado)?|fabricacao|cultivo|criacao) de ", "", words)
            if head != words and len(head.split()) >= 2:
                candidates.add((head, code, "head_phrase"))
    return sorted(candidates)


def digest(path):
    data = path.read_bytes()
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def build(raw, rfb, sector_path, out):
    nodes, subclasses, phrases = read_hierarchy(raw / "ibge-subclasses.json")
    official = set(subclasses)
    structure = {code for row in ods_rows(raw / "concla-2.3.zip", "Estrutura Detalhada", 6) for cell in row if (code := code_from_cell(cell))}
    require(official == structure, f"IBGE and CONCLA subclass sets differ: {sorted(official ^ structure)}")
    reference = {}
    for row in tsv_rows(rfb / "rfb_cnaes.tsv"):
        code = row["code"]
        require(EXACT_CODE.fullmatch(code) and code not in reference, f"invalid or duplicate RFB reference code {code}")
        reference[code] = row["label"]
    mappings = legacy_map(raw, reference, official)
    valid = official | {code for code, row in mappings.items() if row[0] != "unreported"}
    counts = {}
    for row in tsv_rows(rfb / "used_codes.tsv"):
        if row["field"] in ("cnae", "cnae_secundaria"):
            require(row["code"] in official or row["code"] in mappings, f"unclassified used CNAE {row['code']}")
        if row["field"] == "cnae":
            require(row["code"] not in counts, f"duplicate main CNAE {row['code']}")
            counts[row["code"]] = int(row["document_count"])
    sectors, curated, scopes = sectors_and_aliases(sector_path, valid)
    auto = automatic_aliases(subclasses)
    by_phrase = defaultdict(set)
    for alias, code, _ in auto:
        by_phrase[alias].add(code)
    ambiguous = []
    for alias in sorted(curated.keys() | by_phrase.keys()):
        sector_ids = curated.get(alias, set())
        codes = by_phrase.get(alias, set())
        curated_codes = set().union(*(scopes[sector] for sector in sector_ids))
        if len(sector_ids) > 1 or len(codes) > 1 or (sector_ids and codes - curated_codes):
            ambiguous.append((alias, ",".join(sorted(sector_ids)), ",".join(sorted(codes))))

    automatic_codes = {code for codes in by_phrase.values() for code in codes}
    unique_automatic = {next(iter(codes)) for alias, codes in by_phrase.items() if len(codes) == 1 and alias not in curated}
    curated_codes = set().union(*scopes.values())
    unique_curated = set().union(*(scopes[next(iter(ids))] for ids in curated.values() if len(ids) == 1))
    output_rows = [(code, *row, "", "", "", "") for code, row in nodes.items()]
    for code, (relation, targets, source, note) in mappings.items():
        parent = code[:5] if code[:5] in nodes else ""
        output_rows.append((code, "legacy", parent, reference[code], relation, ",".join(targets), source, note))
    top = sorted(sectors, key=lambda s: (-sum(counts.get(code, 0) for code in s["cnae_core"]), s["id"]))[:50]

    require(not out.exists(), f"destination already exists: {out}")
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".cnae-", dir=out.parent) as folder:
        target = Path(folder)
        write_tsv(target / "cnae.tsv", ("code", "level", "parent", "name", "legacy_relation", "legacy_targets", "legacy_source", "legacy_note"), sorted(output_rows))
        write_tsv(target / "sector_aliases_auto.tsv", ("alias", "code", "source"), auto)
        write_tsv(target / "ambiguity.tsv", ("alias", "sectors", "auto_codes"), ambiguous)
        write_tsv(target / "reachability.tsv", ("code", "automatic", "curated", "unambiguous"),
                  ((code, int(code in automatic_codes), int(code in curated_codes), int(code in unique_automatic or code in unique_curated)) for code in sorted(official | mappings.keys())))
        write_tsv(target / "top-50.tsv", ("sector", "core_main_documents"),
                  ((s["id"], sum(counts.get(code, 0) for code in s["cnae_core"])) for s in top))
        (target / "sectors.toml").write_bytes(sector_path.read_bytes())
        manifest = {
            "version": "v1", "cnae_version": "2.3",
            "sources": {name: {"url": url, **digest(raw / name)} for name, url in SOURCES.items()},
            "legacy_exception_sources": EXCEPTION_SOURCES,
            "legacy_exceptions": digest(CATALOG / "cnae_legacy_exceptions.tsv"),
            "rfb_inputs": {name: digest(rfb / name) for name in ("rfb_cnaes.tsv", "used_codes.tsv")},
            "artifacts": {path.name: digest(path) for path in sorted(target.iterdir())},
        }
        (target / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        target.rename(out)
    print(f"official subclasses: {len(official)}, activity phrases: {phrases}, hierarchy nodes: {len(nodes)}")
    print(f"RFB codes: {len(reference)}, covered by official hierarchy: {len(reference.keys() & official)}, legacy dispositions: {len(mappings)}")
    print(f"curated sectors: {len(sectors)}, automatic alias rows: {len(auto)}, ambiguous phrases: {len(ambiguous)}")
    print(f"codes without an unambiguous alias: {len((official | mappings.keys()) - unique_automatic - unique_curated)}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw", type=Path, required=True, help="directory with the IBGE and CONCLA downloads")
    parser.add_argument("--rfb", type=Path, required=True, help="verified catalog rfb extraction directory")
    parser.add_argument("--sectors", type=Path, default=CATALOG / "sectors.toml")
    parser.add_argument("--out", type=Path, required=True, help="new catalogue output directory under /data")
    args = parser.parse_args()
    build(args.raw, args.rfb, args.sectors, args.out)
