import * as path from 'node:path'
import { defineConfig } from '@rspress/core'
import { pluginSitemap } from "@rspress/plugin-sitemap"
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss'

import versions from './versions.json' with { type: 'json' }

export default defineConfig({
  root: path.join(__dirname, 'docs'),
  title: 'RMK',
  icon: '/favicon.ico',
  logo: {
    light: '/logo.svg',
    dark: '/logo.svg'
  },
  outDir: 'dist',
  plugins: [
    pluginSitemap({
      siteUrl: "https://rmk.rs"
    }),
  ],
  builderConfig: {
    plugins: [pluginTailwindcss()],
  },
  multiVersion: {
    default: versions.map((branch) => branch.split('/').pop()!)[0],
    versions: ['main', ...versions.map((branch) => branch.split('/').pop()!)]
  },
  search: {
    versioned: true
  },
  globalStyles: path.join(__dirname, 'docs/styles/index.css'),
  themeConfig: {
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/haobogu/rmk'
      },
      {
        icon: 'discord',
        mode: 'link',
        content: 'https://discord.gg/HHGA7pQxkG'
      }
    ]
  }
})
