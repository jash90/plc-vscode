/**
 * Shared child-process capture helper (stdout on success, stderr on
 * failure) for the CLI-driven commands. Extracted from ldEditor's
 * updatePowerFlow so every caller shares one implementation.
 */

import { spawn } from 'node:child_process';

export interface CaptureInvocation {
  command: string;
  args: string[];
  cwd?: string;
}

/** Run an invocation and resolve with stdout (reject on non-zero exit). */
export function capture(invocation: CaptureInvocation): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(
      invocation.command,
      invocation.args,
      invocation.cwd ? { cwd: invocation.cwd } : undefined,
    );
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    child.on('close', (code: number | null) => {
      if (code === 0) {
        resolve(stdout);
      } else {
        reject(new Error(stderr || `Exit code ${code}`));
      }
    });
    child.on('error', reject);
  });
}
