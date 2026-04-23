export const WIRE_PROTOCOL_MIN_VERSION = 1;
export const WIRE_PROTOCOL_MAX_VERSION = 1;

export interface BreakpointLocation {
  id: string;
  source: string;
  line: number;
  column?: number;
  functionName?: string;
  condition?: string;
  hitCondition?: string;
  logMessage?: string;
}

export interface StackFrame {
  id: number;
  name: string;
  source: string;
  line: number;
  column: number;
  instructionPointerReference?: string;
}

export interface Variable {
  name: string;
  value: string;
  type?: string;
  variablesReference?: number;
  indexedVariables?: number;
  namedVariables?: number;
}

export interface Scope {
  name: string;
  variablesReference: number;
  expensive: boolean;
  source?: {
    name: string;
    path: string;
  };
  line?: number;
  column?: number;
  endLine?: number;
  endColumn?: number;
}

export interface Thread {
  id: number;
  name: string;
}

export interface StoppedEvent {
  reason: 'breakpoint' | 'step' | 'exception' | 'pause' | 'entry' | 'goto' | 'function breakpoint' | 'instruction breakpoint' | 'other';
  threadId: number;
  allThreadsStopped?: boolean;
  description?: string;
  text?: string;
  preserveFocusWhenOpen?: boolean;
}

export type DebugProtocolMessage = {
  type: 'request' | 'response' | 'event';
  seq: number;
  command?: string;
};

export interface DebuggerState {
  isRunning: boolean;
  isPaused: boolean;
  currentThread?: number;
  breakpoints: Map<string, BreakpointLocation[]>;
  callStack?: StackFrame[];
  storage?: Record<string, unknown>;
  args?: string;
  locals?: Record<string, unknown>;
}

export interface BreakpointCapabilities {
  conditionalBreakpoints: boolean;
  hitConditionalBreakpoints: boolean;
  logPoints: boolean;
}
