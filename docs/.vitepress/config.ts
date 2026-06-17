import { defineConfig } from 'vitepress'

export default defineConfig({
    title: 'moonup',
    description: 'Manage multiple MoonBit toolchains with ease',
    head: [
        ['link', { rel: 'icon', type: 'image/svg+xml', href: '/logo.svg' }]
    ],
    themeConfig: {
        logo: '/logo.svg',
        siteTitle: 'moonup',
        nav: [
            { text: 'Guide', link: '/guide/' },
            { text: 'Reference', link: '/reference/' },
            { text: 'GitHub', link: 'https://github.com/chawyehsu/moonup' }
        ],
        sidebar: [
            {
                text: 'Guide',
                items: [
                    { text: 'Getting Started', link: '/guide/' },
                    { text: 'Installation', link: '/guide/installation' },
                    { text: 'Usage', link: '/guide/usage' }
                ]
            },
            {
                text: 'Reference',
                items: [
                    { text: 'CLI Commands', link: '/reference/' }
                ]
            }
        ],
        socialLinks: [
            { icon: 'github', link: 'https://github.com/chawyehsu/moonup' }
        ],
        footer: {
            message: 'Released under the Apache-2.0 License.',
            copyright: 'Copyright © 2024-present Chawye Hsu'
        },
        search: {
            provider: 'local'
        }
    }
})
