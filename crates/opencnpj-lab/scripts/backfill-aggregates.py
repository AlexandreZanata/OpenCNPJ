#!/usr/bin/env python3
"""Build and verify one national T1/T2 aggregate at a time."""

import argparse
import fcntl
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
from datetime import date
from pathlib import Path


DATA = Path("/data/opencnpj-data/intelligence")
ROOT = DATA / "backfill"
BIN = Path("/data/opencnpj-target/release")
CATALOGUE = DATA / "catalog-municipalities/2026-10/municipalities.tsv"
IDLE = ["nice", "-n", "19", "ionice", "-c3"]


def competence(value):
    try:
        parsed = date.fromisoformat(f"{value}-01")
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected YYYY-MM") from error
    return parsed


def months(first, last):
    while first <= last:
        yield first.isoformat()[:7]
        if first == last:
            return
        first = date(first.year + first.month // 12, first.month % 12 + 1, 1)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def allocated_bytes(root):
    used = 0
    for directory, _, files in os.walk(root):
        for name in [directory, *(Path(directory) / name for name in files)]:
            try:
                used += os.lstat(name).st_blocks * 512
            except FileNotFoundError:
                # The builder can rename or remove files during sampling.
                continue
    return used


def run(command, output, sample_disk):
    command = [*IDLE, *map(str, command)]
    with output.open("ab") as log:
        log.write(f"\n$ {shlex.join(command)}\n".encode())
        log.flush()
        with subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT) as process:
            try:
                while True:
                    sample_disk()
                    try:
                        status = process.wait(timeout=5)
                        break
                    except subprocess.TimeoutExpired:
                        continue
            except BaseException:
                process.terminate()
                raise
        sample_disk()
        log.write(f"exit status: {status}\n".encode())
    if status:
        raise SystemExit(f"Command failed with status {status}. See {output}")


def checked_record(month, catalogue_sha):
    archive = ROOT / "aggregates"
    manifest = archive / f"{month}.json"
    aggregate = archive / f"{month}.ocagg"
    if not manifest.is_file():
        raise SystemExit(f"Missing archive manifest: {manifest}")
    record = json.loads(manifest.read_text())
    if (
        record["competence"] != month
        or record["municipality_catalogue_sha256"] != catalogue_sha
        or record["source_verification"]["status"] != "passed"
        or digest(aggregate) != record["aggregate_sha256"]
        or aggregate.with_suffix(".ocagg.sha256").read_text().split()[0] != record["aggregate_sha256"]
    ):
        raise SystemExit(f"Archive does not match {manifest}")
    return record


