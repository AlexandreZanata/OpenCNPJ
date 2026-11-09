"""Ask the questions of this round on a running service and print what came back."""
import json, sys, urllib.request
base = sys.argv[1]
QUESTIONS = [
    "Lista de empresas agricolas em sorriso mt, e um exel com todas",
    "quero a lista de empresas agricolas de sorriso mt",
    "quantas fazendas ativas existem em Sorriso/MT?",
    "quero a lista de empresas xyzolicas de sorriso mt",
    "encontre a empresa zaflas",
    "zaflas",
    "qual o CNPJ da MITEL TELEINFORMATICA?",
    "dados do CNPJ 00.074.676/0001-20",
    "encontre o dono da zaflas",
    "me mande apenas o nome e o telefone em um excel das empresas de TI que tiveram baixa em Mato Grosso",
]
def post(path, body):
    request = urllib.request.Request(base + path, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request) as response:
        return json.loads(response.read())
for question in QUESTIONS:
    reply = post("/api/local/ask", {"question": question})
    answer = reply["answer"]
    lead = next((b.get("text") or b.get("question") or b.get("reason") for b in answer["blocks"] if b["type"] in ("text", "clarify", "reject")), "")
    print(f"Q: {question}\n   {answer['decision']} | {lead}")
    records = next((b for b in answer["blocks"] if b["type"] == "records"), None)
    if records:
        col = records["columns"].index("Atividade principal") if "Atividade principal" in records["columns"] else None
        if col is not None:
            activities = sorted({row[col] for row in records["rows"]})
            print(f"   activities on page: {activities[:6]}{' …' if len(activities) > 6 else ''}")
    company = next((b for b in answer["blocks"] if b["type"] == "company"), None)
    if company:
        print("   company:", dict(company["fields"][:4]))
    extras = [e["title"] for e in reply.get("extras", [])]
    if extras:
        print("   extras:", extras)
    if reply.get("export"):
        e = reply["export"]
        print(f"   file: {e['filename']} rows={e['rows']} columns={e['columns']} notes={e['notes']}")
    if answer.get("suggestions"):
        print("   suggestions:", answer["suggestions"][:3])
    print(f"   {reply['timing_us'].get('total', 0) / 1000:.1f} ms")
