import * as vscode from 'vscode';
import { LaunchLifecycleEvent } from './cli/debuggerProcess';
import { LAUNCH_PHASE_INCREMENT, toLaunchProgressMessage } from './launchLifecycle';

type LaunchProgressHandle = {
  currentIncrement: number;
  done: Promise<void>;
  resolveDone: () => void;
  progress?: vscode.Progress<{ message?: string; increment?: number }>;
  pendingEvents: LaunchLifecycleEvent[];
  statusBar: vscode.StatusBarItem;
};

export class SorobanLaunchProgressReporter implements vscode.Disposable {
  private handles = new Map<string, LaunchProgressHandle>();
  private disposables: vscode.Disposable[] = [];

  constructor() {
    this.disposables.push(
      vscode.debug.onDidTerminateDebugSession((session) => this.finish(session.id))
    );
  }

  createReporter(session: vscode.DebugSession): (event: LaunchLifecycleEvent) => void {
    return (event) => this.report(session, event);
  }

  dispose(): void {
    for (const handle of this.handles.values()) {
      handle.statusBar.dispose();
      handle.resolveDone();
    }
    this.handles.clear();

    for (const disposable of this.disposables) {
      disposable.dispose();
    }
    this.disposables = [];
  }

  private report(session: vscode.DebugSession, event: LaunchLifecycleEvent): void {
    const handle = this.ensureHandle(session);
    handle.pendingEvents.push(event);
    this.flush(handle);

    if (event.status === 'failed' || event.phase === 'ready') {
      this.finish(session.id);
    }
  }

  private ensureHandle(session: vscode.DebugSession): LaunchProgressHandle {
    const existing = this.handles.get(session.id);
    if (existing) {
      return existing;
    }

    let resolveDone: () => void = () => undefined;
    const done = new Promise<void>((resolve) => {
      resolveDone = resolve;
    });

    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
    statusBar.name = 'Soroban Debugger Launch';
    statusBar.show();

    const handle: LaunchProgressHandle = {
      currentIncrement: 0,
      done,
      resolveDone,
      pendingEvents: [],
      statusBar
    };

    this.handles.set(session.id, handle);
    void vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Launching ${session.name}`,
        cancellable: false
      },
      async (progress) => {
        handle.progress = progress;
        this.flush(handle);
        await handle.done;
      }
    );

    return handle;
  }

  private flush(handle: LaunchProgressHandle): void {
    if (!handle.progress) {
      return;
    }

    while (handle.pendingEvents.length > 0) {
      const event = handle.pendingEvents.shift() as LaunchLifecycleEvent;
      const target = LAUNCH_PHASE_INCREMENT[event.phase];
      const increment = event.status === 'failed'
        ? 0
        : Math.max(0, target - handle.currentIncrement);

      if (increment > 0) {
        handle.currentIncrement = Math.min(100, handle.currentIncrement + increment);
      }

      const message = toLaunchProgressMessage(event);
      handle.progress.report({ message, increment });
      handle.statusBar.text = event.status === 'failed'
        ? `$(error) Soroban launch failed`
        : event.phase === 'ready'
          ? `$(debug-start) Soroban ready`
          : `$(sync~spin) ${message}`;
      handle.statusBar.tooltip = message;
    }
  }

  private finish(sessionId: string): void {
    const handle = this.handles.get(sessionId);
    if (!handle) {
      return;
    }

    handle.resolveDone();
    handle.statusBar.dispose();
    this.handles.delete(sessionId);
  }
}