def backfill(month, workers, readers, catalogue_sha):
    sources = ROOT / "raw" / month
    work = ROOT / "work" / month
    logs = ROOT / "logs" / month
    archive = ROOT / "aggregates"
    aggregate = archive / f"{month}.ocagg"
    checksum = aggregate.with_suffix(".ocagg.sha256")
    manifest = archive / f"{month}.json"
    snapshot = work / f"{month}.ocnpj"
    build_report = logs / "build.json"
    disk_log = logs / "disk.tsv"

    if not manifest.exists():
        for path in (work, logs, archive):
            path.mkdir(parents=True, exist_ok=True)

        def sample_disk():
            # Use allocated bytes for the ZIPs, scratch files and snapshot together.
            used = allocated_bytes(sources) + allocated_bytes(work)
            with disk_log.open("a", encoding="utf-8") as stream:
                print(used, shutil.disk_usage(ROOT).free, sep="\t", file=stream)

        opencnpj = BIN / "opencnpj"
        run([opencnpj, "download", "--month", month, "--dest", sources.parent], logs / "download.log", sample_disk)
        # Retain the verified archive only after publishing a complete manifest.
        # Intermediate build files belong to this runner and can be rebuilt from the ZIPs.
        if not snapshot.with_suffix(".ocnpj.sha256").exists():
            scratch = work / "scratch"
            if scratch.exists():
                shutil.rmtree(scratch)
            snapshot.unlink(missing_ok=True)
            run(
                [opencnpj, "build", "--input", sources, "--output", snapshot, "--competence", month,
                 "--profile", "balanced", "--tmp", scratch, "--workers", workers, "--readers", readers,
                 "--report", build_report],
                logs / "build.log", sample_disk,
            )
        verify_log = logs / "verify.log"
        run([opencnpj, "verify", snapshot, "--sources", sources, "--workers", workers], verify_log, sample_disk)
        verified = verify_log.read_text().rsplit("\n$ ", 1)[-1]
        snapshot_sha = re.search(r"sha256 ok: ([a-f0-9]{64})", verified)
        if not snapshot_sha or verified.count("fingerprint MATCH") != 4 or "verify passed in" not in verified:
            raise SystemExit(f"Incomplete source verification. See {verify_log}")
        for path in (aggregate, checksum):
            path.unlink(missing_ok=True)
        aggregate_log = logs / "aggregate.log"
        run(
            [BIN / "opencnpj-intel", "agg", "build", "--snapshot", snapshot,
             "--catalog", CATALOGUE, "--out", aggregate, "--tables", "T1,T2"],
            aggregate_log, sample_disk,
        )
        counts = re.findall(r"^T([12]): \d+ cells, (\d+) counts$", aggregate_log.read_text(), re.MULTILINE)
        totals = {int(table): int(count) for table, count in counts}
        if totals.keys() != {1, 2} or not 0 < totals[1] <= totals[2]:
            raise SystemExit(f"Invalid national totals. See {aggregate_log}")
        samples = [tuple(map(int, row.split())) for row in disk_log.read_text().splitlines()]
        record = {
            "competence": month,
            "source_snapshot_sha256": snapshot_sha[1],
            "municipality_catalogue_sha256": catalogue_sha,
            "source_verification": {"status": "passed", "log": str(verify_log.relative_to(ROOT))},
            "aggregate_sha256": checksum.read_text().split()[0],
            "aggregate_bytes": aggregate.stat().st_size,
            "active_establishments": totals[1],
            "all_establishments": totals[2],
            "sampled_peak_working_bytes": max(used for used, _ in samples),
            "sampled_minimum_volume_free_bytes": min(free for _, free in samples),
        }
        shutil.copyfile(sources / "source-manifest.json", logs / "source-manifest.json")
        staging = manifest.with_suffix(".json.tmp")
        staging.write_text(json.dumps(record, indent=2) + "\n")
        staging.replace(manifest)

    record = checked_record(month, catalogue_sha)
    for path in (sources, work):
        if path.exists():
            shutil.rmtree(path)
    print(json.dumps(record), flush=True)


def summarize(records):
    active = [row["active_establishments"] for row in records]
    lower, upper = min(active), max(active)
    levels = "▁▂▃▄▅▆▇█"
    print(f"National T1 ({lower} to {upper}):")
    print("".join(levels[round((count - lower) * (len(levels) - 1) / max(1, upper - lower))] for count in active))
    print("competence\tactive\tall\tactive_change_pct\tsampled_peak_working_bytes")
    previous = None
    for row, count in zip(records, active, strict=True):
        change = "-" if previous is None else f"{100 * (count - previous) / previous:+.3f}%"
        print(row["competence"], count, row["all_establishments"], change, row["sampled_peak_working_bytes"], sep="\t")
        previous = count
    print("verified_competences", len(records))
    print("sampled_peak_working_bytes", max(row["sampled_peak_working_bytes"] for row in records))
    print("sampled_minimum_volume_free_bytes", min(row["sampled_minimum_volume_free_bytes"] for row in records))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--first", type=competence, default="2024-11")
    parser.add_argument("--last", type=competence, default="2026-10")
    parser.add_argument("--workers", type=int, default=16)
    parser.add_argument("--readers", type=int, default=4)
    parser.add_argument("--summarize", action="store_true", help="validate completed archives and report national totals")
    args = parser.parse_args()
    if args.first > args.last or args.workers < 1 or args.readers < 1:
        parser.error("expected an ordered range and positive worker and reader counts")
    catalogue_sha = digest(CATALOGUE)
    if args.summarize:
        summarize([checked_record(month, catalogue_sha) for month in months(args.first, args.last)])
        return
    ROOT.mkdir(parents=True, exist_ok=True)
    with (ROOT / "backfill.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise SystemExit("A backfill is already running") from None
        for month in months(args.first, args.last):
            backfill(month, args.workers, args.readers, catalogue_sha)


if __name__ == "__main__":
    main()
