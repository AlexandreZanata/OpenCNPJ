import type { EmpresaAggregate, EmpresaFull, EstabelecimentoFull, Simples, Socio } from '../../api/types'
import { Card } from '../ui/Card'
import { EntityFieldGrid, type FieldDef } from './EntityFieldGrid'
import { EstabelecimentoFullPanel } from './EstabelecimentoFullPanel'
import { RecordActions } from './RecordActions'

const empresaFields: FieldDef[] = [
  { key: 'cnpj_basico', label: 'CNPJ Básico' },
  { key: 'razao_social', label: 'Razão Social' },
  { key: 'natureza_juridica', label: 'Natureza Jurídica (code)' },
  { key: 'natureza_descricao', label: 'Natureza Jurídica' },
  { key: 'qualificacao_responsavel', label: 'Qualificação Responsável (code)' },
  { key: 'qualificacao_descricao', label: 'Qualificação Responsável' },
  { key: 'capital_social', label: 'Capital Social', format: 'currency' },
  { key: 'porte_empresa', label: 'Porte' },
  { key: 'ente_federativo_responsavel', label: 'Ente Federativo Responsável' },
  { key: 'uuid_id', label: 'UUID' },
  { key: 'created_at', label: 'Created At', format: 'datetime' },
  { key: 'updated_at', label: 'Updated At', format: 'datetime' },
]

const socioFields: FieldDef[] = [
  { key: 'nome_socio', label: 'Nome' },
  { key: 'identificador_socio', label: 'Identificador' },
  { key: 'cpf_cnpj_socio', label: 'CPF/CNPJ' },
  { key: 'qualificacao_socio', label: 'Qualificação' },
  { key: 'data_entrada_sociedade', label: 'Data Entrada', format: 'date' },
  { key: 'pais', label: 'País' },
  { key: 'representante_legal', label: 'Representante Legal' },
  { key: 'nome_representante', label: 'Nome Representante' },
  { key: 'qualificacao_representante', label: 'Qualificação Representante' },
  { key: 'faixa_etaria', label: 'Faixa Etária' },
]

const simplesFields: FieldDef[] = [
  { key: 'opcao_simples', label: 'Opção Simples' },
  { key: 'data_opcao_simples', label: 'Data Opção Simples', format: 'date' },
  { key: 'data_exclusao_simples', label: 'Data Exclusão Simples', format: 'date' },
  { key: 'opcao_mei', label: 'Opção MEI' },
  { key: 'data_opcao_mei', label: 'Data Opção MEI', format: 'date' },
  { key: 'data_exclusao_mei', label: 'Data Exclusão MEI', format: 'date' },
]

export function EmpresaAggregateCard({ data }: { data: EmpresaAggregate }) {
  return (
    <div className="space-y-4">
      <Card
        title={data.razao_social}
        action={<RecordActions data={data} filename={`empresa-${data.cnpj_basico}`} />}
      >
        <EntityFieldGrid fields={empresaFields} data={data as unknown as Record<string, unknown>} />
      </Card>
      {data.simples && (
        <Card title="Simples Nacional / MEI">
          <EntityFieldGrid fields={simplesFields} data={data.simples as unknown as Record<string, unknown>} />
        </Card>
      )}
      <Card title={`Sócios (${data.socios.length})`}>
        {data.socios.length === 0 ? (
          <p className="text-sm text-slate-500">No partners on file.</p>
        ) : (
          <div className="space-y-4">
            {data.socios.map((socio, index) => (
              <div key={`${socio.nome_socio}-${index}`} className="rounded-lg border border-border/60 p-3">
                <EntityFieldGrid fields={socioFields} data={socio as unknown as Record<string, unknown>} />
              </div>
            ))}
          </div>
        )}
      </Card>
      <Card title={`Estabelecimentos (${data.estabelecimentos.length})`}>
        {data.estabelecimentos.length === 0 ? (
          <p className="text-sm text-slate-500">No branches on file.</p>
        ) : (
          <div className="space-y-6">
            {data.estabelecimentos.map((est) => (
              <EstabelecimentoFullPanel key={est.cnpj_completo} data={est} compact />
            ))}
          </div>
        )}
      </Card>
    </div>
  )
}

export function EmpresaFullPanel({ data }: { data: EmpresaFull }) {
  return (
    <EntityFieldGrid fields={empresaFields} data={data as unknown as Record<string, unknown>} />
  )
}

export function SocioList({ socios }: { socios: Socio[] }) {
  if (socios.length === 0) {
    return <p className="text-sm text-slate-500">No partners on file.</p>
  }
  return (
    <div className="space-y-4">
      {socios.map((socio, index) => (
        <div key={`${socio.nome_socio}-${index}`} className="rounded-lg border border-border/60 p-3">
          <EntityFieldGrid fields={socioFields} data={socio as unknown as Record<string, unknown>} />
        </div>
      ))}
    </div>
  )
}

export function SimplesPanel({ data }: { data?: Simples | null }) {
  if (!data) {
    return <p className="text-sm text-slate-500">No Simples/MEI record.</p>
  }
  return <EntityFieldGrid fields={simplesFields} data={data as unknown as Record<string, unknown>} />
}
