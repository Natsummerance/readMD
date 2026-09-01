import base from './vite.config.mjs'

export default {
  ...base,
  build: {
    ...base.build,
    emptyOutDir: false,
    outDir: 'dist',
    rollupOptions: {
      ...base.build.rollupOptions,
      output: { entryFileNames: '[name].cjs', format: 'cjs' }
    }
  }
}
