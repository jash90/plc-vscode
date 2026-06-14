'use strict';

// VSIX packaging verification (PLC-61).
//
// Inspects the contents of a packaged `.vsix` (a ZIP archive) and asserts that
// both bundled binaries — the language server and the CLI — are present under
// `extension/server/`, with a `.exe` suffix on Windows targets. This guards the
// release pipeline against shipping a VSIX where `server/` is missing or only
// one of the two binaries was bundled.
//
// Dependency-free: reads the ZIP central directory directly so it can run with
// a bare Node install on any release runner.

const fs = require('node:fs');

const EOCD_SIGNATURE = 0x06054b50; // End of Central Directory record
const CDFH_SIGNATURE = 0x02014b50; // Central Directory File Header

/** Base names of the binaries that must be bundled in the VSIX. */
const SERVER_BINARY = 'plc-lsp-server';
const CLI_BINARY = 'plc';

/**
 * The VSIX-relative entry paths that must exist for a platform. VSIX archives
 * store extension files under `extension/`, and binaries gain a `.exe` suffix on
 * Windows (matching the release workflow's bundling step).
 */
function requiredServerEntries(platform) {
  const suffix = platform === 'win32' ? '.exe' : '';
  return [
    `extension/server/${SERVER_BINARY}${suffix}`,
    `extension/server/${CLI_BINARY}${suffix}`,
  ];
}

/**
 * List the file names stored in a ZIP buffer by walking its central directory.
 * Sufficient for non-zip64 archives such as a VSIX. Throws if the buffer is not
 * a readable ZIP.
 */
function listZipEntries(buffer) {
  let eocd = -1;
  for (let i = buffer.length - 22; i >= 0; i -= 1) {
    if (buffer.readUInt32LE(i) === EOCD_SIGNATURE) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) {
    throw new Error('not a ZIP archive: end-of-central-directory record not found');
  }

  const count = buffer.readUInt16LE(eocd + 10);
  let offset = buffer.readUInt32LE(eocd + 16);
  const names = [];

  for (let i = 0; i < count; i += 1) {
    if (buffer.readUInt32LE(offset) !== CDFH_SIGNATURE) {
      throw new Error('corrupt ZIP: central directory file header signature mismatch');
    }
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const nameStart = offset + 46;
    names.push(buffer.toString('utf8', nameStart, nameStart + nameLength));
    offset = nameStart + nameLength + extraLength + commentLength;
  }

  return names;
}

/**
 * Check a list of VSIX entry names for the required bundled binaries.
 * Returns `{ ok, required, missing, hasServerDir }`.
 */
function verifyVsixEntries(entries, platform) {
  const normalized = new Set(entries.map((entry) => entry.replace(/\\/g, '/')));
  const required = requiredServerEntries(platform);
  const missing = required.filter((entry) => !normalized.has(entry));
  const hasServerDir = [...normalized].some((entry) => entry.startsWith('extension/server/'));
  return { ok: missing.length === 0, required, missing, hasServerDir };
}

function getOption(args, name) {
  const index = args.indexOf(name);
  return index >= 0 && index + 1 < args.length ? args[index + 1] : undefined;
}

/** Resolve the target platform from `--platform`, `--target`, or the host. */
function resolvePlatform(args) {
  const explicit = getOption(args, '--platform');
  if (explicit) {
    return explicit;
  }
  const target = getOption(args, '--target');
  if (target) {
    if (target.startsWith('win32')) {
      return 'win32';
    }
    return target.startsWith('darwin') ? 'darwin' : 'linux';
  }
  return process.platform;
}

function main(argv) {
  const args = argv.slice(2);
  const vsixPath = args.find((arg) => !arg.startsWith('--'));
  if (!vsixPath) {
    console.error('usage: node scripts/verify-vsix.js <file.vsix> [--target <vsce-target> | --platform <node-platform>]');
    process.exitCode = 2;
    return;
  }

  const platform = resolvePlatform(args);

  let entries;
  try {
    entries = listZipEntries(fs.readFileSync(vsixPath));
  } catch (error) {
    console.error(`VSIX verification FAILED for ${vsixPath}: ${error.message}`);
    process.exitCode = 1;
    return;
  }

  const result = verifyVsixEntries(entries, platform);
  if (!result.ok) {
    console.error(`VSIX verification FAILED for ${vsixPath} (platform: ${platform})`);
    if (!result.hasServerDir) {
      console.error('  no extension/server/ directory is present in the VSIX');
    }
    console.error('  missing required bundled binaries:');
    for (const entry of result.missing) {
      console.error(`    - ${entry}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log(`VSIX verification ok: ${vsixPath} contains ${result.required.join(' and ')}`);
}

if (require.main === module) {
  main(process.argv);
}

module.exports = {
  SERVER_BINARY,
  CLI_BINARY,
  requiredServerEntries,
  listZipEntries,
  verifyVsixEntries,
  resolvePlatform,
  main,
};
