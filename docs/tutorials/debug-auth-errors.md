# How to Debug Authorization Errors

> [!WARNING]
> The `examples/contracts/auth-example` contract intentionally includes insecure `*_buggy` entrypoints for learning/debugging.
> Do **not** deploy it and do **not** copy/paste `*_buggy` functions into real contracts.
> Use the secure counterparts (`withdraw`, `admin_mint`) as your baseline patterns.

Authorization errors are one of the most common and confusing issues when developing Soroban smart contracts. This tutorial will teach you how to identify, diagnose, and fix authorization problems using the Soroban debugger.

## What Are Authorization Errors?

In Soroban, **authorization** ensures that only permitted addresses can execute certain functions. When a contract calls `address.require_auth()`, it verifies that the address has authorized the current operation.

**Common scenarios requiring authorization:**
- Transferring tokens from an account
- Modifying user-specific data
- Admin-only operations (minting, pausing, upgrades)
- Cross-contract calls with permissions

**What happens when auth fails:**
- The transaction aborts with an authorization error
- Changes are rolled back
- No events are emitted
- The error often lacks clear context about *why* auth failed

## Anatomy of an Authorization Error

When you see an auth error in the debugger, it typically looks like this:

```
✗ Execution failed

Error: HostError
  Status: Auth(InvalidAction)

Call stack:
  → wallet::withdraw
    → Address::require_auth (FAILED)
```

**Key components:**
- **Error type**: `HostError` with `Auth` status
- **Status code**: `InvalidAction`, `MissingAuth`, or `UnauthorizedFunction`
- **Location**: Which `require_auth()` call failed
- **Call stack**: The function chain leading to the failure

## Common Authorization Bugs

### Secure Reference (Copy/Paste Safe)

If you just want a **safe baseline** to start from, use this shape:

```rust
fn read_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::Unauthorized)
}

pub fn withdraw(env: Env, from: Address, amount: i128) -> Result<(), Error> {
    from.require_auth(); // check early
    // ... validate + write storage
    Ok(())
}

pub fn admin_mint(env: Env, to: Address, amount: i128) -> Result<(), Error> {
    let admin = read_admin(&env)?;
    admin.require_auth(); // verify the *admin* is authorizing
    // ... mint/write storage
    Ok(())
}
```

The rest of this section shows intentionally buggy variants (`*_buggy`) so you can learn what the debugger output looks like.

### Bug #1: Missing `require_auth()` Call

**The Problem:**
Forgetting to check authorization allows unauthorized users to perform restricted actions.

**Example (Buggy Code):**
```rust
pub fn withdraw(env: Env, from: Address, amount: i128) -> Result<(), WalletError> {
    // BUG: No authorization check!
    let balance = get_balance(env.clone(), from.clone());
    if balance < amount {
        return Err(WalletError::InsufficientBalance);
    }

    env.storage()
        .persistent()
        .set(&DataKey::Balance(from), &(balance - amount));

    Ok(())
}
```

**Why it's dangerous:**
Anyone can call `withdraw(alice_address, 1000)` and drain Alice's funds without her permission!

**Fixed Code:**
```rust
pub fn withdraw(env: Env, from: Address, amount: i128) -> Result<(), WalletError> {
    // FIXED: Require authorization from the withdrawing address
    from.require_auth();

    let balance = get_balance(env.clone(), from.clone());
    if balance < amount {
        return Err(WalletError::InsufficientBalance);
    }

    env.storage()
        .persistent()
        .set(&DataKey::Balance(from), &(balance - amount));

    Ok(())
}
```

### Bug #2: Checking Wrong Address

**The Problem:**
Verifying the wrong address for authorization.

**Example (Buggy Code):**
```rust
pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
    // BUG: Checking 'to' instead of 'from'!
    to.require_auth();

    // ... transfer logic
}
```

**Why it fails:**
The recipient (`to`) must authorize instead of the sender (`from`), which makes no sense!

**Fixed Code:**
```rust
pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
    // FIXED: Check authorization from the sender
    from.require_auth();

    // ... transfer logic
}
```

### Bug #3: Missing Admin Authorization

**The Problem:**
Admin functions that don't verify the caller is actually the admin.

**Example (Buggy Code):**
```rust
pub fn admin_mint(env: Env, to: Address, amount: i128) -> Result<(), Error> {
    let _admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::Unauthorized)?;

    // BUG: We fetched admin but didn't verify the caller IS the admin!

    mint_tokens(env, to, amount);
    Ok(())
}
```

