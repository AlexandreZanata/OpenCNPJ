import type { EstabelecimentoFull } from '../../api/types'
import { formatCnpj } from '../../utils/cnpj'
import { Badge } from '../ui/Badge'
import { Card } from '../ui/Card'
import { EntityFieldGrid, type FieldDef } from './EntityFieldGrid'
import { unwrapString } from '../../utils/format'

const estabFields: FieldDef[] = [
  { key: 'cnpj_completo', label: 'CNPJ Completo', render: (v) => formatCnpj(String(v ?? '')) },
  { key: 'cnpj_basico', label: 'CNPJ Básico' },
  { key: 'cnpj_ordem', label: 'Ordem' },
  { key: 'cnpj_dv', label: 'DV' },
  { key: 'identificador_matriz_filial', label: 'Matriz/Filial' },
  { key: 'nome_fantasia', label: 'Nome Fantasia' },
  { key: 'razao_social', label: 'Razão Social' },
  { key: 'capital_social', label: 'Capital Social', format: 'currency' },
  { key: 'situacao_cadastral', label: 'Situação Cadastral' },
  { key: 'data_situacao_cadastral', label: 'Data Situação', format: 'date' },
  { key: 'motivo_situacao_cadastral', label: 'Motivo (code)' },
  { key: 'motivo_descricao', label: 'Motivo' },
  { key: 'data_inicio_atividade', label: 'Início Atividade', format: 'date' },
  { key: 'cnae_fiscal_principal', label: 'CNAE Principal' },
  { key: 'cnae_descricao', label: 'CNAE Descrição' },
  { key: 'cnae_fiscal_secundaria', label: 'CNAEs Secundários' },
  { key: 'tipo_logradouro', label: 'Tipo Logradouro' },
  { key: 'logradouro', label: 'Logradouro' },
  { key: 'numero', label: 'Número' },
  { key: 'complemento', label: 'Complemento' },
  { key: 'bairro', label: 'Bairro' },
  { key: 'cep', label: 'CEP' },
  { key: 'uf', label: 'UF' },
  { key: 'municipio', label: 'Município (code)' },
  { key: 'municipio_nome', label: 'Município' },
  { key: 'pais', label: 'País (code)' },
  { key: 'pais_descricao', label: 'País' },
  { key: 'nome_cidade_exterior', label: 'Cidade Exterior' },
  { key: 'ddd_1', label: 'DDD 1' },
  { key: 'telefone_1', label: 'Telefone 1' },
  { key: 'ddd_2', label: 'DDD 2' },
  { key: 'telefone_2', label: 'Telefone 2' },
  { key: 'ddd_fax', label: 'DDD Fax' },
  { key: 'fax', label: 'Fax' },
  { key: 'email', label: 'Email' },
  { key: 'situacao_especial', label: 'Situação Especial' },
  { key: 'data_situacao_especial', label: 'Data Situação Especial', format: 'date' },
  { key: 'uuid_id', label: 'UUID' },
  { key: 'created_at', label: 'Created At', format: 'datetime' },
]

interface EstabelecimentoFullPanelProps {
  data: EstabelecimentoFull
  compact?: boolean
}

export function EstabelecimentoFullPanel({ data, compact }: EstabelecimentoFullPanelProps) {
  const situacao = unwrapString(data.situacao_cadastral)
  const active = situacao === '2' || situacao === '02'
  const title = compact
    ? formatCnpj(data.cnpj_completo)
    : `Estabelecimento ${formatCnpj(data.cnpj_completo)}`

  return (
    <Card title={title}>
      <div className="mb-3">
        <Badge tone={active ? 'success' : 'warning'}>{active ? 'Active' : situacao || 'Unknown'}</Badge>
      </div>
      <EntityFieldGrid fields={estabFields} data={data as unknown as Record<string, unknown>} />
    </Card>
  )
}
