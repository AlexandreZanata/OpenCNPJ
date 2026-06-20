import type { Estabelecimento } from '../../api/types'
import { Badge } from '../ui/Badge'
import { Card } from '../ui/Card'
import { formatCnpj } from '../../utils/cnpj'
import { unwrapString } from '../../utils/format'

interface EstabelecimentoDetailProps {
  data: Estabelecimento
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs uppercase tracking-wide text-slate-500">{label}</dt>
      <dd className="mt-1 text-sm text-slate-100">{value || '—'}</dd>
    </div>
  )
}

export function EstabelecimentoDetail({ data }: EstabelecimentoDetailProps) {
  const situacao = unwrapString(data.situacao_cadastral)
  const active = situacao === '2' || situacao === '02'

  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <Card title="Estabelecimento">
        <div className="mb-4 flex flex-wrap items-center gap-3">
          <p className="font-mono text-lg text-white">{formatCnpj(data.cnpj_completo)}</p>
          <Badge tone={active ? 'success' : 'warning'}>{active ? 'Active' : situacao || 'Unknown'}</Badge>
        </div>
        <dl className="grid gap-4 sm:grid-cols-2">
          <Field label="Razão Social (Empresa)" value={unwrapString(data.razao_social)} />
          <Field label="Nome Fantasia" value={unwrapString(data.nome_fantasia)} />
          <Field label="CNAE Principal" value={unwrapString(data.cnae_fiscal_principal)} />
          <Field label="CNAE Description" value={unwrapString(data.cnae_descricao)} />
        </dl>
      </Card>
      <Card title="Address & Contact">
        <dl className="grid gap-4 sm:grid-cols-2">
          <Field label="UF" value={unwrapString(data.uf)} />
          <Field label="Município" value={unwrapString(data.municipio_nome) || unwrapString(data.municipio)} />
          <Field label="Logradouro" value={unwrapString(data.logradouro)} />
          <Field label="Número" value={unwrapString(data.numero)} />
          <Field label="Bairro" value={unwrapString(data.bairro)} />
          <Field label="CEP" value={unwrapString(data.cep)} />
          <Field label="Email" value={unwrapString(data.email)} />
          <Field
            label="Telefone"
            value={[unwrapString(data.ddd_1), unwrapString(data.telefone_1)].filter(Boolean).join(' ')}
          />
        </dl>
      </Card>
    </div>
  )
}
