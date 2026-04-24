# Tutorial: Profile a Hotspot

This tutorial covers the `profile` command, which helps identify which parts of your contract consume the most CPU budget.

## The Goal
Find the most expensive operations in a specific execution path.

## Step 1: Run the profiler
Execute the function you want to measure using the `profile` command:

```bash
soroban-debug profile \
  --contract complex.wasm \
  --function heavy_compute \
  --args '[1000]' \
  --output profile.json
```

## Step 2: Review the summary
You can view a human-readable summary of the generated profile:

```bash
soroban-debug budget-summary --input profile.json
```

Look for operations with the highest instruction counts, such as repeated `storage::get` calls or heavy loops.

## Step 3: Optimize and compare
After modifying your Rust code to be more efficient, run the profiler again and compare the results:

```bash
soroban-debug budget-diff --before profile.json --after optimized.json
```