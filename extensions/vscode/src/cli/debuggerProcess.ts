import { ChildProcess, execFile, spawn } from 'child_process';
import * as fs from 'fs';
import * as net from 'net';
import * as path from 'path';

export interface DebuggerProcessConfig {
  contractPath: string;
  snapshotPath?: string;
  entrypoint?: string;
  args?: unknown[];
  trace?: boolean;
  binaryPath?: string;
  port?: number;
  token?: string;
}

export type LaunchPreflightField =
  | 'binaryPath'
  | 'contractPath'
  | 'snapshotPath'
  | 'entrypoint'
  | 'args'
  | 'port'
  | 'token';

export type LaunchPreflightQuickFix =
  | 'pickBinary'
  | 'pickContract'
  | 'pickSnapshot'
  | 'openLaunchConfig'
  | 'generateLaunchConfig'
  | 'openSettings';

export interface LaunchPreflightIssue {
  field: LaunchPreflightField;
  message: string;
  expected: string;
  quickFixes: LaunchPreflightQuickFix[];
}

export interface LaunchPreflightResult {
  ok: boolean;
  issues: LaunchPreflightIssue[];
  resolvedBinaryPath: string;
}

export interface DebuggerExecutionResult {
  output: string;
  paused: boolean;
  completed: boolean;
}

export interface DebuggerInspection {
  function?: string;
  args?: string;
  stepCount: number;
  paused: boolean;
  callStack: string[];
}

export interface DebuggerContinueResult {
  completed: boolean;
  output?: string;
  paused: boolean;
}

export interface BackendBreakpointCapabilities {
  conditionalBreakpoints: boolean;
  hitConditionalBreakpoints: boolean;
  logPoints: boolean;
}

export async function validateLaunchConfig(config: DebuggerProcessConfig): Promise<LaunchPreflightResult> {
  const issues: LaunchPreflightIssue[] = [];
  const resolvedBinaryPath = resolveDebuggerBinaryPath(config);

  pushFileIssue(issues, 'binaryPath', resolvedBinaryPath, 'an existing soroban-debug binary', [
    'pickBinary',
    'openLaunchConfig',
    'openSettings',
    'generateLaunchConfig'
  ]);
  pushFileIssue(issues, 'contractPath', config.contractPath, 'a readable Soroban contract WASM file', [
    'pickContract',
    'openLaunchConfig',
    'generateLaunchConfig'
  ]);

  if (config.snapshotPath) {
    pushFileIssue(issues, 'snapshotPath', config.snapshotPath, 'a readable snapshot JSON file', [
      'pickSnapshot',
      'openLaunchConfig'
    ]);
  }

  if (typeof config.entrypoint !== 'undefined') {
    if (typeof config.entrypoint !== 'string' || config.entrypoint.trim().length === 0) {
      issues.push({
        field: 'entrypoint',
        message: "Launch config field 'entrypoint' must be a non-empty string.",
        expected: "A contract function name such as 'main' or 'increment'.",
        quickFixes: ['openLaunchConfig', 'generateLaunchConfig']
      });
    }
  }

  if (typeof config.args !== 'undefined') {
    const argsIssue = validateArgs(config.args);
    if (argsIssue) {
      issues.push(argsIssue);
    }
  }

  if (typeof config.port !== 'undefined') {
    if (!Number.isInteger(config.port) || config.port < 1 || config.port > 65535) {
      issues.push({
        field: 'port',
        message: `Launch config field 'port' must be an integer between 1 and 65535; received ${String(config.port)}.`,
        expected: 'A TCP port in the inclusive range 1-65535.',
        quickFixes: ['openLaunchConfig', 'openSettings']
      });
    } else {
      const portAvailable = await isPortAvailable(config.port);
      if (!portAvailable) {
        issues.push({
          field: 'port',
          message: `Launch config field 'port' uses ${config.port}, but that port is already in use.`,
          expected: 'An unused local TCP port, or omit the field to auto-select one.',
          quickFixes: ['openLaunchConfig', 'openSettings']
        });
      }
    }
  }

  if (typeof config.token !== 'undefined') {
    if (typeof config.token !== 'string' || config.token.trim().length === 0) {
      issues.push({
        field: 'token',
        message: "Launch config field 'token' must be a non-empty string when provided.",
        expected: 'A non-empty authentication token, or omit the field entirely.',
        quickFixes: ['openLaunchConfig', 'openSettings']
      });
    } else if (config.token.trim().length < 16) {
      issues.push({
        field: 'token',
        message: "Launch config field 'token' is too short for remote debugging.",
        expected: 'Use at least 16 characters, preferably a cryptographically random 32-byte token.',
        quickFixes: ['openLaunchConfig', 'openSettings']
      });
    } else if (/[\r\n]/.test(config.token)) {
      issues.push({
        field: 'token',
        message: "Launch config field 'token' cannot contain newline characters.",
        expected: 'A single-line authentication token.',
        quickFixes: ['openLaunchConfig', 'openSettings']
      });
    }
  }

  return {
    ok: issues.length === 0,
    issues,
    resolvedBinaryPath
  };
}

