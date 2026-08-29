"use strict";
/**
 * PLCopen XML export/import commands (PLC-115). Both drive the `plc` CLI's
 * special-cased `convert` (model-level interchange through plc_plcopen) —
 * the extension never reimplements the mapping.
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
exports.exportPlcopen = exportPlcopen;
exports.importPlcopen = importPlcopen;
const vscode = __importStar(require("vscode"));
const ldCapture_1 = require("./ldCapture");
const ldCli_1 = require("./ldCli");
async function exportPlcopen(context) {
    const editor = vscode.window.activeTextEditor;
    const target = editor?.document.uri ??
        (await pickFile('Choose the .ld file to export'))?.fsPath;
    if (!target) {
        return;
    }
    const source = typeof target === 'string' ? vscode.Uri.file(target) : target;
    const destination = vscode.Uri.file(source.fsPath.replace(/\.ld$/, '.plcopen'));
    const invocation = (0, ldCli_1.resolveRunInvocation)(context, 'convert', [
        'ld',
        'plcopen',
        source.fsPath,
    ]);
    try {
        const xml = await (0, ldCapture_1.capture)(invocation);
        await vscode.workspace.fs.writeFile(destination, Buffer.from(xml, 'utf8'));
        void vscode.window.showInformationMessage(`Exported ${destination.fsPath}`);
    }
    catch (error) {
        void vscode.window.showErrorMessage(`PLCopen export failed: ${error.message}`);
    }
}
async function importPlcopen(context) {
    const picked = await vscode.window.showOpenDialog({
        canSelectMany: false,
        filters: { 'PLCopen XML': ['plcopen', 'xml'] },
    });
    if (!picked || picked.length === 0) {
        return;
    }
    const source = picked[0];
    const destination = vscode.Uri.file(source.fsPath.replace(/\.(plcopen|xml)$/, '.ld'));
    const invocation = (0, ldCli_1.resolveRunInvocation)(context, 'convert', [
        'plcopen',
        'ld',
        source.fsPath,
    ]);
    try {
        const json = await (0, ldCapture_1.capture)(invocation);
        await vscode.workspace.fs.writeFile(destination, Buffer.from(json, 'utf8'));
        void vscode.window.showTextDocument(destination);
        void vscode.window.showInformationMessage(`Imported ${destination.fsPath}`);
    }
    catch (error) {
        void vscode.window.showErrorMessage(`PLCopen import failed: ${error.message}`);
    }
}
async function pickFile(label) {
    const picked = await vscode.window.showOpenDialog({
        canSelectMany: false,
        openLabel: label,
        filters: { 'Ladder Diagram': ['ld'] },
    });
    return picked?.[0];
}
//# sourceMappingURL=plcopen.js.map