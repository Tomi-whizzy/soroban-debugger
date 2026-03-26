import { LaunchLifecycleEvent, LaunchLifecyclePhase } from './cli/debuggerProcess';

export const LAUNCH_PHASE_INCREMENT: Record<LaunchLifecyclePhase, number> = {
  spawn: 15,
  connect: 45,
  authenticate: 65,
  load: 90,
  ready: 100
};

export function toLaunchProgressMessage(event: LaunchLifecycleEvent): string {
  const phase = event.phase.charAt(0).toUpperCase() + event.phase.slice(1);
  const prefix = event.status === 'failed'
    ? `${phase} failed`
    : event.status === 'completed'
      ? `${phase} complete`
      : phase;

  return `${prefix}: ${event.message}`;
}
