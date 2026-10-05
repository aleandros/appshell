import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkCatalogs, checkLocaleConfig, translationKeys } from './i18n.mjs';

const root = fileURLToPath(new URL('../apps/web/src/', import.meta.url));
const config = JSON.parse(fs.readFileSync(path.join(root, 'config/i18n.json'), 'utf8'));
const errors = checkLocaleConfig(config);
if (!errors.length && (config.checkMissingTranslations || process.argv.includes('--strict'))) {
  const read = (tag) =>
    JSON.parse(fs.readFileSync(path.join(root, `config/locales/${tag}.json`), 'utf8'));
  const catalogs = {};
  for (const tag of new Set(['en', ...config.languages.map(({ tag }) => tag)])) {
    try {
      catalogs[tag] = read(tag);
    } catch (error) {
      errors.push(`${tag}: ${error.message}`);
    }
  }
  function keys(directory) {
    return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
      const file = path.join(directory, entry.name);
      return entry.isDirectory()
        ? keys(file)
        : /\.tsx?$/.test(file) && !/\.test\.ts$/.test(file)
          ? translationKeys(file, fs.readFileSync(file, 'utf8'))
          : [];
    });
  }
  if (Object.hasOwn(catalogs, 'en'))
    errors.push(...checkCatalogs(catalogs.en, catalogs, keys(root)));
}
if (errors.length) {
  console.error(errors.join('\n'));
  process.exitCode = 1;
} else {
  process.stdout.write(
    config.checkMissingTranslations || process.argv.includes('--strict')
      ? 'Translation catalogs passed.\n'
      : 'Translation completeness check disabled (use --strict to run once).\n',
  );
}
