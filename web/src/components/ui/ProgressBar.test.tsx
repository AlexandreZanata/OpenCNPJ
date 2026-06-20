import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ProgressBar } from './ProgressBar'

describe('ProgressBar', () => {
  it('renders indeterminate animation class', () => {
    const html = renderToStaticMarkup(<ProgressBar percent={null} label="Working…" />)
    expect(html).toContain('progress-indeterminate')
    expect(html).toContain('Working…')
  })

  it('renders determinate percent width', () => {
    const html = renderToStaticMarkup(<ProgressBar percent={42} />)
    expect(html).toContain('width:42%')
    expect(html).toContain('42%')
  })
})
