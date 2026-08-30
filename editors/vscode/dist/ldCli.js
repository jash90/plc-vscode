"use strict";
/**
 * Shared resolution of `plc` CLI invocations (dev cargo vs production
 * bundled binary). Extracted from extension.ts so the LD editor and the
 * run/debug commands resolve the binary identically — fixing the PLC-105
 * bug where the LD editor looked for `./dist/plc` while binaries ship
 * under `server/`.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.isProduction = isProduction;
exports.resolveRunInvocation = resolveRunInvocation;
const path = __importStar(require("node:path"));
const vscode = __importStar(require("vscode"));
const bundled_1 = require("./bundled");
function isProduction(context) {
    return context.extensionMode === vscode.ExtensionMode.Production;
}
function workspaceRoot(context) {
    const configured = vscode.workspace.getConfiguration('plcVscode').get('repositoryRoot', '');
    return configured || path.resolve(context.extensionPath, '..', '..');
}
/**
 * Build the command/args to invoke a `plc` subcommand (`run`, `ld`, `debug`,
 * …). Production runs the bundled binary directly; development drives the
 * workspace CLI via cargo, swapping the trailing `run` subcommand from
 * `cliArgs` for the requested one.
 */
function resolveRunInvocation(context, subcommand, extraArgs) {
    if (isProduction(context)) {
        return {
            command: context.asAbsolutePath((0, bundled_1.bundledBinaryRelativePath)(bundled_1.CLI_BINARY)),
            args: [subcommand, ...extraArgs],
        };
    }
    const config = vscode.workspace.getConfiguration('plcVscode');
    const command = config.get('cliCommand', 'cargo');
    const cliArgs = config.get('cliArgs', [
        'run',
        '--quiet',
        '--package',
        'plc_cli',
        '--',
        'run',
    ]);
    const cargoPrefix = cliArgs.slice(0, -1);
    return {
        command,
        args: [...cargoPrefix, subcommand, ...extraArgs],
        cwd: workspaceRoot(context),
    };
}
//# sourceMappingURL=ldCli.js.map