import { build as bundle } from 'esbuild';
import { build as buildRenderer } from 'vite';
import './build-native';

await bundle({ entryPoints: ['src/electron/main.ts', 'src/electron/preload.ts'], outdir: 'dist/electron',
  outExtension: { '.js': '.cjs' }, bundle: true, platform: 'node', format: 'cjs', external: ['electron'], sourcemap: true });
await buildRenderer({ root: 'src/renderer', base: './', build: { outDir: '../../dist/renderer', emptyOutDir: true }, worker: { format: 'es' } });
