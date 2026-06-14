"use strict";
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
exports.CLI_BINARY = exports.SERVER_BINARY = void 0;
exports.bundledBinaryRelativePath = bundledBinaryRelativePath;
const path = __importStar(require("node:path"));
/** Base names of the binaries bundled inside a packaged extension. */
exports.SERVER_BINARY = 'plc-lsp-server';
exports.CLI_BINARY = 'plc';
/**
 * Relative path (inside the packaged extension) to a bundled binary for the
 * given platform. Binaries live under `server/` and gain a `.exe` suffix on
 * Windows. Pure (no `vscode` import) so it can be unit-tested in Node.
 */
function bundledBinaryRelativePath(base, platform = process.platform) {
    const suffix = platform === 'win32' ? '.exe' : '';
    return path.join('server', `${base}${suffix}`);
}
//# sourceMappingURL=bundled.js.map