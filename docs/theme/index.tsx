import { Layout } from '@rspress/core/theme-original'

import branchs from '../versions.json' with { type: 'json' }

const NotFoundLayout = () => {
  if (typeof window === 'undefined') return
  const versions = [...branchs.map((b) => b.split('/').pop()!), 'main']
  const version = window.location.pathname.split('/')[1]
  window.location.href = versions.includes(version) ? `/${version}` : '/'
}

export { Layout, NotFoundLayout }
export * from '@rspress/core/theme-original'
export * from './components/LangBadge'
export * from './components/LinkCard'