export function resolveDebuggerBinaryPath(config: Pick<DebuggerProcessConfig, 'binaryPath'>): string {
  if (config.binaryPath) {
    return config.binaryPath;
  }

  if (process.env.SOROBAN_DEBUG_BIN) {
    return process.env.SOROBAN_DEBUG_BIN;
  }

  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const candidates = [
    path.join(repoRoot, 'target', 'debug', process.platform === 'win32' ? 'soroban-debug.exe' : 'soroban-debug'),
    process.platform === 'win32' ? 'soroban-debug.exe' : 'soroban-debug'
  ];

  return candidates.find(candidate => fs.existsSync(candidate)) || candidates[candidates.length - 1];
}

type DebugRequest =
  | { type: 'Authenticate'; token: string }
  | { type: 'LoadContract'; contract_path: string }
  | { type: 'Execute'; function: string; args?: string }
  | { type: 'StepIn' }
  | { type: 'Next' }
  | { type: 'StepOut' }
  | { type: 'Continue' }
  | { type: 'Inspect' }
  | { type: 'GetStorage' }
  | {
      type: 'SetBreakpoint';
      id: string;
      function: string;
      condition?: string;
      hit_condition?: string;
      log_message?: string;
    }
  | { type: 'ClearBreakpoint'; id: string }
  | { type: 'Evaluate'; expression: string; frame_id?: number }
  | { type: 'Ping' }
  | { type: 'Disconnect' }
  | { type: 'LoadSnapshot'; snapshot_path: string }
  | { type: 'GetCapabilities' };

type DebugResponse =
  | { type: 'Authenticated'; success: boolean; message: string }
  | { type: 'ContractLoaded'; size: number }
  | { type: 'ExecutionResult'; success: boolean; output: string; error?: string; paused: boolean; completed: boolean }
  | { type: 'StepResult'; paused: boolean; current_function?: string; step_count: number }
  | { type: 'ContinueResult'; completed: boolean; output?: string; error?: string; paused: boolean }
  | { type: 'InspectionResult'; function?: string; args?: string; step_count: number; paused: boolean; call_stack: string[] }
  | { type: 'StorageState'; storage_json: string }
  | { type: 'SnapshotLoaded'; summary: string }
  | { type: 'BreakpointSet'; id: string; function: string }
  | { type: 'BreakpointCleared'; id: string }
  | {
      type: 'BreakpointsList';
      breakpoints: Array<{
        id: string;
        function: string;
        condition?: string;
        hit_condition?: string;
        log_message?: string;
      }>;
    }
  | { type: 'EvaluateResult'; result: string; result_type?: string; variables_reference: number }
  | {
      type: 'Capabilities';
      breakpoints: {
        conditional_breakpoints: boolean;
        hit_conditional_breakpoints: boolean;
        log_points: boolean;
      };
    }
  | { type: 'Pong' }
  | { type: 'Disconnected' }
  | { type: 'Error'; message: string };

