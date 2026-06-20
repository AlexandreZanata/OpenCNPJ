export function digitsOnly(value: string): string {
  return value.replace(/\D/g, '')
}

export function formatCnpj(value: string): string {
  const digits = digitsOnly(value)
  if (digits.length !== 14) {
    return value
  }
  return digits.replace(/^(\d{2})(\d{3})(\d{3})(\d{4})(\d{2})$/, '$1.$2.$3/$4-$5')
}

export function isValidCnpj(value: string): boolean {
  const digits = digitsOnly(value)
  if (digits.length !== 14 || /^(\d)\1+$/.test(digits)) {
    return false
  }

  const calc = (slice: string, factors: number[]): number => {
    const sum = factors.reduce((acc, factor, index) => acc + Number(slice[index]) * factor, 0)
    const mod = sum % 11
    return mod < 2 ? 0 : 11 - mod
  }

  const base = digits.slice(0, 12)
  const d1 = calc(base, [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2])
  const d2 = calc(base + d1, [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2])
  return digits.endsWith(`${d1}${d2}`)
}

export function normalizeCnpjInput(value: string): string {
  return digitsOnly(value).slice(0, 14)
}
