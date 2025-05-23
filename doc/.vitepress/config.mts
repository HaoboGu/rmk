import { defineConfig } from 'vitepress'
import taskLists from 'markdown-it-task-lists'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: 'RMK',
  description: 'A Rmk Site',
  head: [['link', { rel: 'icon', href: '/images/rmk_logo.svg' }]],
  rewrites: {
    'en/:rest*': ':rest*'
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    logo: '/images/rmk_logo.svg',
    nav: nav(),

    sidebar: {
      '/documentation/': { base: '/documentation/', items: sidebarGuide() }
    },

    socialLinks: [{ icon: 'github', link: 'https://github.com/HaoboGu/rmk' }],
    search: {
      provider: 'local'
    }
  },
  // locales: {
  //   root: { label: 'English' },
  //   zh: { label: '简体中文' },
  // },
  markdown: {
    config: (md) => {
      // 启用任务列表插件
      // const taskLists = require('markdown-it-task-lists')
      md.use(taskLists)
    },
    // 启用代码块自动格式化
    codeTransformers: [
      {
        // 缩进修复
        postprocess: (html) => html.replace(/^(\s*)<pre>/gm, '<pre>')
      }
    ]
  }
})

function nav() {
  return [{ text: 'Docs', link: '/documentation/introduction' }]
}
function sidebarGuide() {
  return [
    {
      items: [{ text: 'Introduction', link: '/introduction' }]
    },
    {
      text: 'User Guide',
      items: [
        { text: '1.Overview', link: 'user_guide/1_guide_overview' },
        {
          text: '2.Create RMK Firmware',
          link: 'user_guide/2_create_firmware',
          collapsed: true,
          items: [
            {
              text: 'Cloud Compilation',
              link: 'user_guide/2-1_cloud_compilation'
            },
            {
              text: 'Local Compilation',
              link: 'user_guide/2-2_local_compilation'
            }
          ]
        },
        { text: '3.Flash the Firmware', link: 'user_guide/3_flash_firmware' },
        { text: 'FAQs', link: 'user_guide/faq' },
        { text: 'Real World Examples', link: 'user_guide/real_world_examples' }
      ]
    },
    {
      text: 'Features',
      items: [
        {
          text: 'Keyboard Configuration',
          link: 'features/keyboard_configuration',
          collapsed: true,
          items: [
            {
              text: 'Keyboard and Matrix',
              link: 'features/configuration/keyboard_matrix'
            },
            { text: 'Layout', link: 'features/configuration/layout' },
            {
              text: 'Split Keyboard',
              link: 'features/configuration/split_keyboard'
            },
            { text: 'Storage', link: 'features/configuration/storage' },
            { text: 'Behavior', link: 'features/configuration/behavior' },
            {
              text: 'Input Device',
              link: 'features/configuration/input_device'
            },
            {
              text: 'Wireless/Bluetooth',
              link: 'features/configuration/wireless'
            },
            { text: 'Light', link: 'features/configuration/light' },
            { text: 'RMK Config', link: 'features/configuration/rmk_config' },
            { text: 'Appendix', link: 'features/configuration/appendix' }
          ]
        },
        {
          text: 'Keymap',
          link: 'features/keymap',
          collapsed: true,
          items: [{ text: 'Special Key', link: 'features/keymap/special_keys' }]
        },
        { text: 'Vial Support', link: 'features/vial_support' },
        { text: 'Wireless', link: 'features/wireless' },
        { text: 'Low-Power', link: 'features/low_power' },
        { text: 'Storage', link: 'features/storage' },
        { text: 'Split Keyboard', link: 'features/split_keyboard' },
        {
          text: 'Binary Size Optimization',
          link: 'features/binary_size_optimization'
        },
        { text: 'Use Rust API', link: 'features/use_rust_api' }
      ]
    },
    {
      text: 'Input Devices',
      items: [
        { text: 'Rotary Encoder', link: 'input_devices/encoder' },
        { text: 'Joystick', link: 'input_devices/joystick' }
      ]
    },
    {
      text: 'Development',
      items: [
        { text: 'Roadmap', link: 'development/roadmap' },
        { text: 'How to Contribute', link: 'development/how_to_contribute' }
      ]
    }
  ]
}
