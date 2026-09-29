import fs from 'node:fs';
import path from 'node:path';
import ts from 'typescript';

export function layer(file) {
  if (file.startsWith('features/')) return `feature:${file.split('/')[1]}`;
  for (const prefix of ['pages', 'components', 'lib', 'config', 'styles', 'app']) {
    if (file.startsWith(`${prefix}/`)) return prefix;
  }
  if (['main.tsx', 'router.tsx', 'vite-env.d.ts'].includes(file)) return 'app';
  return undefined;
}

export function importViolation(from, to) {
  const source = layer(from);
  const target = layer(to);
  if (!source || !target) return 'unclassified source; choose an architectural layer';
  if (source.startsWith('feature:') && target.startsWith('feature:') && source !== target)
    return 'features must not import each other; compose them in a page or app';
  if (
    target.startsWith('feature:') &&
    source !== target &&
    to !== `features/${to.split('/')[1]}/index.ts`
  )
    return 'consume features through their public index.ts';
  if (to === 'lib/api.ts' && !source.startsWith('feature:') && from !== 'lib/api.test.ts')
    return 'only feature APIs may consume the HTTP client';
  if (to === 'lib/api.generated.ts' && !source.startsWith('feature:') && from !== 'lib/schemas.ts')
    return 'wire types belong at validated API boundaries';
  if (source === 'config' && target !== 'config') return 'configuration must be independent';
  if (source === 'lib' && !['lib', 'config'].includes(target))
    return 'shared libraries cannot depend on UI or features';
  if (source === 'components' && !['components', 'lib', 'config', 'styles'].includes(target))
    return 'shared controls cannot depend on pages, app, or features';
  if (source === 'components' && ['lib/query-client.ts', 'lib/schemas.ts'].includes(to))
    return 'shared controls must receive data and callbacks through props';
  if (source.startsWith('feature:') && ['app', 'pages'].includes(target))
    return 'features cannot depend on application composition';
  if (source === 'pages' && target === 'app')
    return 'pages cannot depend on application composition';
  return undefined;
}

export function inspectSource(file, source) {
  const tree = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true);
  const imports = [];
  const errors = [];
  const network = new Set(['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'sendBeacon']);
  function record(node) {
    if (node && ts.isStringLiteralLike(node)) imports.push(node.text);
    else errors.push('module imports must have literal paths');
  }
  function visit(node) {
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) {
      if (node.moduleSpecifier) record(node.moduleSpecifier);
    }
    if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument))
      record(node.argument.literal);
    if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) && node.expression.text === 'require'))
    )
      record(node.arguments[0]);
    if (ts.isImportEqualsDeclaration(node)) errors.push('use ECMAScript imports');
    if (
      ts.isIdentifier(node) &&
      network.has(node.text) &&
      file !== 'lib/api.ts' &&
      !file.endsWith('.test.ts')
    )
      errors.push('network access belongs in lib/api.ts');
    if (
      ts.isElementAccessExpression(node) &&
      node.argumentExpression &&
      ts.isStringLiteralLike(node.argumentExpression) &&
      network.has(node.argumentExpression.text) &&
      file !== 'lib/api.ts'
    )
      errors.push('network access belongs in lib/api.ts');
    ts.forEachChild(node, visit);
  }
  visit(tree);
  return { imports, errors: [...new Set(errors)] };
}

export function cycles(graph) {
  const done = new Set();
  const active = [];
  const errors = [];
  function visit(file) {
    if (active.includes(file)) {
      errors.push([...active.slice(active.indexOf(file)), file].join(' -> '));
      return;
    }
    if (done.has(file)) return;
    active.push(file);
    for (const next of graph.get(file) ?? []) visit(next);
    active.pop();
    done.add(file);
  }
  for (const file of graph.keys()) visit(file);
  return errors;
}

export function check(root = path.resolve('apps/web')) {
  const configPath = path.join(root, 'tsconfig.json');
  const config = ts.readConfigFile(configPath, ts.sys.readFile);
  if (config.error)
    throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'));
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root);
  if (parsed.errors.length)
    throw new Error(
      parsed.errors
        .map((error) => ts.flattenDiagnosticMessageText(error.messageText, '\n'))
        .join('\n'),
    );
  const src = path.join(root, 'src');
  // Walk the tree independently of tsconfig include/exclude so new files cannot escape.
  function files(directory) {
    return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
      const absolute = path.join(directory, entry.name);
      return entry.isDirectory()
        ? files(absolute)
        : /\.[cm]?[jt]sx?$/.test(entry.name)
          ? [absolute]
          : [];
    });
  }
  const graph = new Map();
  const errors = [];
  for (const absolute of files(src)) {
    const file = path.relative(src, absolute).split(path.sep).join('/');
    if (!layer(file)) errors.push(`${file}: unclassified source file`);
    if (file === 'lib/api.generated.ts') continue;
    const result = inspectSource(file, fs.readFileSync(absolute, 'utf8'));
    errors.push(...result.errors.map((error) => `${file}: ${error}`));
    const edges = [];
    for (const specifier of result.imports) {
      const resolved = ts.resolveModuleName(
        specifier,
        absolute,
        parsed.options,
        ts.sys,
      ).resolvedModule;
      if (!resolved || resolved.isExternalLibraryImport) continue; // tsc validates unresolved imports.
      const target = path.relative(src, resolved.resolvedFileName).split(path.sep).join('/');
      if (target.startsWith('../')) {
        errors.push(`${file}: imports outside src are forbidden`);
        continue;
      }
      const violation = importViolation(file, target);
      if (violation) errors.push(`${file} -> ${target}: ${violation}`);
      edges.push(target);
    }
    graph.set(file, edges);
  }
  errors.push(...cycles(graph).map((cycle) => `dependency cycle: ${cycle}`));
  return errors;
}
