// PLC-117 — LD editor end-to-end: open a fixture through the real custom
// editor, drive edits via the documented message surface, save, simulate,
// and assert through the testState hook. Runs inside the Extension
// Development Host.
import * as assert from 'node:assert';
import * as path from 'node:path';
import * as vscode from 'vscode';

const ROOT = path.resolve(__dirname, '../../../..');

async function withDelay<T>(promise: Promise<T>, ms: number): Promise<T> {
  const value = await promise;
  await new Promise((resolve) => setTimeout(resolve, ms));
  return value;
}

suite('LD editor e2e', function () {
  this.timeout(60000);

  suiteSetup(async function () {
    // The tests drive the prebuilt debug CLI (dev-mode cargo spawns are
    // slow and need the workspace root).
    const config = vscode.workspace.getConfiguration('plcVscode');
    await config.update(
      'cliCommand',
      path.join(ROOT, 'target', 'debug', 'plc'),
      vscode.ConfigurationTarget.Global,
    );
    await config.update('cliArgs', [], vscode.ConfigurationTarget.Global);
  });

  test('opens LD fixture in the custom editor', async () => {
    const uri = vscode.Uri.file(path.join(ROOT, 'tests/ld/motor_control_v2.ld'));
    const doc = await withDelay(
      vscode.commands.executeCommand(
        'vscode.openWith',
        uri,
        'plc-vscode.ldEditor',
      ) as Promise<unknown>,
      1500,
    );
    void doc;
    const state = await vscode.commands.executeCommand('plc-vscode.ld.testState') as {
      programJson: string;
      dirty: boolean;
    };
    const program = JSON.parse(state.programJson);
    assert.strictEqual(program.rungs.length, 4, 'fixture loaded');
    assert.strictEqual(program.name, 'MotorControl');
  });

  test('undo restores the previous model after an edit command', async () => {
    const before = await vscode.commands.executeCommand('plc-vscode.ld.testState') as {
      programJson: string;
    };
    await vscode.commands.executeCommand('plc-vscode.ld.edit', {
      type: 'addRung',
      label: 'Add rung',
      rung: -1,
      branch: -1,
      index: -1,
    });
    await new Promise((resolve) => setTimeout(resolve, 500));
    const after = await vscode.commands.executeCommand('plc-vscode.ld.testState') as {
      programJson: string;
      dirty: boolean;
    };
    assert.strictEqual(JSON.parse(after.programJson).rungs.length, JSON.parse(before.programJson).rungs.length + 1);
    assert.strictEqual(after.dirty, true, 'edit marks the document dirty');

    await vscode.commands.executeCommand('plc-vscode.ld.undo');
    await new Promise((resolve) => setTimeout(resolve, 500));
    const undone = await vscode.commands.executeCommand('plc-vscode.ld.testState') as {
      programJson: string;
    };
    assert.deepStrictEqual(JSON.parse(undone.programJson).rungs.length, JSON.parse(before.programJson).rungs.length);
  });

  test('simulation produces power flow for the seal-in rung', async () => {
    this.timeout(20000);
    await vscode.commands.executeCommand('plc-vscode.ld.simStep');
    // Toggle Start through the input hook, step again.
    await vscode.commands.executeCommand('plc-vscode.ld.simInput', 'Start', true);
    await vscode.commands.executeCommand('plc-vscode.ld.simStep');
    // Serve events arrive asynchronously over stdout — poll with deadline.
    const deadline = Date.now() + 10000;
    for (;;) {
      const state = await vscode.commands.executeCommand('plc-vscode.ld.testState') as {
        powerFlow?: { rungs?: Array<{ rung_result?: boolean }> };
      };
      const rungs = state.powerFlow?.rungs;
      if (rungs && rungs.length > 0) {
        assert.strictEqual(rungs[0].rung_result, true, 'seal-in rung energized with Start on');
        return;
      }
      if (Date.now() > deadline) {
        assert.fail('power flow present (timed out waiting for serve events)');
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  });

  test('generated ST opens as a virtual document', async () => {
    await vscode.commands.executeCommand(
      'plc-vscode.showGeneratedSt',
      vscode.Uri.file(path.join(ROOT, 'tests/ld/motor_control_v2.ld')),
    );
    const deadline = Date.now() + 10000;
    for (;;) {
      const editor = vscode.window.activeTextEditor;
      if (editor && editor.document.uri.scheme === 'plc-ld-st') {
        assert.ok(editor.document.getText().includes('PROGRAM'), 'ST content generated');
        return;
      }
      if (Date.now() > deadline) {
        assert.fail('generated ST view opened (timed out)');
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  });
});