type DebugMessage = {
  id: number;
  request?: DebugRequest;
  response?: DebugResponse;
};

type PendingRequest = {
  resolve: (response: DebugResponse) => void;
  reject: (error: Error) => void;
};

export class DebuggerProcess {
  private process: ChildProcess | null = null;
  private socket: net.Socket | null = null;
  private buffer = '';
  private requestId = 0;
  private pendingRequests = new Map<number, PendingRequest>();
  private config: DebuggerProcessConfig;
  private port: number | null = null;

  constructor(config: DebuggerProcessConfig) {
    this.config = config;
  }

  async start(): Promise<void> {
    if (this.process || this.socket) {
      return;
    }

    const binaryPath = resolveDebuggerBinaryPath(this.config);
    const port = this.config.port ?? await this.findAvailablePort();
    this.port = port;

    const child = spawn(binaryPath, this.buildArgs(port), {
      stdio: ['ignore', 'pipe', 'pipe'],
      env: {
        ...process.env,
        ...(this.config.trace ? { RUST_LOG: 'debug' } : {})
      }
    });
    this.process = child;

    child.once('exit', () => {
      this.rejectPendingRequests(new Error('Debugger server exited'));
      this.socket?.destroy();
      this.socket = null;
    });

    await this.waitForServer(port);
    await this.connect(port);

    if (this.config.token) {
      const response = await this.sendRequest({
        type: 'Authenticate',
        token: this.config.token
      });
      this.expectResponse(response, 'Authenticated');
      if (!response.success) {
        throw new Error(response.message);
      }
    }

    if (this.config.snapshotPath) {
      const response = await this.sendRequest({
        type: 'LoadSnapshot',
        snapshot_path: this.config.snapshotPath
      });
      this.expectResponse(response, 'SnapshotLoaded');
    }

    const contractResponse = await this.sendRequest({
      type: 'LoadContract',
      contract_path: this.config.contractPath
    });
    this.expectResponse(contractResponse, 'ContractLoaded');
  }

  async execute(): Promise<DebuggerExecutionResult> {
    const response = await this.sendRequest({
      type: 'Execute',
      function: this.config.entrypoint || 'main',
      args: this.config.args && this.config.args.length > 0
        ? JSON.stringify(this.config.args)
        : undefined
    });
    this.expectResponse(response, 'ExecutionResult');

    if (!response.success) {
      throw new Error(response.error || 'Execution failed');
    }

    return {
      output: response.output,
      paused: response.paused,
      completed: response.completed
    };
  }

  async stepIn(): Promise<{ paused: boolean; current_function?: string; step_count: number }> {
    const response = await this.sendRequest({ type: 'StepIn' });
    this.expectResponse(response, 'StepResult');
    return response as any;
  }

  async next(): Promise<{ paused: boolean; current_function?: string; step_count: number }> {
    const response = await this.sendRequest({ type: 'Next' });
    this.expectResponse(response, 'StepResult');
    return response as any;
  }

  async stepOut(): Promise<{ paused: boolean; current_function?: string; step_count: number }> {
    const response = await this.sendRequest({ type: 'StepOut' });
    this.expectResponse(response, 'StepResult');
    return response as any;
  }

  async continueExecution(): Promise<DebuggerContinueResult> {
    const response = await this.sendRequest({ type: 'Continue' });
    this.expectResponse(response, 'ContinueResult');
    if (response.error) {
      throw new Error(response.error);
    }
    return {
      completed: response.completed,
      output: response.output,
      paused: response.paused
    };
  }

  async inspect(): Promise<DebuggerInspection> {
    const response = await this.sendRequest({ type: 'Inspect' });
    this.expectResponse(response, 'InspectionResult');
    return {
      function: response.function,
      args: response.args,
      stepCount: response.step_count,
      paused: response.paused,
      callStack: response.call_stack
    };
  }

