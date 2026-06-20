import { Search } from 'lucide-react'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Button } from '../ui/Button'
import { Input } from '../ui/Input'
import { formatCnpj, isValidCnpj, normalizeCnpjInput } from '../../utils/cnpj'

export function CnpjSearchBar() {
  const navigate = useNavigate()
  const [value, setValue] = useState('')
  const [error, setError] = useState('')

  const submit = () => {
    const digits = normalizeCnpjInput(value)
    if (!isValidCnpj(digits)) {
      setError('Enter a valid 14-digit CNPJ.')
      return
    }
    setError('')
    navigate(`/cnpj/${digits}`)
  }

  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
      <div className="flex-1">
        <Input
          label="CNPJ (Estabelecimento)"
          placeholder="00.000.000/0000-00"
          value={value}
          onChange={(event) => {
            const digits = normalizeCnpjInput(event.target.value)
            setValue(formatCnpj(digits))
          }}
          onKeyDown={(event) => event.key === 'Enter' && submit()}
        />
        {error && <p className="mt-1 text-sm text-red-400">{error}</p>}
      </div>
      <Button onClick={submit}>
        <Search className="h-4 w-4" /> Lookup
      </Button>
    </div>
  )
}
