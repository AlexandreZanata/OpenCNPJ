import json, re, statistics, sys, time, urllib.request
base = sys.argv[1]
src = open(sys.argv[2], encoding="utf-8").read()
questions = re.findall(r'^\s+"([^"]+)",?$', src, re.M)
def post(path, body):
    req = urllib.request.Request(base + path, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    started = time.perf_counter()
    with urllib.request.urlopen(req) as r:
        data = r.read()
    return data, (time.perf_counter() - started) * 1000
context = None
print(f"{'decision':18s} {'act':12s} {'wall_ms':>8s} {'engine_ms':>9s} {'interp_us':>9s}  question")
for q in questions:
    # Conversation examples continue the previous turn; the rest start a new conversation.
    follow = q.startswith(("E ", "Agora", "Me mande isso", "Só o"))
    body = {"question": q} | ({"context": context} if follow and context else {})
    runs = []
    for _ in range(3):
        data, wall = post("/api/local/ask", body)
        reply = json.loads(data)
        runs.append((wall, reply["timing_us"].get("total", 0) / 1000, reply["timing_us"].get("interpret", 0)))
    context = reply["context"]
    wall, engine, interp = sorted(runs)[1]
    print(f"{reply['answer']['decision']:18s} {reply['interpretation']['act']:12s} {wall:8.1f} {engine:9.1f} {interp:9d}  {q}")
    if reply.get("export"):
        offer = reply["export"]
        data, wall = post("/api/local/export", {"request": offer["request"], "export": offer["export"], "question": q})
        print(f"{'  file':18s} {offer['format']:12s} {wall:8.1f} {'':9s} {'':9s}  {len(data)} bytes, {offer['rows']} rows -> {offer['filename']}")
