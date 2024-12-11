import { defineUserConfig, PageHeader } from 'vuepress'
import { viteBundler } from '@vuepress/bundler-vite'
import { defaultTheme } from '@vuepress/theme-default'
import { path } from '@vuepress/utils'

import { googleAnalyticsPlugin } from '@vuepress/plugin-google-analytics'
import { registerComponentsPlugin } from '@vuepress/plugin-register-components'

function htmlDecode(input: string): string {
  return input.replace("&#39;", "'").replace("&amp;", "&").replace("&quot;", '"')
}

function fixPageHeader(header: PageHeader) {
  header.title = htmlDecode(header.title)
  header.children.forEach(child => fixPageHeader(child))
}

export default defineUserConfig({
  lang: 'en-GB',
  title: 'GitHub Backup',
  description: "Automatically backup your GitHub repositories.",

  head: [
    ['meta', { name: "description", content: "Automatically backup your GitHub repositories and releases, just in case." }],
    ['link', { rel: 'icon', href: '/favicon.ico' }]
  ],

  bundler: viteBundler(),

  extendsPage(page, app) {
    const fixedHeaders = page.headers || []
    fixedHeaders.forEach(header => fixPageHeader(header))

    page.headers = fixedHeaders;
  },

  theme: defaultTheme({
    logo: 'https://cdn.sierrasoftworks.com/logos/icon.png',
    logoDark: 'https://cdn.sierrasoftworks.com/logos/icon_light.png',

    repo: "SierraSoftworks/github-backup",
    docsDir: 'docs',
    navbar: [
      {
        text: "Getting Started",
        link: "/guide/README.md",
      },
      {
        text: "Advanced",
        children: [
          '/advanced/filters.md',
          '/advanced/query-params.md',
          '/advanced/refspecs.md'
        ]
      },
      {
        text: "Reference",
        children: [
          '/reference/repo.md',
          '/reference/release.md'
        ]
      },
      {
        text: "Report an Issue",
        link: "https://github.com/SierraSoftworks/github-backup/issues/new",
        target: "_blank"
      }
    ],

    sidebar: {
      '/': [
        {
          text: "Getting Started",
          children: [
            '/guide/README.md',
            '/guide/enterprise.md',
            '/guide/telemetry.md'
          ]
        },
        {
          text: "Reference",
          children: [
            '/reference/repo.md',
            '/reference/release.md'
          ]
        },
        {
          text: "Advanced",
          children: [
            '/advanced/filters.md',
            '/advanced/query-params.md',
            '/advanced/refspecs.md'
          ]
        }
      ],
    }
  }),

  plugins: [
    googleAnalyticsPlugin({ id: "G-R57T3LCFD4" }),
    registerComponentsPlugin({
      componentsDir: path.resolve(__dirname, './components'),
    })
  ]
})
