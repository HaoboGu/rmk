import * as path from 'node:path';
import { defineConfig } from 'rspress/config';

export default defineConfig({
  root: path.join(__dirname, 'docs'),
  title: 'RMK',
  icon: '/rmk_logo.svg',
  logo: {
    light: '/rmk_logo.svg',
    dark: '/rmk_logo.svg',
  },
  outDir: 'dist',
  multiVersion: {
    default: 'main',
    versions: ['main'],
  },
  globalStyles: path.join(__dirname, 'docs/styles/index.css'),
  themeConfig: {
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/haobogu/rmk',
      },
    ],
  },
});
