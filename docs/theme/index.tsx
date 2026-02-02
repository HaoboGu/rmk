import branchs from '../versions.json' assert { type: 'json' }

const NotFoundLayout = () => {
  if (typeof window === 'undefined') return
  const versions = [...branchs.map((b) => b.split('/').pop()!), 'main']
  const version = window.location.pathname.split('/')[1]
  window.location.href = versions.includes(version) ? `/${version}` : '/'
}

export {
  NotFoundLayout
}

export * from '@rspress/core/theme-original'
export * from './components/LangBadge'
export * from './components/LinkCard'
