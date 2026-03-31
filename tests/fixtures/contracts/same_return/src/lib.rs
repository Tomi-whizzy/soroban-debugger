#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env};

#[contract]
pub struct SameReturn;

#[contractimpl]
impl SameReturn {
    // Two branches that intentionally return the same value.
    // This is used to ensure the symbolic analyzer does not dedupe paths by OK/ERR output only.
    pub fn same(env: Env, x: i64) -> i64 {
        if x > 0 {
            env.storage().instance().set(&symbol_short!("x"), &x);
            7
        } else {
            env.storage().instance().remove(&symbol_short!("x"));
            7
        }
    }
}

