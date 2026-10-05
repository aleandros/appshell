import ts from 'typescript';

const pluralSuffix = /_(zero|one|two|few|many|other)$/;
function placeholders(message) {
  return [
    ...new Set([...message.matchAll(/{{\s*-?\s*([^},\s]+)[^}]*}}/g)].map((match) => match[1])),
  ]
    .sort()
    .join(',');
}

export function checkLocaleConfig(config) {
  const errors = [];
  if (!config || !Array.isArray(config.languages) || !config.languages.length)
    return ['Configure at least one supported language.'];
  if (
    config.languages.some(
      (language) => !language || typeof language !== 'object' || Array.isArray(language),
    )
  )
    return ['Each language must be an object.'];
  if (
    Object.keys(config).some(
      (key) => !['defaultLanguage', 'languages', 'checkMissingTranslations'].includes(key),
    )
  )
    errors.push('Unknown language configuration option.');
  const tags = config.languages.map((language) => language.tag);
  if (!tags.includes(config.defaultLanguage)) errors.push('Default language must be supported.');
  if (new Set(tags).size !== tags.length) errors.push('Language tags must be unique.');
  if (typeof config.checkMissingTranslations !== 'boolean')
    errors.push('checkMissingTranslations must be boolean.');
  for (const language of config.languages) {
    if (Object.keys(language).some((key) => !['tag', 'label', 'direction'].includes(key)))
      errors.push('Unknown language option.');
    try {
      if (
        typeof language.tag !== 'string' ||
        !/^[a-zA-Z0-9-]+$/.test(language.tag) ||
        Intl.getCanonicalLocales(language.tag)[0] !== language.tag
      )
        throw new Error();
    } catch {
      errors.push(`Invalid canonical BCP 47 tag: ${String(language.tag)}`);
    }
    if (
      !['ltr', 'rtl'].includes(language.direction) ||
      typeof language.label !== 'string' ||
      !language.label.trim()
    )
      errors.push(`Language ${String(language.tag)} needs a label and ltr/rtl direction.`);
  }
  return errors;
}

/** Flat i18next JSON v4 catalogs, including locale-specific cardinal/ordinal plurals. */
export function checkCatalogs(source, catalogs, usedKeys = []) {
  const errors = [];
  if (!source || typeof source !== 'object' || Array.isArray(source))
    return ['en: source catalog must be a flat JSON object'];
  const valid = (value) => typeof value === 'string' && value.trim().length > 0;
  const pluralBases = new Set(
    Object.keys(source)
      .filter((key) => pluralSuffix.test(key))
      .map((key) => key.replace(pluralSuffix, '')),
  );
  for (const key of usedKeys) {
    if (!Object.hasOwn(source, key) && !pluralBases.has(key) && !pluralBases.has(`${key}_ordinal`))
      errors.push(`en: missing source key ${key}`);
  }
  for (const [tag, catalog] of Object.entries(catalogs)) {
    if (!catalog || typeof catalog !== 'object' || Array.isArray(catalog)) {
      errors.push(`${tag}: catalog must be a flat JSON object`);
      continue;
    }
    const required = new Map(Object.entries(source).filter(([key]) => !pluralSuffix.test(key)));
    for (const base of pluralBases) {
      const ordinal = base.endsWith('_ordinal');
      for (const category of new Intl.PluralRules(tag, {
        type: ordinal ? 'ordinal' : 'cardinal',
      }).resolvedOptions().pluralCategories) {
        required.set(
          `${base}_${category}`,
          source[`${base}_${category}`] ?? source[`${base}_other`],
        );
      }
    }
    for (const [key, reference] of required) {
      const value = catalog[key];
      if (!valid(value)) errors.push(`${tag}: missing or empty translation ${key}`);
      else if (!valid(reference) || placeholders(value) !== placeholders(reference))
        errors.push(`${tag}: interpolation mismatch for ${key}`);
    }
    for (const [key, value] of Object.entries(catalog)) {
      if (!valid(value)) errors.push(`${tag}: invalid translation ${key}`);
      else if (!required.has(key)) {
        const base = key.replace(pluralSuffix, '');
        const reference = source[key] ?? source[`${base}_other`];
        if (reference === undefined) errors.push(`${tag}: unknown translation ${key}`);
        else if (placeholders(value) !== placeholders(reference))
          errors.push(`${tag}: interpolation mismatch for ${key}`);
      }
    }
  }
  return errors;
}

export function translationKeys(file, source) {
  const keys = [];
  const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
  function visit(node) {
    if (
      ts.isCallExpression(node) &&
      ((ts.isIdentifier(node.expression) && node.expression.text === 't') ||
        (ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === 't'))
    ) {
      const key = node.arguments[0];
      if (key && ts.isStringLiteralLike(key)) keys.push(key.text);
    }
    if (
      ts.isJsxAttribute(node) &&
      node.name.getText(tree) === 'i18nKey' &&
      node.initializer &&
      ts.isStringLiteral(node.initializer)
    )
      keys.push(node.initializer.text);
    ts.forEachChild(node, visit);
  }
  visit(tree);
  return keys;
}
