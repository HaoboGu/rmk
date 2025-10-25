const langBadge = (name: string, icon: string) => () => (
  <div style={{ display: 'flex', alignItems: 'center', gap: '0.3rem' }}>
    <span className={icon} />
    <span>{name}</span>
  </div>
)

// https://icon-sets.iconify.design/material-icon-theme/
export const Rust = langBadge('Rust', 'icon-[material-icon-theme--rust]')
export const Toml = langBadge('Toml', 'icon-[material-icon-theme--toml]')

export default () => null;