  async getStorage(): Promise<Record<string, unknown>> {
    const response = await this.sendRequest({ type: 'GetStorage' });
    this.expectResponse(response, 'StorageState');
    const parsed = JSON.parse(response.storage_json);
    if (parsed && typeof parsed === 'object') {
      return parsed as Record<string, unknown>;
    }
    return {};
  }

  async ping(): Promise<void> {
    const response = await this.sendRequest({ type: 'Ping' });
    this.expectResponse(response, 'Pong');
  }

  async getCapabilities(): Promise<BackendBreakpointCapabilities> {
    const response = await this.sendRequest({ type: 'GetCapabilities' });
    this.expectResponse(response, 'Capabilities');
    return {
      conditionalBreakpoints: response.breakpoints.conditional_breakpoints,
      hitConditionalBreakpoints: response.breakpoints.hit_conditional_breakpoints,
      logPoints: response.breakpoints.log_points
    };
  }

  async setBreakpoint(breakpoint: {
    id: string;
    functionName: string;
    condition?: string;
    hitCondition?: string;
    logMessage?: string;
  }): Promise<void> {
    const response = await this.sendRequest({
      type: 'SetBreakpoint',
      id: breakpoint.id,
      function: breakpoint.functionName,
      condition: breakpoint.condition,
      hit_condition: breakpoint.hitCondition,
      log_message: breakpoint.logMessage
    });
    this.expectResponse(response, 'BreakpointSet');
  }

  async clearBreakpoint(breakpointId: string): Promise<void> {
    const response = await this.sendRequest({
      type: 'ClearBreakpoint',
      id: breakpointId
    });
    this.expectResponse(response, 'BreakpointCleared');
  }

  async evaluate(expression: string, frameId?: number): Promise<{ result: string; type?: string; variablesReference: number }> {
    const response = await this.sendRequest({
      type: 'Evaluate',
      expression,
      frame_id: frameId
    });
    this.expectResponse(response, 'EvaluateResult');
    return {
      result: response.result,
      type: response.result_type,
      variablesReference: response.variables_reference
    };
  }

  async getContractFunctions(): Promise<Set<string>> {
    const binaryPath = resolveDebuggerBinaryPath(this.config);

    const output = await new Promise<string>((resolve, reject) => {
      execFile(
        binaryPath,
        ['inspect', '--contract', this.config.contractPath, '--functions'],
        { env: process.env },
        (error, stdout, stderr) => {
          if (error) {
            reject(new Error(stderr || stdout || String(error)));
            return;
          }
          resolve(stdout);
        }
      );
    });

    const functions = new Set<string>();
    for (const line of output.split(/\r?\n/)) {
      const match = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\(/);
      if (match) {
        functions.add(match[1]);
      }
    }

    return functions;
  }

  async stop(): Promise<void> {
    try {
      if (this.socket && !this.socket.destroyed) {
        await this.sendRequest({ type: 'Disconnect' }).catch(() => undefined);
      }
    } finally {
      this.socket?.destroy();
      this.socket = null;
    }

    if (!this.process) {
      return;
    }

    if (this.process.killed) {
      this.process = null;
      return;
    }

    await new Promise<void>((resolve) => {
      if (!this.process) {
        resolve();
        return;
      }

      const child = this.process;
      const timeout = setTimeout(() => {
        if (!child.killed) {
          child.kill('SIGKILL');
        }
      }, 5000);

      child.once('exit', () => {
        clearTimeout(timeout);
        resolve();
      });
      child.kill('SIGTERM');
    });

    this.process = null;
  }

  getInputStream() {
    return null;
  }

  getOutputStream() {
    return this.process?.stdout;
  }

  getErrorStream() {
    return this.process?.stderr;
  }

  private buildArgs(port: number): string[] {
    const args = ['server', '--port', String(port)];

    if (this.config.token) {
      args.push('--token', this.config.token);
    }

    return args;
  }

