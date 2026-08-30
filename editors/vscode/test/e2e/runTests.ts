// PLC-117 — E2E runner: boots the Extension Development Host with the
// prebuilt debug CLI, runs the LD editor suite. `npm run test:e2e`.
import { downloadAndUnzipVSCode, resolveCliPathFromVSCodeExecutablePath, runTests } from '@vscode/test-electron';
import * as path from 'node:path';

async function main(): Promise<void> {
  const extensionDevelopmentPath = path.resolve(__dirname, '../..');
  const extensionTestsPath = path.resolve(__dirname, 'suite');

  // The suite configures plcVscode.cliCommand to the prebuilt binary.
  const vscodeExecutablePath = await downloadAndUnzipVSCode('stable');
  const cliPath = resolveCliPathFromVSCodeExecutablePath(vscodeExecutablePath);

  const failure = await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    vscodeExecutablePath,
    launchArgs: ['--disable-extensions'],
  });
  process.exit(failure ? 1 : 0);
}

void main().catch((error) => {
  console.error(error);
  process.exit(1);
});
