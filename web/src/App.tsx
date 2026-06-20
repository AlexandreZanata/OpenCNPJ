import { Navigate, Route, Routes } from 'react-router-dom'
import { AppShell } from './components/layout/AppShell'
import { AnalyticsPage } from './pages/AnalyticsPage'
import { CnpjLookupPage } from './pages/CnpjLookupPage'
import { DashboardPage } from './pages/DashboardPage'
import { EmpresaSearchPage } from './pages/EmpresaSearchPage'
import { EstabelecimentoSearchPage } from './pages/EstabelecimentoSearchPage'

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<DashboardPage />} />
        <Route path="cnpj" element={<CnpjLookupPage />} />
        <Route path="cnpj/:cnpj" element={<CnpjLookupPage />} />
        <Route path="empresas" element={<EmpresaSearchPage />} />
        <Route path="estabelecimentos" element={<EstabelecimentoSearchPage />} />
        <Route path="analytics" element={<AnalyticsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}
