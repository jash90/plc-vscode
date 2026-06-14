const assert = require('assert');
const path = require('path');

const {
  requiredServerEntries,
  listZipEntries,
  verifyVsixEntries,
  resolvePlatform,
} = require(path.join(__dirname, '..', 'scripts', 'verify-vsix.js'));

// Build a minimal ZIP buffer containing only a central directory + EOCD record
// with the given entry names. listZipEntries only reads the central directory,
// so this is enough to exercise the parser without any external tooling.
function makeZip(names) {
  const headers = names.map((name) => {
    const nameBuf = Buffer.from(name, 'utf8');
    const header = Buffer.alloc(46);
    header.writeUInt32LE(0x02014b50, 0); // central directory file header signature
    header.writeUInt16LE(nameBuf.length, 28); // file name length
    return Buffer.concat([header, nameBuf]);
  });
  const centralDirectory = Buffer.concat(headers);

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0); // end of central directory signature
  eocd.writeUInt16LE(names.length, 8); // records on this disk
  eocd.writeUInt16LE(names.length, 10); // total records
  eocd.writeUInt32LE(centralDirectory.length, 12); // size of central directory
  eocd.writeUInt32LE(0, 16); // central directory offset (starts at buffer start)

  return Buffer.concat([centralDirectory, eocd]);
}

// listZipEntries extracts the stored names from a real ZIP central directory.
const zip = makeZip([
  'extension/package.json',
  'extension/server/plc-lsp-server',
  'extension/server/plc',
]);
assert.deepStrictEqual(listZipEntries(zip).sort(), [
  'extension/package.json',
  'extension/server/plc',
  'extension/server/plc-lsp-server',
]);

// Required entries are platform-aware (.exe only on Windows).
assert.deepStrictEqual(requiredServerEntries('linux'), [
  'extension/server/plc-lsp-server',
  'extension/server/plc',
]);
assert.deepStrictEqual(requiredServerEntries('win32'), [
  'extension/server/plc-lsp-server.exe',
  'extension/server/plc.exe',
]);

// A VSIX with both binaries passes.
const ok = verifyVsixEntries(
  ['extension/package.json', 'extension/server/plc-lsp-server', 'extension/server/plc'],
  'linux',
);
assert.strictEqual(ok.ok, true);
assert.deepStrictEqual(ok.missing, []);

// server/ missing entirely fails, reporting both binaries and no server dir.
const noServer = verifyVsixEntries(['extension/package.json'], 'linux');
assert.strictEqual(noServer.ok, false);
assert.strictEqual(noServer.hasServerDir, false);
assert.strictEqual(noServer.missing.length, 2);

// Only one binary bundled fails, naming the missing one.
const onlyServer = verifyVsixEntries(['extension/server/plc-lsp-server'], 'linux');
assert.strictEqual(onlyServer.ok, false);
assert.strictEqual(onlyServer.hasServerDir, true);
assert.deepStrictEqual(onlyServer.missing, ['extension/server/plc']);

// Windows requires the .exe suffix: non-suffixed names do not satisfy it.
const winNoExe = verifyVsixEntries(
  ['extension/server/plc-lsp-server', 'extension/server/plc'],
  'win32',
);
assert.strictEqual(winNoExe.ok, false);

const winOk = verifyVsixEntries(
  ['extension/server/plc-lsp-server.exe', 'extension/server/plc.exe'],
  'win32',
);
assert.strictEqual(winOk.ok, true);

// Platform resolution: explicit flags win, otherwise derived from the target.
assert.strictEqual(resolvePlatform(['--platform', 'win32']), 'win32');
assert.strictEqual(resolvePlatform(['--target', 'win32-x64']), 'win32');
assert.strictEqual(resolvePlatform(['--target', 'linux-x64']), 'linux');
assert.strictEqual(resolvePlatform(['--target', 'darwin-arm64']), 'darwin');
// darwin and linux both omit the .exe suffix.
assert.deepStrictEqual(requiredServerEntries('darwin'), requiredServerEntries('linux'));

console.log('verify-vsix ok');
