# VS Code Launch Progress

The Soroban VS Code extension now surfaces launch progress in the editor UI while a debug session is starting.

## Reported phases

- `spawn`: the extension starts the `soroban-debug server` process
- `connect`: the adapter waits for the backend socket and negotiates the wire protocol
- `authenticate`: the adapter sends the optional launch token
- `load`: the adapter loads the snapshot, then the contract
- `ready`: the backend is ready and the session can move into normal debug configuration

## Failure boundaries

If launch fails, the progress notification stops on the phase that failed and the status bar text switches to a failure state. VS Code still shows the normal debug launch error, but the lifecycle boundary is visible immediately in the progress UI.