  isRunning(): boolean {
    return this.process !== null && this.socket !== null && !this.socket.destroyed;
  }

  private async findAvailablePort(): Promise<number> {
    return await new Promise<number>((resolve, reject) => {
      const server = net.createServer();
      server.listen(0, '127.0.0.1', () => {
        const address = server.address();
        if (!address || typeof address === 'string') {
          reject(new Error('Failed to determine an available port'));
          return;
        }

        const port = address.port;
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          resolve(port);
        });
      });
      server.on('error', reject);
    });
  }

  private async waitForServer(port: number): Promise<void> {
    const deadline = Date.now() + 10000;

    while (Date.now() < deadline) {
      if (this.process && this.process.exitCode !== null) {
        throw new Error(`Debugger server exited with code ${this.process.exitCode}`);
      }

      if (await this.canConnect(port)) {
        return;
      }

      await new Promise(resolve => setTimeout(resolve, 100));
    }

    throw new Error(`Timed out waiting for debugger server on port ${port}`);
  }

  private async canConnect(port: number): Promise<boolean> {
    return await new Promise<boolean>((resolve) => {
      const socket = net.createConnection({ host: '127.0.0.1', port }, () => {
        socket.destroy();
        resolve(true);
      });

      socket.on('error', () => {
        socket.destroy();
        resolve(false);
      });
    });
  }

  private async connect(port: number): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      const socket = net.createConnection({ host: '127.0.0.1', port }, () => {
        this.socket = socket;
        resolve();
      });

      socket.setEncoding('utf8');
      socket.on('data', (chunk: string) => {
        this.buffer += chunk;
        this.consumeMessages();
      });
      socket.on('error', reject);
      socket.on('close', () => {
        this.rejectPendingRequests(new Error('Debugger connection closed'));
        this.socket = null;
      });
    });
  }

  private consumeMessages(): void {
    while (true) {
      const newlineIndex = this.buffer.indexOf('\n');
      if (newlineIndex === -1) {
        return;
      }

      const line = this.buffer.slice(0, newlineIndex).trim();
      this.buffer = this.buffer.slice(newlineIndex + 1);

      if (!line) {
        continue;
      }

      const message = JSON.parse(line) as DebugMessage;
      const pending = this.pendingRequests.get(message.id);
      if (!pending || !message.response) {
        continue;
      }

      this.pendingRequests.delete(message.id);
      pending.resolve(message.response);
    }
  }

  private async sendRequest(request: DebugRequest): Promise<DebugResponse> {
    if (!this.socket) {
      throw new Error('Debugger connection is not established');
    }

    this.requestId += 1;
    const id = this.requestId;
    const message: DebugMessage = { id, request };

    const responsePromise = new Promise<DebugResponse>((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
    });

    this.socket.write(`${JSON.stringify(message)}\n`);
    const response = await responsePromise;
    if (response.type === 'Error') {
      throw new Error(response.message);
    }
    return response;
  }

  private rejectPendingRequests(error: Error): void {
    for (const pending of this.pendingRequests.values()) {
      pending.reject(error);
    }
    this.pendingRequests.clear();
  }

  private expectResponse<T extends DebugResponse['type']>(
    response: DebugResponse,
    type: T
  ): asserts response is Extract<DebugResponse, { type: T }> {
    if (response.type !== type) {
      throw new Error(`Unexpected debugger response: expected ${type}, got ${response.type}`);
    }
  }
}

