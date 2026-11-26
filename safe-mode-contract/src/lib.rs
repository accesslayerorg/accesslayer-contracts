#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Mode {
    Normal = 0,
    SafeMode = 1,
    Paused = 2,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Risk = 0,
    Exit = 1,
    Admin = 2,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    Unauthorized = 2,
    SafeMode = 3,
    Paused = 4,
    Timelock = 5,
}

#[contracttype]
enum Key {
    Admin,
    Mode,
    PendingNormal(u32),
}

const NORMAL_DELAY: u32 = 100;

#[contract]
pub struct SafeModeContract;

#[contractimpl]
impl SafeModeContract {
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&Key::Admin) {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        e.storage().instance().set(&Key::Admin, &admin);
        e.storage().instance().set(&Key::Mode, &Mode::Normal);
        Ok(())
    }
    pub fn set_mode(e: Env, admin: Address, mode: Mode, reason: String) -> Result<(), Error> {
        let owner: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::NotInitialized)?;
        if owner != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        let current: Mode = e
            .storage()
            .instance()
            .get(&Key::Mode)
            .unwrap_or(Mode::Normal);
        if let (Mode::SafeMode, Mode::Normal) = (current, mode) {
            e.storage().instance().set(
                &Key::PendingNormal(e.ledger().sequence().saturating_add(NORMAL_DELAY)),
                &true,
            );
            return Err(Error::Timelock);
        }
        e.storage().instance().set(&Key::Mode, &mode);
        e.events()
            .publish((soroban_sdk::symbol_short!("mode"), admin), (mode, reason));
        Ok(())
    }
    pub fn activate_normal(e: Env, admin: Address) -> Result<(), Error> {
        let owner: Address = e
            .storage()
            .instance()
            .get(&Key::Admin)
            .ok_or(Error::NotInitialized)?;
        if owner != admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        let current: Mode = e
            .storage()
            .instance()
            .get(&Key::Mode)
            .unwrap_or(Mode::Normal);
        if current != Mode::SafeMode {
            return Ok(());
        }
        let mut ready = false;
        for i in 0..=NORMAL_DELAY {
            if e.storage()
                .instance()
                .has(&Key::PendingNormal(e.ledger().sequence().saturating_sub(i)))
            {
                ready = true;
            }
        }
        if !ready {
            return Err(Error::Timelock);
        }
        e.storage().instance().set(&Key::Mode, &Mode::Normal);
        Ok(())
    }
    pub fn require_mode(e: Env, action: Action) -> Result<(), Error> {
        let mode: Mode = e
            .storage()
            .instance()
            .get(&Key::Mode)
            .ok_or(Error::NotInitialized)?;
        match (mode, action) {
            (Mode::Paused, Action::Admin)
            | (Mode::Normal, _)
            | (Mode::SafeMode, Action::Exit)
            | (Mode::SafeMode, Action::Admin) => Ok(()),
            (Mode::SafeMode, Action::Risk) => Err(Error::SafeMode),
            (Mode::Paused, _) => Err(Error::Paused),
        }
    }
    pub fn mode(e: Env) -> Result<Mode, Error> {
        e.storage()
            .instance()
            .get(&Key::Mode)
            .ok_or(Error::NotInitialized)
    }
}
