import { NavLink } from 'react-router-dom'
import { BarChart3, Building2, LayoutDashboard, MapPin, Search } from 'lucide-react'

const links = [
  { to: '/', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/cnpj', label: 'CNPJ Lookup', icon: Search },
  { to: '/empresas', label: 'Empresa Search', icon: Building2 },
  { to: '/estabelecimentos', label: 'Estabelecimento Search', icon: MapPin },
  { to: '/analytics', label: 'Analytics', icon: BarChart3 },
]

export function Sidebar() {
  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-surface px-4 py-6">
      <div className="mb-8 px-2">
        <p className="text-xs uppercase tracking-widest text-brand-500">Receita Federal</p>
        <h1 className="text-xl font-bold text-white">BUSCA CNPJ</h1>
        <p className="mt-1 text-xs text-slate-400">Enterprise data portal</p>
      </div>
      <nav className="flex flex-col gap-1">
        {links.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            end={to === '/'}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition ${
                isActive ? 'bg-brand-600/20 text-brand-100' : 'text-slate-400 hover:bg-slate-800 hover:text-white'
              }`
            }
          >
            <Icon className="h-4 w-4" />
            {label}
          </NavLink>
        ))}
      </nav>
    </aside>
  )
}
