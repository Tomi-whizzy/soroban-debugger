import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import { DebuggerProcess, validateLaunchConfig } from '../cli/debuggerProcess';
import { resolveSourceBreakpoints } from '../dap/sourceBreakpoints';

async function main(): Promise<void> {
  const extensionRoot = process.cwd();
  const repoRoot = path.resolve(extensionRoot, '..', '..');

  const emittedFiles = [
    path.join(extensionRoot, 'dist', 'extension.js'),
    path.join(extensionRoot, 'dist', 'debugAdapter.js'),
    path.join(extensionRoot, 'dist', 'cli', 'debuggerProcess.js')
  ];

  for (const file of emittedFiles) {
    assert.ok(fs.existsSync(file), `Missing compiled artifact: ${file}`);
  }

  const preflightBinaryPath = emittedFiles[0];
  const contractPath = path.join(repoRoot, 'tests', 'fixtures', 'wasm', 'echo.wasm');
  assert.ok(fs.existsSync(contractPath), `Missing fixture WASM: ${contractPath}`);
  const snapshotPath = path.join(repoRoot, 'extensions', 'vscode', 'package.json');

  const goodPreflight = await validateLaunchConfig({
    binaryPath: preflightBinaryPath,
    contractPath,
    snapshotPath,
    entrypoint: 'echo',
    args: ['7'],
    token: 'debug-token-1234567890'
  });
  assert.equal(goodPreflight.ok, true, 'Expected valid launch configuration to pass preflight');

  const missingContract = await validateLaunchConfig({
    binaryPath: preflightBinaryPath,
    contractPath: path.join(repoRoot, 'missing-contract.wasm'),
    entrypoint: 'echo',
    args: []
  });
  assert.equal(missingContract.ok, false, 'Expected missing contract path to fail preflight');
  assert.equal(missingContract.issues[0].field, 'contractPath');
  assert.match(missingContract.issues[0].message, /contractPath/);

  const badArgs = await validateLaunchConfig({
    binaryPath: preflightBinaryPath,
    contractPath,
    entrypoint: 'echo',
    args: [{ nested: undefined }]
  });
  assert.equal(badArgs.ok, false, 'Expected non-serializable args to fail preflight');
  assert.equal(badArgs.issues[0].field, 'args');
  assert.match(badArgs.issues[0].message, /\$\.0\.nested/);

  const badPort = await validateLaunchConfig({
    binaryPath: preflightBinaryPath,
    contractPath,
    entrypoint: 'echo',
    args: [],
    port: 70000
  });
  assert.equal(badPort.ok, false, 'Expected out-of-range port to fail preflight');
  assert.equal(badPort.issues[0].field, 'port');

  const badToken = await validateLaunchConfig({
    binaryPath: preflightBinaryPath,
    contractPath,
    entrypoint: 'echo',
    args: [],
    token: '   '
  });
  assert.equal(badToken.ok, false, 'Expected blank token to fail preflight');
  assert.equal(badToken.issues[0].field, 'token');

  const shortToken = await validateLaunchConfig({
    binaryPath: preflightBinaryPath,
    contractPath,
    entrypoint: 'echo',
    args: [],
    token: 'short-token'
  });
  assert.equal(shortToken.ok, false, 'Expected short token to fail preflight');
  assert.equal(shortToken.issues[0].field, 'token');
  assert.match(shortToken.issues[0].expected, /32-byte token/i);

  const binaryPath = process.env.SOROBAN_DEBUG_BIN
    || path.join(repoRoot, 'target', 'debug', process.platform === 'win32' ? 'soroban-debug.exe' : 'soroban-debug');

  if (!fs.existsSync(binaryPath)) {
    console.log(`Skipping debugger smoke test because the CLI binary was not found at ${binaryPath}`);
    return;
  }

  const debuggerProcess = new DebuggerProcess({
    binaryPath,
    contractPath,
    entrypoint: 'echo',
    args: ['7']
  });

  await debuggerProcess.start();
  await debuggerProcess.ping();

  const sourcePath = path.join(repoRoot, 'tests', 'fixtures', 'contracts', 'echo', 'src', 'lib.rs');
  const exportedFunctions = await debuggerProcess.getContractFunctions();
  const resolvedBreakpoints = resolveSourceBreakpoints(sourcePath, [10], exportedFunctions);
  assert.equal(resolvedBreakpoints[0].verified, true, 'Expected echo breakpoint to resolve');
  assert.equal(resolvedBreakpoints[0].functionName, 'echo');

  await debuggerProcess.setBreakpoint({
    id: 'echo',
    functionName: 'echo'
  });
  const paused = await debuggerProcess.execute();
  assert.equal(paused.paused, true, 'Expected breakpoint to pause before execution');

  const pausedInspection = await debuggerProcess.inspect();
  assert.match(pausedInspection.args || '', /7/, 'Expected paused inspection to include call args');

  const resumed = await debuggerProcess.continueExecution();
  assert.match(resumed.output || '', /7/, 'Expected continue() to finish echo()');
  await debuggerProcess.clearBreakpoint('echo');

  const result = await debuggerProcess.execute();
  assert.match(result.output, /7/, 'Expected second echo() to return the input');

  const inspection = await debuggerProcess.inspect();
  assert.ok(Array.isArray(inspection.callStack), 'Expected call stack array from inspection');
  assert.match(inspection.args || '', /7/, 'Expected inspection to include args');

  const storage = await debuggerProcess.getStorage();
  assert.ok(typeof storage === 'object' && storage !== null, 'Expected storage snapshot object');

  await debuggerProcess.stop();
  console.log('VS Code extension smoke tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
