import config from '../../apps/web/eslint.config.js';
export default [
  ...config,
  { ignores: ['storybook-static'] },
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: { parserOptions: { tsconfigRootDir: import.meta.dirname } },
  },
];
