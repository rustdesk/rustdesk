import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
    resolve: {
        alias: {
            './libsodium.mjs': path.resolve(__dirname, 'node_modules/libsodium/dist/modules-esm/libsodium.mjs'),
        },
    },
    build: {
        target: 'esnext',
        manifest: false,
        minify: false,
        sourcemap: true,
        rollupOptions: {
            output: {
                entryFileNames: `[name].js`,
                chunkFileNames: `[name].js`,
                assetFileNames: `[name].[ext]`,
            }
        }
    },
})