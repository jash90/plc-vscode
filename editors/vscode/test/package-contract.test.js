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

console.log('package contract ok');
