"use strict";
/**
 * Shared child-process capture helper (stdout on success, stderr on
 * failure) for the CLI-driven commands. Extracted from ldEditor's
 * updatePowerFlow so every caller shares one implementation.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.capture = capture;
const node_child_process_1 = require("node:child_process");
/** Run an invocation and resolve with stdout (reject on non-zero exit). */
function capture(invocation) {
    return new Promise((resolve, reject) => {
        const child = (0, node_child_process_1.spawn)(invocation.command, invocation.args, invocation.cwd ? { cwd: invocation.cwd } : undefined);
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', (chunk) => {
            stdout += chunk.toString();
        });
        child.stderr.on('data', (chunk) => {
            stderr += chunk.toString();
        });
        child.on('close', (code) => {
            if (code === 0) {
                resolve(stdout);
            }
            else {
                reject(new Error(stderr || `Exit code ${code}`));
            }
        });
        child.on('error', reject);
    });
}
//# sourceMappingURL=ldCapture.js.map