import { useDark } from '@rspress/core/runtime'

const langBadge = (name: string, defaultIcon: string, darkIcon?: string) => () => {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '0.3rem' }}>
      <span className={useDark() && darkIcon ? darkIcon : defaultIcon} />
      <span>{name}</span>
    </div>
  )
}

// https://icon-sets.iconify.design/material-icon-theme/
export const Rust = langBadge('Rust', 'icon-[material-icon-theme--rust]')
export const Toml = langBadge(
  'Toml',
  'icon-[material-icon-theme--toml-light]',
  'icon-[material-icon-theme--toml]'
)

export default () => null
