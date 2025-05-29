// https://vitepress.dev/guide/custom-theme
import { h } from 'vue'
import type { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme-without-fonts'
import './my-font.css'
import './style.css'
import './custom.css'
import './custom.scss'
import './custom-block-style.scss'

export default {
  extends: DefaultTheme,
  
  Layout: () => {
    return h(DefaultTheme.Layout, null, {
      // https://vitepress.dev/guide/extending-default-theme#layout-slots
      
    })
  },
  enhanceApp({ app, router, siteData }) {
    // ...
  }
} satisfies Theme
