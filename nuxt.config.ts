import tailwindcss from '@tailwindcss/vite'

// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2026-09-09',
  devtools: { enabled: true },
  ssr: false,
  srcDir: 'src/',
  alias: {
    '@bindings': './src/types/bindings',
  },
  modules: [
    '@nuxtjs/i18n',
    '@nuxt/icon',
    '@pinia/nuxt',
    '@vueuse/nuxt',
  ],
  components: [
    { path: '~/components', pathPrefix: true },
  ],
  css: [
    '~/assets/css/tailwind.css',
  ],
  vite: {
    plugins: [tailwindcss()],
    clearScreen: false,
    server: {
      // 3030 statt Nuxt-Default 3000, das auf dem Dev-System oft belegt ist.
      // Muss mit tauri.conf.json → build.devUrl und security.devCsp
      // synchron bleiben.
      port: 3030,
      strictPort: true,
      hmr: process.env.TAURI_ENV_HOST
        ? {
            protocol: 'ws',
            host: process.env.TAURI_ENV_HOST,
            port: 1421,
          }
        : undefined,
      watch: {
        ignored: ['**/src-tauri/**'],
      },
    },
  },
  i18n: {
    restructureDir: 'src/i18n',
    defaultLocale: 'de',
    locales: [
      { code: 'de', file: 'de.json' },
      { code: 'en', file: 'en.json' },
    ],
    strategy: 'no_prefix',
    detectBrowserLanguage: false,
  },
  icon: {
    mode: 'svg',
    clientBundle: {
      scan: true,
      includeCustomCollections: true,
    },
    serverBundle: false,
  },
  typescript: {
    strict: true,
  },
})
