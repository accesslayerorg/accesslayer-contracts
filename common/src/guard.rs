use soroban_sdk::{contracterror, contracttype, panic_with_error, Env};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Reentrant = 1,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Locked,
}

/// Shared temporary-storage reentrancy guard.
pub struct ReentrancyGuard {
    env: Env,
}

impl ReentrancyGuard {
    /// Marks the current contract invocation as locked, reverting if already locked.
    pub fn enter(env: &Env) -> Self {
        let storage = env.storage().temporary();
        if storage.get::<_, bool>(&DataKey::Locked).unwrap_or(false) {
            panic_with_error!(env, Error::Reentrant);
        }
        storage.set(&DataKey::Locked, &true);
        Self { env: env.clone() }
    }

    /// Clears the lock after a guarded operation finishes.
    pub fn exit(self) {
        drop(self);
    }

    /// Runs a guarded operation and clears the lock for both `Ok` and `Err` results.
    ///
    /// A trap reverts the entire Soroban invocation, including the temporary
    /// storage write performed by `enter`.
    pub fn run<T>(env: &Env, operation: impl FnOnce() -> T) -> T {
        let guard = Self::enter(env);
        let result = operation();
        guard.exit();
        result
    }
}

impl Drop for ReentrancyGuard {
    fn drop(&mut self) {
        self.env.storage().temporary().remove(&DataKey::Locked);
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ReentrancyGuard};
    use soroban_sdk::{contract, contractimpl, Env};

    #[contract]
    struct GuardHarness;

    #[contractimpl]
    impl GuardHarness {
        pub fn run_success(env: Env) -> bool {
            ReentrancyGuard::run(&env, || ());
            true
        }

        pub fn run_error(env: Env) -> bool {
            let result: Result<(), Error> = ReentrancyGuard::run(&env, || Err(Error::Reentrant));
            result.is_err()
        }

        pub fn nested(env: Env) {
            ReentrancyGuard::run(&env, || {
                let _nested = ReentrancyGuard::enter(&env);
            });
        }
    }

    #[test]
    fn guard_is_cleared_after_success_and_error_results() {
        let env = Env::default();
        let id = env.register(GuardHarness, ());
        let client = GuardHarnessClient::new(&env, &id);
        assert!(client.run_success());
        assert!(client.run_error());
        assert!(client.run_success());
    }

    #[test]
    fn nested_guard_reverts_with_reentrant_error() {
        let env = Env::default();
        let id = env.register(GuardHarness, ());
        let client = GuardHarnessClient::new(&env, &id);
        assert!(client.try_nested().is_err());
        assert!(client.run_success());
    }
}
