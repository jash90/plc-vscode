/**
 * E2E test-hook re-exports (PLC-117): thin barrel over ldEditor's exported
 * accessors so extension.ts can import them in one line.
 */

export {
  activeLdDocument,
  activeLdProvider,
  activeLdSimulation,
} from './ldEditor';
