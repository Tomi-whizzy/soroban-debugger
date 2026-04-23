# Variables View Architecture & Plan

## Current State Analysis

### `src/debug/adapter.ts` and `src/debug/logManager.ts`
1. **`src/debug/adapter.ts`**: This file contains the `SorobanDebugAdapterDescriptorFactory`. It serves as the entry point for VS Code to launch the inline Debug Adapter. It essentially injects the `LogManager` and `SorobanLaunchProgressReporter` into the core `SorobanDebugSession` (which lives in `src/dap/adapter.ts`). It doesn't handle the variables directly but bootstraps the session that does.
2. **`src/debug/logManager.ts`**: This handles extension-level logging. It manages an output channel ("Soroban Debugger") and a persistent file (`debug.log`), providing structured logging capabilities (with log phases like `Lifecycle`, `DAP`, `Backend`). It acts as a foundational observability layer for debugging interactions.

### Current Variable Mapping Logic
The variables are passed from the underlying Debugger runtime to the VS Code UI via the **Debug Adapter Protocol (DAP)** in `src/dap/adapter.ts` and `src/dap/variableStore.ts`:
1. **`scopesRequest` (`src/dap/adapter.ts`)**: Currently defines only two main scopes:
   - `Arguments` (parsed from `this.state.args`)
   - `Storage` (parsed from `this.state.storage`)
2. **`variablesRequest` (`src/dap/adapter.ts`)**: When the user expands a scope in the UI, VS Code sends a `variablesRequest` with a `variablesReference`. The adapter asks `variableStore.ts` for the children of that reference.
3. **`VariableStore` (`src/dap/variableStore.ts`)**: A custom caching engine that stores the hierarchy of objects/arrays. It takes JSON structures, formats them into DAP-compatible `Variable` items (applying truncations, specific formatting for `bytes`, `address`, and `Array` paging), and generates IDs (`variablesReference`) for nested expansion. 

Currently, **Local Variables** are not explicitly handled or grouped in a "Locals" scope. Everything relies on `args` and `storage`.

---

## Contract-Aware Variable Grouping (Implementation)

To make the variables view "contract-aware", we have introduced a more sophisticated scoping system that categorizes variables based on their context in a Soroban smart contract. The adapter now explicitly distinguishes between:

- **Locals (Local Stack)**: In-memory WASM function variables.
- **Storage (Persistent Storage)**: Persistent on-chain data associated with the contract state.

### The New `Locals` Scope
When a user pauses the debugger, the VS Code UI now surfaces a dedicated `Locals` scope alongside `Storage` and `Arguments`. This enables developers to easily separate their local in-flight function data from the persistent ledger data.

### Contract-Aware Type Parsing
This implementation is **Contract-Aware** because the `Locals` scope routes directly through the existing `VariableStore`. This means that Soroban-specific types inside locals (like `bytes`, `address`, or customized arrays/maps) benefit from the exact same intelligent type-parsing and decoding logic that the storage map uses.

> [!NOTE]
> **Technical Note:** To support this, we added `locals?: Record<string, unknown>` to the `DebuggerState` interface in `src/dap/protocol.ts`. This allows the Rust backend to send stack locals seamlessly to the TypeScript debug adapter, which then registers the scope in `src/dap/adapter.ts`.
