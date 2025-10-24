import { Icon } from '@iconify/react'

const langBadge = (name: string, icon: string) => () => (
  <div style={{ display: 'flex', alignItems: 'center', gap: '0.3rem' }}>
    <Icon icon={icon} />
    <span>{name}</span>
  </div>
)

// https://icon-sets.iconify.design/material-icon-theme/
export const Rust = langBadge('Rust', 'material-icon-theme:rust')
export const Toml = langBadge('Toml', 'material-icon-theme:toml')
