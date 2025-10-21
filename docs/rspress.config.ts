import * as path from 'node:path'
import { defineConfig } from 'rspress/config'

import versions from './versions.json' assert { type: 'json' }

export default defineConfig({
  root: path.join(__dirname, 'docs'),
  title: 'RMK',
  icon: '/rmk_logo.svg',
  logo: {
    light: '/rmk_logo.svg',
    dark: '/rmk_logo.svg'
  },
  outDir: 'dist',
  multiVersion: {
    default: versions.map((branch) => branch.split('/').pop()!)[0],
    versions: ['main', ...versions.map((branch) => branch.split('/').pop()!)]
  },
  search: {
    versioned: true,
  },
  globalStyles: path.join(__dirname, 'docs/styles/index.css'),
  themeConfig: {
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/haobogu/rmk'
      }
    ]
  }
})
