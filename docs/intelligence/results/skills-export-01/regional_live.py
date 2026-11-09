"""Ask free-form regional file requests on a running service and check every file."""
import csv, io, json, sys, urllib.request, zipfile
base = sys.argv[1]
QUESTIONS = [
    "Bah, tchê, me passa uma planilha com as farmácias ativas de Porto Alegre/RS",
    "Barbaridade, guri, me alcança um CSV dos restaurantes que fecharam em 2025 em Caxias do Sul/RS, valeu",
    "Oxe, gera um Excel aí das padarias ativas de Recife/PE, visse?",
    "Oxente, me manda uma tabela com nome e telefone das academias de Salvador/BA",
    "Ô meu rei, me envia o CNPJ e a razão social das pet shops abertas em 2025 em Fortaleza/CE num txt",
    "Uai sô, cê consegue me mandar um ecxel com as padarias de Belo Horizonte/MG?",
    "Uai, me arruma um relatório das farmácias que fecharam em 2024 em Uberlândia/MG",
    "Vôte, me manda aí uma planilhinha com as empresas de TI ativas de Cuiabá/MT",
    "Tchá por Deus, gera um CSV com todos os dados dos restaurantes de Sorriso/MT",
    "Égua, maninho, me manda um arquivo das padarias ativas de Belém/PA",
    "Mano, joga num excel as academias de São Paulo/SP que abriram em 2025",
    "Mermão, manda aí em csv as farmácias de Niterói/RJ, por favor",
    "Fala, parceiro, bota numa planilha o nome e o endereço completo das padarias de Goiânia/GO",
    "Tem como me passar uma planilha com as empresas que tiveram baixa em Mato Grosso no setor de TI?",
    "dá pra puxar uma planilha dos supermercados ativos de Campo Grande/MS rapidinho?",
    "Me manda um PDF das padarias de Sinop/MT",
]
def post(path, body):
    request = urllib.request.Request(base + path, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request) as response:
        return response.read(), dict(response.headers)
for question in QUESTIONS:
    reply = json.loads(post("/api/local/ask", {"question": question})[0])
    answer, offer = reply["answer"], reply["export"]
    lead = next((b.get("text") or b.get("question") or b.get("reason") for b in answer["blocks"] if b["type"] in ("text", "clarify", "reject")), "")
    print(f"Q: {question}\n   {answer['decision']} | {lead}")
    if not offer:
        continue
    data, headers = post("/api/local/export", {"request": offer["request"], "export": offer["export"], "question": question})
    if offer["format"] == "xlsx":
        sheet = zipfile.ZipFile(io.BytesIO(data)).read("xl/worksheets/sheet1.xml").decode()
        rows = sheet.count("<row ") - 1
    elif offer["format"] == "csv":
        rows = len(list(csv.reader(io.StringIO(data.decode("utf-8-sig")), delimiter=";"))) - 1
    else:
        rows = int(headers.get("x-opencnpj-rows", headers.get("X-Opencnpj-Rows", -1)))
    status = "OK" if rows == offer["rows"] else "MISMATCH"
    print(f"   file {offer['format']} {offer['filename']} rows={rows} offered={offer['rows']} columns={len(offer['columns'])} {status}")