**Why it's dangerous:**
Anyone can mint tokens because we never checked if the caller is the admin!

**Fixed Code:**
```rust
pub fn admin_mint(env: Env, to: Address, amount: i128) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::Unauthorized)?;

    // FIXED: Verify the admin is authorizing this action
    admin.require_auth();

    mint_tokens(env, to, amount);
    Ok(())
}
```

## Debugging Authorization Errors: Step-by-Step

### Step 1: Identify the Error

Run your contract and observe the error:

```bash
soroban-debug run \
  --contract examples/contracts/auth-example/target/wasm32-unknown-unknown/release/soroban_auth_example.wasm \
  --function withdraw_buggy \
  --args '[
    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFGHL",
    500
  ]'
```

**Output:**
```
✓ Execution completed successfully

⚠ WARNING: Function executed without authorization check!
  This function modified state without calling require_auth()
  Security Risk: High

Affected Function: withdraw_buggy
Storage Modified: Balance(GAAA...)
```

### Step 2: Inspect the Authorization Tree

The debugger's auth inspector shows which addresses need to authorize:

```bash
soroban-debug run \
  --contract contract.wasm \
  --function withdraw \
  --args '[...]' \
  --inspect-auth
```

**Output:**
```
Authorization Tree:
  ├─ Contract: CDEF... (withdraw)
  │   └─ Required Auth: GAAA...FGHL
  │       ├─ Status: ❌ NOT VERIFIED
  │       └─ Reason: No require_auth() call found
  └─ Result: FAILED (Missing authorization)

Recommendation: Add 'from.require_auth()' at line 45
```

**What this tells you:**
- The function needs auth from address `GAAA...FGHL`
- No authorization check was performed
- The fix: Add `require_auth()` call

### Step 3: Compare Buggy vs. Fixed Versions

Run both versions and compare auth behavior:

**Buggy version:**
```bash
soroban-debug run \
  --contract contract.wasm \
  --function withdraw_buggy \
  --args '["GAAAA...", 500]' \
  --trace-output buggy_trace.json
```

**Fixed version:**
```bash
soroban-debug run \
  --contract contract.wasm \
  --function withdraw \
  --args '["GAAAA...", 500]' \
  --trace-output fixed_trace.json
```

**Compare traces:**
```bash
soroban-debug compare buggy_trace.json fixed_trace.json
```

**Output:**
```diff
Authorization Differences:

- withdraw_buggy:
    ❌ No authorization checks performed
    ⚠ Security risk: State modified without permission

+ withdraw:
    ✓ Address GAAA... required authorization
    ✓ Auth check passed at line 52
    ✓ Secure: State modification authorized
```

### Step 4: Use Interactive Mode for Deep Inspection

Set breakpoints and inspect auth state:

```bash
soroban-debug interactive \
  --contract contract.wasm

> break withdraw
> run withdraw GAAAA... 500
Breakpoint hit: withdraw

> auth
Authorization Status:
  Required: GAAAA...FGHL
  Verified: false

> step
> step
> auth
Authorization Status:
  Required: GAAAA...FGHL
  Verified: true ✓
  Verified at: line 52 (from.require_auth())
```

## Real-World Example: Debugging a Wallet Contract

Let's debug the example wallet contract that has intentional auth bugs.

### Build the Contract

```bash
cd examples/contracts/auth-example
cargo build --target wasm32-unknown-unknown --release
```

### Test the Buggy Withdraw

```bash
soroban-debug run \
  --contract target/wasm32-unknown-unknown/release/soroban_auth_example.wasm \
  --function withdraw_buggy \
  --args '[
    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFGHL",
    500
  ]'
```

**Output:**
```
✓ Execution completed

⚠ SECURITY WARNING: Authorization bypass detected!

Function: withdraw_buggy
Issue: State modification without authorization
Affected Storage:
  - Balance(GAAA...FGHL): 1000 → 500

Risk Assessment: CRITICAL
  ❌ Anyone can withdraw from any account
  ❌ No permission checks performed
  ❌ Vulnerable to unauthorized fund drainage

Recommendation:
  Add authorization check before line 47:
    from.require_auth();
```

### Test the Fixed Withdraw

```bash
soroban-debug run \
  --contract target/wasm32-unknown-unknown/release/soroban_auth_example.wasm \
  --function withdraw \
  --args '[
    "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFGHL",
    500
  ]'
```

