import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
import { viteStaticCopy } from 'vite-plugin-static-copy';

export default {
    build: {
        lib: {
            entry: 'src/index.js',
            name: 'DracoDecoderJS',
            fileName: (format) => `index.${format}.js`,
            formats: ['es', 'umd'],
        },
        rollupOptions: {
            external: [],
            output: {
                assetFileNames: '[name][extname]', // 保持 wasm 文件名
            },
        },
        minify: 'terser',
        terserOptions: {
            compress: {
                drop_console: true,
                drop_debugger: true
            },
            mangle: true,
        },
    },
    plugins: [
        wasm(),
        topLevelAwait(),
        viteStaticCopy({
            targets: [
                {
                    src: 'node_modules/draco3d/*.wasm',
                    dest: 'draco3d'
                }
            ]
        }),
    ],
};