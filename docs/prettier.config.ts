import type { Config } from 'prettier'

const config: Config = {
  printWidth: 100,
  semi: false,
  singleQuote: true,
  trailingComma: 'none',
  tabWidth: 2,
  proseWrap: 'always',
  overrides: [
    {
      files: '*.md',
      options: {
        proseWrap: 'preserve'
      }
    }
  ],
}

export default config
