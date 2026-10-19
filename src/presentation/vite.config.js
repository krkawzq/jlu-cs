import { defineConfig } from 'vite';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = path.dirname(fileURLToPath(import.meta.url));
export default defineConfig({
  root,
  base: '/assets/',
  build: {
    outDir: path.resolve(root, '../../build/presentation-web'),
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      input: path.join(root, 'web/editor.js'),
      output: { entryFileNames: 'editor.js', chunkFileNames: 'chunks/[name]-[hash].js', assetFileNames: '[name]-[hash][extname]' }
    },
    manifest: true,
    chunkSizeWarningLimit: 4000
  },
  worker: { format: 'es' }
});
