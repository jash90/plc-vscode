"use strict";
/**
 * E2E test-hook re-exports (PLC-117): thin barrel over ldEditor's exported
 * accessors so extension.ts can import them in one line.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.activeLdSimulation = exports.activeLdProvider = exports.activeLdDocument = void 0;
var ldEditor_1 = require("./ldEditor");
Object.defineProperty(exports, "activeLdDocument", { enumerable: true, get: function () { return ldEditor_1.activeLdDocument; } });
Object.defineProperty(exports, "activeLdProvider", { enumerable: true, get: function () { return ldEditor_1.activeLdProvider; } });
Object.defineProperty(exports, "activeLdSimulation", { enumerable: true, get: function () { return ldEditor_1.activeLdSimulation; } });
//# sourceMappingURL=ldTestHooks.js.map