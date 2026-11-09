#!/usr/bin/env python3
"""Serve the frontend and the OpenCNPJ CLI adapter on loopback or one LAN."""

import argparse
import csv
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from ipaddress import IPv4Address, IPv4Network
import json
from pathlib import Path
import subprocess


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, directory, engine, municipalities, **kwargs):
        self.engine = engine
        self.municipalities = municipalities
        super().__init__(*args, directory=directory, **kwargs)

    def do_GET(self):
        if self.path == "/":
            self.send_response(302)
            self.send_header("Location", "/?local=1")
            self.end_headers()
            return
        super().do_GET()

    def send_json(self, status, body):
        content = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(content)))
        self.end_headers()
        self.wfile.write(content)

    def do_POST(self):
        if self.path != "/api/local/ask":
            self.send_error(404)
            return
        if self.headers.get_content_type() != "application/json":
            self.send_json(415, {"error": "Envie a pergunta em JSON."})
            return
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if not 0 < size <= 8192:
                raise ValueError("request size")
            body = json.loads(self.rfile.read(size))
            question = body.get("question") if isinstance(body, dict) else None
            if not isinstance(question, str) or not 0 < len(question.strip()) <= 1000 or "\0" in question:
                raise ValueError("invalid question")
        except ValueError:
            self.send_json(400, {"error": "Digite uma pergunta de até 1.000 caracteres."})
            return

        try:
            process = subprocess.run(
                [*self.engine, f"--text={question.strip()}"],
                capture_output=True,
                check=True,
                timeout=45,
            )
            reply = json.loads(process.stdout)
        except (OSError, subprocess.SubprocessError, ValueError) as error:
            self.log_error("OpenCNPJ CLI failed: %s", error)
            self.send_json(503, {"error": "O motor local não conseguiu concluir a consulta."})
            return

        execution = reply.get("execution") or {}
        operation = (reply["interpretation"].get("request") or {}).get("op", {})
        if operation.get("dim") == "municipality":
            for row in execution.get("value", {}).get("rows", []):
                row["label"] = self.municipalities.get(row["key"], str(row["key"]))
        self.send_json(200, {"interpretation": reply["interpretation"], "execution": reply.get("execution")})


class LocalSearchServer(ThreadingHTTPServer):
    def __init__(self, address, handler, allowed_network):
        self.allowed_network = allowed_network
        super().__init__(address, handler)

    def verify_request(self, request, client_address):
        return IPv4Address(client_address[0]) in self.allowed_network


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=5173)
    parser.add_argument("--bind", type=IPv4Address, default=IPv4Address("127.0.0.1"))
    parser.add_argument("--allow-network", type=IPv4Network, help="Allowed client subnet for a LAN bind")
    for asset in ("engine", "catalog", "sectors", "agg"):
        parser.add_argument(f"--{asset}", type=Path, required=True)
    parser.add_argument("--history", type=Path)
    args = parser.parse_args()
    if args.bind.is_unspecified or args.bind.is_multicast:
        parser.error("--bind must be a specific unicast IPv4 address")
    if not args.bind.is_loopback and args.allow_network is None:
        parser.error("a LAN bind requires --allow-network")
    allowed_network = args.allow_network or IPv4Network("127.0.0.0/8")
    if args.bind not in allowed_network:
        parser.error("--bind must belong to --allow-network")
    directory = Path(__file__).resolve().parents[1] / "dist"
    for asset in (args.engine, args.catalog, args.sectors, args.agg, directory / "index.html"):
        if not asset.is_file():
            parser.error(f"missing file: {asset}")
    with args.catalog.open(encoding="utf-8", newline="") as stream:
        municipalities = {
            int(row["id"]): f'{row["name"]}/{row["uf"]}'
            for row in csv.DictReader(stream, delimiter="\t")
            if row["uf"]
        }
    engine = [
        str(args.engine.resolve()), "ask",
        "--catalog", str(args.catalog.resolve()),
        "--sectors", str(args.sectors.resolve()),
        "--agg", str(args.agg.resolve()),
    ]
    if args.history:
        engine.extend(["--history", str(args.history.resolve())])
    handler = partial(Handler, directory=directory, engine=engine, municipalities=municipalities)
    with LocalSearchServer((str(args.bind), args.port), handler, allowed_network) as server:
        print(f"http://{args.bind}:{server.server_port}/?local=1#/consulta", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