**Output:**
```
✓ Execution completed successfully

Authorization Verified:
  ✓ Address GAAA...FGHL authorized withdrawal
  ✓ Permission check at line 52
  ✓ Secure state modification

Storage Changes:
  Balance(GAAA...FGHL): 1000 → 500

Security Status: ✓ SECURE
```

## Authorization Error Patterns

### Pattern 1: InvalidAction

**What it means:** The address didn't sign the transaction

**Common causes:**
- Calling from wrong address
- Missing signature in transaction
- Testing without `mock_all_auths()`

**How to fix:**
```rust
// In tests, use:
env.mock_all_auths();

// Or specifically mock one address:
env.mock_auths(&[MockAuth {
    address: &user,
    invoke: &MockAuthInvoke {
        contract: &contract_id,
        fn_name: "withdraw",
        args: (user.clone(), 100i128).into_val(&env),
        sub_invokes: &[],
    },
}]);
```

### Pattern 2: MissingAuth

**What it means:** No authorization context provided

**Common causes:**
- Cross-contract call missing auth
- Invoker didn't propagate auth

**How to fix:**
```rust
// When calling another contract, use require_auth_for_args:
client.transfer(&from, &to, &amount);
```

### Pattern 3: UnauthorizedFunction

**What it means:** The function isn't exported or doesn't exist

**Common causes:**
- Typo in function name
- Function not marked with `#[contractimpl]`
- Wrong contract address

**How to fix:**
- Use `soroban-debug inspect --contract <WASM>` to list all exported functions and verify the name.
- Ensure the function is marked with `#[contractimpl]` and is public.
- Double-check the contract address if you are using multiple contracts in your environment.

## Testing Authorization

### Unit Test Template

```rust
#[test]
fn test_authorization_required() {
    let env = Env::default();
    // DON'T use mock_all_auths to test real auth behavior

    let contract_id = env.register(MyContract, ());
    let client = MyContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    // This should fail without authorization
    let result = client.try_withdraw(&user, &100);
    assert!(result.is_err());

    // Now mock auth and try again
    env.mock_all_auths();
    let result = client.try_withdraw(&user, &100);
    assert!(result.is_ok());
}
```

### Integration Test with Debugger

```bash
# Test that auth is actually required
soroban-debug run \
  --contract contract.wasm \
  --function protected_function \
  --args '[...]' \
  --expect-error "Auth"
```

## Authorization Checklist

Before deploying your contract, verify:

- [ ] All state-modifying functions check authorization
- [ ] The correct address is verified (sender, not recipient)
- [ ] Admin functions verify caller is admin
- [ ] Cross-contract calls propagate authorization
- [ ] Tests verify auth is actually enforced (don't just use `mock_all_auths()`)
- [ ] Error messages are clear about which auth failed

## Best Practices

1. ✅ **Always require auth for state changes**: If it modifies storage, it needs `require_auth()`
2. ✅ **Fail closed**: Require auth by default, allow public access explicitly
3. ✅ **Check early**: Call `require_auth()` at the beginning of functions
4. ✅ **Use descriptive errors**: Create custom error types for auth failures
5. ✅ **Test without mocking**: Write tests that verify auth is actually enforced
6. ✅ **Document auth requirements**: Comment which addresses need to authorize
7. ✅ **Use the debugger**: Catch auth bugs before deployment

## Common Mistakes to Avoid

❌ **Don't:** Use `mock_all_auths()` in all tests
✅ **Do:** Test both with and without auth to verify it's enforced

❌ **Don't:** Skip auth checks for "convenience functions"
✅ **Do:** Require auth for any function that changes state

❌ **Don't:** Assume the caller is trustworthy
✅ **Do:** Always verify authorization explicitly

❌ **Don't:** Check if admin exists without verifying caller is admin
✅ **Do:** Both fetch admin AND call `admin.require_auth()`

## Additional Resources

- [Soroban Authorization Guide](https://soroban.stellar.org/docs/learn/authorization)
- [Example Auth Contract](../../examples/contracts/auth-example/)
- [Debugger FAQ - Auth Errors](../faq.md)

## Summary

Authorization errors are preventable with proper tooling and testing:

- **Use the debugger** to detect missing `require_auth()` calls
- **Compare traces** to see before/after auth fixes
- **Test without `mock_all_auths()`** to verify enforcement
- **Check the right address** (sender, not recipient; caller is admin, not just admin exists)
- **Inspect the auth tree** to understand which addresses need to authorize

With the Soroban debugger's auth inspector, you can catch these bugs during development instead of discovering them in production!
