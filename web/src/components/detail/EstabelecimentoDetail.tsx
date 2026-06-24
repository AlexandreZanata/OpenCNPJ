import type { EstabelecimentoSearchResult } from '../../api/types'
import { Card } from '../ui/Card'
import { EmpresaFullPanel, SimplesPanel, SocioList } from './EmpresaAggregateCard'
import { EstabelecimentoFullPanel } from './EstabelecimentoFullPanel'
import { RecordActions } from './RecordActions'

interface EstabelecimentoDetailProps {
  data: EstabelecimentoSearchResult
}

export function EstabelecimentoDetail({ data }: EstabelecimentoDetailProps) {
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap justify-end">
        <RecordActions data={data} filename={`estabelecimento-${data.cnpj_completo}`} />
      </div>
      <EstabelecimentoFullPanel data={data} />
      <Card title="Empresa">
        <EmpresaFullPanel data={data.empresa} />
      </Card>
      <Card title={`Sócios (${data.socios.length})`}>
        <SocioList socios={data.socios} />
      </Card>
      <Card title="Simples Nacional / MEI">
        <SimplesPanel data={data.simples} />
      </Card>
    </div>
  )
}
