const assert = require('assert');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));

assert.strictEqual(pkg.name, 'plc-vscode');
assert.strictEqual(pkg.activationEvents.includes('onLanguage:structured-text'), true);
assert.strictEqual(pkg.main, './dist/extension.js');
assert.ok(pkg.contributes.languages.some((language) => language.id === 'structured-text'));
assert.ok(pkg.contributes.commands.some((command) => command.command === 'plc-vscode.showStatus'));
assert.ok(pkg.contributes.commands.some((command) => command.command === 'plc-vscode.runCurrentFile'));

// Syntax highlighting: a TextMate grammar must be contributed for the language.
assert.ok(Array.isArray(pkg.contributes.grammars), 'grammars contribution missing');
const grammar = pkg.contributes.grammars.find(
  (entry) => entry.language === 'structured-text' && entry.scopeName === 'source.st',
);
assert.ok(grammar, 'structured-text grammar not registered');
const grammarPath = path.join(root, grammar.path);
assert.ok(fs.existsSync(grammarPath), `grammar file missing: ${grammar.path}`);
const grammarJson = JSON.parse(fs.readFileSync(grammarPath, 'utf8'));
assert.strictEqual(grammarJson.scopeName, 'source.st');
assert.ok(Array.isArray(grammarJson.patterns) && grammarJson.patterns.length > 0);

// Stepping debugger: a `plc-st` debug type must be contributed for the language,
// breakpoints must be enabled, and the activation event must be present.
assert.ok(Array.isArray(pkg.contributes.debuggers), 'debuggers contribution missing');
const debugger_ = pkg.contributes.debuggers.find((entry) => entry.type === 'plc-st');
assert.ok(debugger_, 'plc-st debugger not registered');
assert.ok(
  Array.isArray(debugger_.languages) && debugger_.languages.includes('structured-text'),
  'plc-st debugger must target structured-text',
);
assert.ok(
  Array.isArray(pkg.contributes.breakpoints) &&
    pkg.contributes.breakpoints.some((entry) => entry.language === 'structured-text'),
  'breakpoints not enabled for structured-text',
);
assert.ok(
  pkg.activationEvents.includes('onDebugResolve:plc-st'),
  'onDebugResolve:plc-st activation event missing',
);

// Right-click Run/Debug: the debug command and context menus must be contributed.
assert.ok(
  pkg.contributes.commands.some((command) => command.command === 'plc-vscode.debugCurrentFile'),
  'plc-vscode.debugCurrentFile command not registered',
);
assert.ok(pkg.contributes.menus, 'menus contribution missing');

const menuTargets = ['editor/context', 'explorer/context', 'editor/title/run'];
for (const menuId of menuTargets) {
  const entries = pkg.contributes.menus[menuId];
  assert.ok(Array.isArray(entries) && entries.length > 0, `menus['${menuId}'] missing`);

  const runEntry = entries.find((entry) => entry.command === 'plc-vscode.runCurrentFile');
  const debugEntry = entries.find((entry) => entry.command === 'plc-vscode.debugCurrentFile');
  assert.ok(runEntry, `Run not in menus['${menuId}']`);
  assert.ok(debugEntry, `Debug not in menus['${menuId}']`);

  // editor menus scope via editorLangId; explorer + title-run via resourceLangId.
  const langCtx = menuId === 'editor/context' ? 'editorLangId' : 'resourceLangId';
  for (const entry of [runEntry, debugEntry]) {
    assert.ok(
      typeof entry.when === 'string' && entry.when.includes(`${langCtx} == structured-text`),
      `menus['${menuId}'] entry for ${entry.command} must scope via ${langCtx} == structured-text`,
    );
    assert.ok(
      typeof entry.group === 'string' && entry.group.length > 0,
      `menus['${menuId}'] entry for ${entry.command} must declare a group`,
    );
  }

  // Run must sort before Debug within the same group.
  assert.ok(
    runEntry.group < debugEntry.group,
    `menus['${menuId}']: Run (${runEntry.group}) must order before Debug (${debugEntry.group})`,
  );
}

// Every command referenced by a menu must be a declared contributes.command.
const declaredCommands = new Set(pkg.contributes.commands.map((c) => c.command));
for (const menuId of menuTargets) {
  for (const entry of pkg.contributes.menus[menuId]) {
    assert.ok(
      declaredCommands.has(entry.command),
      `menus['${menuId}'] references undeclared command ${entry.command}`,
    );
  }
}

assert.ok(
  pkg.activationEvents.includes('onCommand:plc-vscode.debugCurrentFile'),
  'onCommand:plc-vscode.debugCurrentFile activation event missing',
);

// Production bundling: the compiled binary-path helper must resolve under
// server/ and be platform-aware.
const { bundledBinaryRelativePath, SERVER_BINARY } = require(path.join(root, 'dist', 'bundled.js'));
assert.strictEqual(
  bundledBinaryRelativePath(SERVER_BINARY, 'linux'),
  path.join('server', 'plc-lsp-server'),
);
assert.strictEqual(
  bundledBinaryRelativePath(SERVER_BINARY, 'win32'),
  path.join('server', 'plc-lsp-server.exe'),
);

// server/ must not be excluded from the package.
const vscodeignore = fs.readFileSync(path.join(root, '.vscodeignore'), 'utf8');
assert.ok(!/^server(\/|\b)/m.test(vscodeignore), 'server/ must be packaged, not ignored');

console.log('package contract ok');
