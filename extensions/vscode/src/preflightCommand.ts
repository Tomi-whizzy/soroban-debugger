import type {
  DebuggerProcessConfig,
  LaunchPreflightIssue,
  LaunchPreflightQuickFix,
  LaunchPreflightResult
} from './cli/debuggerProcess';

export type SorobanLaunchConfig = {
  name?: string;
  type?: string;
  request?: string;
} & Partial<DebuggerProcessConfig>;

export type WorkspaceFolderLike = {
  name: string;
};

export type LaunchConfigSource = {
  folder?: WorkspaceFolderLike;
  configurations: readonly unknown[] | undefined;
};

export type LaunchConfigCandidate = {
  folder?: WorkspaceFolderLike;
  config: SorobanLaunchConfig;
  label: string;
  description?: string;
  detail?: string;
};

export type LaunchPreflightCommandOutcome = 'passed' | 'failed' | 'cancelled' | 'no-config';

export interface LaunchPreflightCommandHost {
  launchConfigSources: readonly LaunchConfigSource[];
  selectLaunchConfig(candidates: readonly LaunchConfigCandidate[]): Promise<LaunchConfigCandidate | undefined>;
  validateLaunchConfig(config: SorobanLaunchConfig): Promise<LaunchPreflightResult>;
  showInformationMessage(message: string, ...actions: string[]): Promise<string | undefined>;
  showWarningMessage(message: string, ...actions: string[]): Promise<string | undefined>;
  showErrorMessage(message: string, ...actions: string[]): Promise<string | undefined>;
  applyQuickFix(quickFix: LaunchPreflightQuickFix, folder?: WorkspaceFolderLike): Promise<void>;
}

export function collectSorobanLaunchConfigs(
  sources: readonly LaunchConfigSource[]
): LaunchConfigCandidate[] {
  const includeFolderName = sources.filter((source) => source.folder).length > 1;
  const candidates: LaunchConfigCandidate[] = [];

  for (const source of sources) {
    for (const rawConfig of source.configurations ?? []) {
      if (!isSorobanLaunchConfig(rawConfig)) {
        continue;
      }

      candidates.push({
        folder: source.folder,
        config: rawConfig,
        label: rawConfig.name?.trim() || 'Soroban launch config',
        description: includeFolderName ? source.folder?.name : undefined,
        detail: rawConfig.contractPath?.trim()
      });
    }
  }

  return candidates;
}

export async function runLaunchPreflightCommand(
  host: LaunchPreflightCommandHost
): Promise<LaunchPreflightCommandOutcome> {
  const candidates = collectSorobanLaunchConfigs(host.launchConfigSources);

  if (candidates.length === 0) {
    const firstFolder = host.launchConfigSources.find((source) => source.folder)?.folder;
    const actions = firstFolder
      ? [toQuickPickLabel('generateLaunchConfig'), toQuickPickLabel('openLaunchConfig')]
      : [];
    const message = firstFolder
      ? 'No Soroban launch configurations were found. Generate one first, then rerun launch preflight.'
      : 'No Soroban launch configurations were found. Open a workspace folder or add a Soroban launch configuration, then rerun launch preflight.';
    const selected = await host.showWarningMessage(message, ...actions);
    const quickFix = fromQuickPickLabel(selected);
    if (quickFix) {
      await host.applyQuickFix(quickFix, firstFolder);
    }
    return 'no-config';
  }

  const candidate = candidates.length === 1
    ? candidates[0]
    : await host.selectLaunchConfig(candidates);

  if (!candidate) {
    return 'cancelled';
  }

  const preflight = await host.validateLaunchConfig(candidate.config);
  if (preflight.ok) {
    await host.showInformationMessage(formatPreflightSuccessMessage(candidate));
    return 'passed';
  }

  const issue = preflight.issues[0];
  const selected = await host.showErrorMessage(
    formatPreflightFailureMessage(candidate, preflight.issues),
    ...issue.quickFixes.map(toQuickPickLabel)
  );
  const quickFix = fromQuickPickLabel(selected);
  if (quickFix) {
    await host.applyQuickFix(quickFix, candidate.folder);
  }

  return 'failed';
}

export function formatPreflightSuccessMessage(candidate: LaunchConfigCandidate): string {
  return `Launch preflight passed for "${candidate.label}". The debugger backend was not started.`;
}

export function formatPreflightFailureMessage(
  candidate: LaunchConfigCandidate,
  issues: readonly LaunchPreflightIssue[]
): string {
  const headline = `Launch preflight found ${issues.length} issue${issues.length === 1 ? '' : 's'} in "${candidate.label}".`;
  const details = issues
    .slice(0, 3)
    .map((issue) => `${issue.field}: ${issue.message}`)
    .join(' ');
  const remainder = issues.length > 3
    ? ` ${issues.length - 3} more issue${issues.length - 3 === 1 ? '' : 's'} omitted.`
    : '';

  return `${headline} ${details}${remainder}`.trim();
}

export function toQuickPickLabel(quickFix: LaunchPreflightQuickFix): string {
  switch (quickFix) {
    case 'pickBinary':
      return 'Select Binary';
    case 'pickContract':
      return 'Select Contract';
    case 'pickSnapshot':
      return 'Select Snapshot';
    case 'openLaunchConfig':
      return 'Open launch.json';
    case 'generateLaunchConfig':
      return 'Generate Launch Config';
    case 'openSettings':
      return 'Open Settings';
    default:
      return quickFix;
  }
}

export function fromQuickPickLabel(label: string | undefined): LaunchPreflightQuickFix | undefined {
  switch (label) {
    case 'Select Binary':
      return 'pickBinary';
    case 'Select Contract':
      return 'pickContract';
    case 'Select Snapshot':
      return 'pickSnapshot';
    case 'Open launch.json':
      return 'openLaunchConfig';
    case 'Generate Launch Config':
      return 'generateLaunchConfig';
    case 'Open Settings':
      return 'openSettings';
    default:
      return undefined;
  }
}

function isSorobanLaunchConfig(value: unknown): value is SorobanLaunchConfig {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const config = value as Record<string, unknown>;
  return config.type === 'soroban' && config.request === 'launch';
}
