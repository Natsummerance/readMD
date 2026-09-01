import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../..')
const upstream = path.join(root, 'third_party/hermes-agent-pet/apps/desktop/src')
const host = path.join(here, 'src/host')
const resolve = (...segments) => path.join(...segments)
// CI/packaging installs dependencies locally.  The optional environment value
// only lets maintainers verify this bridge against an existing Hermes checkout
// without copying a second node_modules tree into ReadMD.
const modules = process.env.READMD_PET_NODE_MODULES || path.join(here, 'node_modules')

export default {
  base: './',
  cacheDir: path.join(here, '.vite-cache'),
  root: here,
  resolve: {
    alias: [
      { find: /^react$/, replacement: resolve(modules, 'react/index.js') },
      { find: /^react\/jsx-runtime$/, replacement: resolve(modules, 'react/jsx-runtime.js') },
      { find: /^react\/jsx-dev-runtime$/, replacement: resolve(modules, 'react/jsx-dev-runtime.js') },
      { find: /^react-dom\/client$/, replacement: resolve(modules, 'react-dom/client.js') },
      { find: /^nanostores$/, replacement: resolve(modules, 'nanostores/index.js') },
      { find: /^@nanostores\/react$/, replacement: resolve(modules, '@nanostores/react/index.js') },
      { find: '@/components/pet/pet-bubble', replacement: resolve(upstream, 'components/pet/pet-bubble.tsx') },
      { find: '@/components/pet/pet-sprite', replacement: resolve(upstream, 'components/pet/pet-sprite.tsx') },
      { find: '@/components/pet/use-pet-zoom-gesture', replacement: resolve(upstream, 'components/pet/use-pet-zoom-gesture.ts') },
      { find: '@/store/pet-overlay', replacement: resolve(upstream, 'store/pet-overlay.ts') },
      { find: '@/store/pet', replacement: resolve(upstream, 'store/pet.ts') },
      { find: '@/components/chat/vibe-hearts', replacement: resolve(host, 'components/chat/vibe-hearts.tsx') },
      { find: '@/components/error-boundary', replacement: resolve(host, 'components/error-boundary.tsx') },
      { find: '@/themes/context', replacement: resolve(host, 'themes/context.tsx') },
      { find: '@/store/session', replacement: resolve(host, 'store/session.ts') },
      { find: '@/store/profile', replacement: resolve(host, 'store/profile.ts') },
      { find: '@/store/pet-gallery', replacement: resolve(host, 'store/pet-gallery.ts') },
      { find: '@/lib/storage', replacement: resolve(host, 'lib/storage.ts') },
      { find: '@/lib/renderer-loop-pause', replacement: resolve(host, 'lib/renderer-loop-pause.ts') },
      { find: '@/lib/icons', replacement: resolve(host, 'lib/icons.tsx') }
    ]
  },
  build: {
    outDir: path.join(here, 'dist/renderer'),
    emptyOutDir: true,
    rollupOptions: { external: ['electron'] }
  }
}