function pushFileIssue(
  issues: LaunchPreflightIssue[],
  field: 'binaryPath' | 'contractPath' | 'snapshotPath',
  filePath: string | undefined,
  expected: string,
  quickFixes: LaunchPreflightQuickFix[]
): void {
  if (!filePath || filePath.trim().length === 0) {
    issues.push({
      field,
      message: `Launch config field '${field}' must point to ${expected}.`,
      expected,
      quickFixes
    });
    return;
  }

  if (field === 'binaryPath' && isCommandOnPath(filePath)) {
    return;
  }

  try {
    const stat = fs.statSync(filePath);
    if (!stat.isFile()) {
      issues.push({
        field,
        message: `Launch config field '${field}' points to '${filePath}', but that path is not a file.`,
        expected,
        quickFixes
      });
      return;
    }

    fs.accessSync(filePath, fs.constants.R_OK);
  } catch {
    issues.push({
      field,
      message: `Launch config field '${field}' points to '${filePath}', but the file does not exist or is not readable.`,
      expected,
      quickFixes
    });
  }
}

function isCommandOnPath(command: string): boolean {
  if (path.isAbsolute(command) || command.includes(path.sep) || command.includes('/')) {
    return false;
  }

  const pathValue = process.env.PATH;
  if (!pathValue) {
    return false;
  }

  const extensions = process.platform === 'win32'
    ? (process.env.PATHEXT || '.EXE;.CMD;.BAT;.COM')
        .split(';')
        .filter((ext) => ext.length > 0)
    : [''];

  for (const directory of pathValue.split(path.delimiter)) {
    for (const extension of extensions) {
      const candidate = path.join(directory, command.endsWith(extension) ? command : `${command}${extension}`);
      if (fs.existsSync(candidate)) {
        return true;
      }
    }
  }

  return false;
}

function validateArgs(args: unknown): LaunchPreflightIssue | undefined {
  if (!Array.isArray(args)) {
    return {
      field: 'args',
      message: `Launch config field 'args' must be an array; received ${describeValue(args)}.`,
      expected: 'A JSON array such as [] or ["alice", 10].',
      quickFixes: ['openLaunchConfig', 'generateLaunchConfig']
    };
  }

  const seen = new Set<unknown>();
  const invalidPath = findNonSerializableValue(args, '$', seen);
  if (invalidPath) {
    return {
      field: 'args',
      message: `Launch config field 'args' contains a non-JSON-serializable value at ${invalidPath}.`,
      expected: 'Only JSON-serializable values: strings, numbers, booleans, null, arrays, and objects.',
      quickFixes: ['openLaunchConfig', 'generateLaunchConfig']
    };
  }

  return undefined;
}

function findNonSerializableValue(value: unknown, pathLabel: string, seen: Set<unknown>): string | undefined {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean'
  ) {
    return undefined;
  }

  if (typeof value === 'number') {
    return Number.isFinite(value) ? undefined : pathLabel;
  }

  if (
    typeof value === 'undefined' ||
    typeof value === 'function' ||
    typeof value === 'symbol' ||
    typeof value === 'bigint'
  ) {
    return pathLabel;
  }

  if (Array.isArray(value)) {
    if (seen.has(value)) {
      return pathLabel;
    }
    seen.add(value);
    for (let index = 0; index < value.length; index += 1) {
      const found = findNonSerializableValue(value[index], `${pathLabel}[${index}]`, seen);
      if (found) {
        return found;
      }
    }
    seen.delete(value);
    return undefined;
  }

  if (typeof value === 'object') {
    const record = value as Record<string, unknown>;
    if (seen.has(record)) {
      return pathLabel;
    }
    seen.add(record);
    for (const [key, item] of Object.entries(record)) {
      const found = findNonSerializableValue(item, `${pathLabel}.${key}`, seen);
      if (found) {
        return found;
      }
    }
    seen.delete(record);
    return undefined;
  }

  return pathLabel;
}

function describeValue(value: unknown): string {
  if (Array.isArray(value)) {
    return 'an array';
  }
  if (value === null) {
    return 'null';
  }
  return typeof value;
}

async function isPortAvailable(port: number): Promise<boolean> {
  return await new Promise<boolean>((resolve) => {
    const server = net.createServer();
    server.once('error', () => {
      server.close();
      resolve(false);
    });
    server.once('listening', () => {
      server.close(() => resolve(true));
    });
    server.listen(port, '127.0.0.1');
  });
}
