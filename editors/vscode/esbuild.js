// Bundles the LD webview UI (src/ldWebview/main.ts) into a single IIFE the
// webview loads with a nonce. Output is committed (media/ldEditor/main.js),
// mirroring the committed dist/ convention, and is deterministic — rebuilding
// without source changes produces a byte-identical file (checked in CI via
// `git diff --exit-code -- media`).
'use strict';

const esbuild = require('esbuild');
const path = require('node:path');

const root = __dirname;

esbuild
  .build({
    entryPoints: [path.join(root, 'src', 'ldWebview', 'main.ts')],
    bundle: true,
    format: 'iife',
    target: 'es2022',
    outfile: path.join(root, 'media', 'ldEditor', 'main.js'),
    legalComments: 'none',
    sourcemap: false,
    logLevel: 'info',
  })
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
