import { defineConfig } from "vitepress";
const nav = () => [{ text: "Docs", link: "/documentation/introduction" }];
const sidebarGuide = () => [
  {
    items: [
      { text: "Introduction", link: "/introduction" },
    ],
  },
  {
    text: "User Guide",
    items: [
      { text: "Overview", link: "user_guide/1_guide_overview" },
      { text: "CreateFirmware", link: "user_guide/2_create_firmware" },
    ],
  },
];
// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "RMK",
  description: "A Rmk Site",
  rewrites: {
    "en/:rest*": ":rest*",
  },
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    logo: "/images/rmk_logo.svg",
    nav: nav(),

    sidebar: {
      "/documentation/": { base: "/documentation/", items: sidebarGuide() },
    },

    socialLinks: [
      { icon: "github", link: "https://github.com/vuejs/vitepress" },
    ],
  },
  // locales: {
  //   root: { label: 'English' },
  //   zh: { label: '简体中文' },
  // },
});
