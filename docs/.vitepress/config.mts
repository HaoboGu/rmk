import { defineConfig } from 'vitepress'
import taskLists from 'markdown-it-task-lists'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: ' ',
  description: 'RMK keyboard firmware',
  head: [['link', { rel: 'icon', href: '/images/rmk_logo.svg' }]],
  rewrites: {
    'en/:rest*': ':rest*'
  },
  base: '/',
  themeConfig: {
    logo: '/images/rmk_logo.svg',
    nav: nav(),
    outline: {
      level: 'deep'
    },
    sidebar: {
      '/docs/': { base: '/docs/', items: sidebarGuide() }
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/HaoboGu/rmk' },
      { icon: 'discord', link: 'https://discord.gg/HHGA7pQxkG' }
    ],
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
  },
  vite: {
    css: { 
      preprocessorOptions: {
        scss: {
          api: 'modern',
        }
      }
    }
  },

  transformHead: ({ assets }) => {
    const textFont = assets.find((file) => /OpenSans\.[\w-]+\.ttf/.test(file))
    const codeFont = assets.find((file) => /FiraCode\.[\w-]+\.ttf/.test(file))
    if (textFont && codeFont) {
      return [
        [
          'link',
          {
            rel: 'preload',
            href: textFont,
            as: 'font',
            type: 'font/ttf',
            crossorigin: ''
          }
        ],
        [
          'link',
          {
            rel: 'preload',
            href: codeFont,
            as: 'font',
            type: 'font/ttf',
            crossorigin: ''
          }
        ]
      ]
    }
  }
})

function nav() {
  return [
    { text: 'Guide', link: '/docs/user_guide/1_guide_overview' },
    { text: 'Documentation', link: '/docs/introduction' },
    {
      text: `v0.7.7`,
      items: [
        {
          items: [{ text: 'v0.7.7', link: '/docs/introduction' }]
        },
        {
          items: [{ text: 'Migration Guide', link: '/docs/migration_guide' }]
        },
        {
          items: [{ text: 'v0.6.1', link: 'https://haobogu.github.io/rmk' }]
        }
      ]
    },
    { text: 'API', link: 'https://docs.rs/rmk/latest/rmk/' }
  ]
}
function sidebarGuide() {
  return [
    {
      text: 'Introduction',
      items: [{ text: 'RMK Introduction', link: 'introduction' }]
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
      text: 'Migration Guide',
      items: [{ text: 'From v0.6 to v0.7', link: 'migration_guide' }]
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
          items: [
            { text: 'Keycodes', link: 'features/keymap/keycodes' },
            { text: 'Special Key', link: 'features/keymap/special_keys' },
            { text: 'Keyboard Macros', link: 'features/keymap/keyboard_macros' },
            {
              text: 'Special Characters and Unicode',
              link: 'features/keymap/special_characters_and_unicode'
            }
          ]
        },
        { text: 'Vial Support', link: 'features/vial_support' },
        { text: 'Wireless', link: 'features/wireless' },
        { text: 'Low-Power', link: 'features/low_power' },
        { text: 'Storage', link: 'features/storage' },
        { text: 'Split Keyboard', link: 'features/split_keyboard' },
        { text: 'USB Logging', link: 'features/usb_logging' },
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
