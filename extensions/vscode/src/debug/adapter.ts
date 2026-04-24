import * as vscode from 'vscode';
import { DebugAdapterDescriptor, DebugAdapterInlineImplementation } from 'vscode';
import { SorobanDebugSession } from '../dap/adapter';
import { LogManager } from './logManager';
import { SorobanLaunchProgressReporter } from '../launchProgress';
import { EventsTreeDataProvider } from '../eventsTree';

export class SorobanDebugAdapterDescriptorFactory
  implements vscode.DebugAdapterDescriptorFactory, vscode.Disposable {

  private context: vscode.ExtensionContext;
  private logManager: LogManager;
  private launchProgressReporter: SorobanLaunchProgressReporter;
  private eventsTreeDataProvider: EventsTreeDataProvider;
  private session: SorobanDebugSession | null = null;

  constructor(
    context: vscode.ExtensionContext,
    logManager: LogManager,
    launchProgressReporter: SorobanLaunchProgressReporter,
    eventsTreeDataProvider: EventsTreeDataProvider
  ) {
    this.context = context;
    this.logManager = logManager;
    this.launchProgressReporter = launchProgressReporter;
    this.eventsTreeDataProvider = eventsTreeDataProvider;
  }

  async createDebugAdapterDescriptor(
    session: vscode.DebugSession,
    executable: vscode.DebugAdapterExecutable | undefined
  ): Promise<DebugAdapterDescriptor | null> {
    this.session = new SorobanDebugSession(
      this.logManager,
      this.launchProgressReporter.createReporter(session),
      this.eventsTreeDataProvider
    );
    return new DebugAdapterInlineImplementation(this.session);
  }

  dispose(): void {
    this.session = null;
  }
}
