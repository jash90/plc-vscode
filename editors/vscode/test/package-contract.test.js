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
console.log('package contract ok');
