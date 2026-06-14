import * as path from 'node:path';

/** Base names of the binaries bundled inside a packaged extension. */
export const SERVER_BINARY = 'plc-lsp-server';
export const CLI_BINARY = 'plc';

/**
 * Relative path (inside the packaged extension) to a bundled binary for the
 * given platform. Binaries live under `server/` and gain a `.exe` suffix on
 * Windows. Pure (no `vscode` import) so it can be unit-tested in Node.
 */
export function bundledBinaryRelativePath(
  base: string,
  platform: NodeJS.Platform = process.platform,
): string {
  const suffix = platform === 'win32' ? '.exe' : '';
  return path.join('server', `${base}${suffix}`);
}
