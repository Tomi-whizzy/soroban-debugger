# Tutorial: Review an Upgrade

This tutorial covers the `upgrade-check` command, used to safely evaluate the impact of replacing an old contract binary with a new one.

## The Goal
Ensure that a new version of your contract doesn't break existing integrations or cause unexpected execution changes.

## Step 1: Prepare the binaries
You need the currently deployed WASM and the new WASM you plan to deploy.

## Step 2: Run the upgrade check
Compare the two binaries:

```bash
soroban-debug upgrade-check --old old.wasm --new new.wasm
```

## Step 3: Interpret the results
The tool classifies the upgrade into one of three categories:
- **Safe:** No exported functions were removed, and signatures match.
- **Caution:** Non-breaking additions (like new functions) were found.
- **Breaking:** Functions were removed, or arguments changed in incompatible ways.