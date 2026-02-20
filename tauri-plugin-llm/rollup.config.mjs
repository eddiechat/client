import typescript from '@rollup/plugin-typescript';

export default {
  input: 'guest-js/index.ts',
  output: {
    dir: 'dist',
    format: 'es',
    sourcemap: true,
  },
  plugins: [
    typescript({
      tsconfig: './tsconfig.json',
    }),
  ],
  external: ['@tauri-apps/api/core'],
};
