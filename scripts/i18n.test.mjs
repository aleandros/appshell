import assert from 'node:assert/strict';
import { test } from 'node:test';
import { checkCatalogs, checkLocaleConfig, translationKeys } from './i18n.mjs';

test('catalog check catches missing, empty, unknown and mismatched interpolations', () => {
  const source = { Hello: 'Hello {{name}}', Bye: 'Bye' };
  assert.deepEqual(
    checkCatalogs(source, { es: { Hello: 'Hola {{name}}', Bye: 'Adiós' } }, ['Hello']),
    [],
  );
  const errors = checkCatalogs(
    source,
    { es: { Hello: 'Hola {{person}}', Bye: '', Extra: 'Extra' } },
    ['Missing'],
  );
  assert.ok(errors.some((error) => error.includes('missing source key Missing')));
  assert.ok(errors.some((error) => error.includes('interpolation mismatch')));
  assert.ok(errors.some((error) => error.includes('empty translation Bye')));
  assert.ok(errors.some((error) => error.includes('unknown translation Extra')));
});
test('plural forms follow each locale, including Arabic and Japanese', () => {
  const source = { seats_one: '{{count}} seat', seats_other: '{{count}} seats' };
  assert.deepEqual(checkCatalogs(source, { ja: { seats_other: '{{count}} 席' } }, ['seats']), []);
  const errors = checkCatalogs(source, { ar: source });
  for (const category of ['zero', 'two', 'few', 'many'])
    assert.ok(errors.some((error) => error.includes(`seats_${category}`)));
  assert.ok(
    checkCatalogs(source, { es: { ...source, seats_zero: 'No seats' } }).some((error) =>
      error.includes('interpolation mismatch'),
    ),
  );
});
test('checks ordinal forms and rejects nested/non-string catalogs', () => {
  const source = {
    place_ordinal_one: '{{count}}st',
    place_ordinal_two: '{{count}}nd',
    place_ordinal_few: '{{count}}rd',
    place_ordinal_other: '{{count}}th',
  };
  assert.deepEqual(
    checkCatalogs(source, { ja: { place_ordinal_other: '{{count}}番目' } }, ['place']),
    [],
  );
  assert.ok(checkCatalogs({ key: 'Value' }, { es: { key: { nested: 'No' } } }).length);
  assert.ok(checkCatalogs({}, { es: [] }).length);
  assert.ok(checkCatalogs(null, {}).length);
});
test('extracts literal translation references without treating dynamic data as keys', () => {
  assert.deepEqual(
    translationKeys(
      'screen.tsx',
      "t('Hello'); i18n.t('Bye'); t(user.name); const x = <p>{t('Title')}</p>",
    ),
    ['Hello', 'Bye', 'Title'],
  );
});
test('validates safe canonical config before opening catalog paths', () => {
  const config = {
    defaultLanguage: 'en',
    languages: [{ tag: 'en', label: 'English', direction: 'ltr' }],
    checkMissingTranslations: false,
  };
  assert.deepEqual(checkLocaleConfig(config), []);
  assert.ok(checkLocaleConfig({ ...config, languages: [null] }).length);
  assert.ok(checkLocaleConfig({ ...config, defaultLanguage: 'es' }).length);
  assert.ok(
    checkLocaleConfig({
      ...config,
      languages: [{ tag: '../secret', label: '', direction: 'sideways' }],
    }).length,
  );
});
