#![no_main]

use creator_keys::p2p_swap_dividend_stream_oracle::{
    KeySwapContract, KeySwapContractClient, SWAP_EXPIRY_SECS,
};
use creator_keys::ContractError;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

const ACCOUNT_COUNT: usize = 4;
const INITIAL_TOKEN_BALANCE: i128 = 1_000_000;

struct ProposalState {
    id: u32,
    proposer: usize,
    counterparty: usize,
    offered_token: usize,
    offered_amount: i128,
    expires_at: u64,
    active: bool,
}

fn new_token(env: &Env) -> (Address, StellarAssetClient<'_>, TokenClient<'_>) {
    let token = env.register_stellar_asset_contract_v2(Address::generate(env));
    let address = token.address();
    (
        address.clone(),
        StellarAssetClient::new(env, &address),
        TokenClient::new(env, &address),
    )
}

fn assert_escrow_invariants(
    swap: &KeySwapContractClient<'_>,
    accounts: &[Address; ACCOUNT_COUNT],
    tokens: &[TokenClient<'_>; 2],
    proposals: &[ProposalState],
) {
    for (token_index, token) in tokens.iter().enumerate() {
        let expected_reserve: i128 = proposals
            .iter()
            .filter(|proposal| proposal.active && proposal.offered_token == token_index)
            .map(|proposal| proposal.offered_amount)
            .sum();
        assert_eq!(
            token.balance(&swap.address),
            expected_reserve,
            "escrow reserve must equal the offers in all open proposals"
        );

        let total_balance: i128 = accounts
            .iter()
            .map(|account| token.balance(account))
            .sum::<i128>()
            + token.balance(&swap.address);
        assert_eq!(
            total_balance,
            INITIAL_TOKEN_BALANCE * ACCOUNT_COUNT as i128,
            "token balances including escrow must be conserved"
        );
    }
}

fuzz_target!(|input: &[u8]| {
    let env = Env::default();
    env.mock_all_auths();

    let swap_id = env.register(KeySwapContract, ());
    let swap = KeySwapContractClient::new(&env, &swap_id);
    let accounts = std::array::from_fn(|_| Address::generate(&env));
    let (token_a_id, token_a_admin, token_a) = new_token(&env);
    let (token_b_id, token_b_admin, token_b) = new_token(&env);
    let tokens = [token_a, token_b];
    let token_ids = [token_a_id, token_b_id];

    for account in &accounts {
        token_a_admin.mint(account, &INITIAL_TOKEN_BALANCE);
        token_b_admin.mint(account, &INITIAL_TOKEN_BALANCE);
    }

    let mut proposals = Vec::new();
    for step in input.chunks_exact(4).take(64) {
        let operation = step[0] % 5;
        let actor = usize::from(step[1]) % ACCOUNT_COUNT;
        let other = usize::from(step[2]) % ACCOUNT_COUNT;
        let amount = i128::from(step[3] % 8) + 1;

        match operation {
            0 if actor != other => {
                let offered_token = usize::from(step[3] % 2);
                let requested_token = 1 - offered_token;
                if let Ok(Ok(id)) = swap.try_propose_swap(
                    &accounts[actor],
                    &token_ids[offered_token],
                    &amount,
                    &token_ids[requested_token],
                    &amount,
                    &accounts[other],
                ) {
                    proposals.push(ProposalState {
                        id,
                        proposer: actor,
                        counterparty: other,
                        offered_token,
                        offered_amount: amount,
                        expires_at: env.ledger().timestamp() + SWAP_EXPIRY_SECS,
                        active: true,
                    });
                }
            }
            1 if !proposals.is_empty() => {
                let proposal_index = usize::from(step[3]) % proposals.len();
                let proposal = &mut proposals[proposal_index];
                if proposal.active
                    && swap
                        .try_accept_swap(&accounts[actor], &proposal.id)
                        .is_ok_and(|result| result.is_ok())
                {
                    proposal.active = false;
                }
            }
            2 if !proposals.is_empty() => {
                let proposal_index = usize::from(step[3]) % proposals.len();
                let proposal = &mut proposals[proposal_index];
                if proposal.active
                    && swap
                        .try_cancel_swap(&accounts[actor], &proposal.id)
                        .is_ok_and(|result| result.is_ok())
                {
                    proposal.active = false;
                }
            }
            3 => {
                env.ledger().with_mut(|ledger| {
                    ledger.timestamp = ledger.timestamp.saturating_add(SWAP_EXPIRY_SECS);
                });
            }
            4 if !proposals.is_empty() => {
                let proposal_index = usize::from(step[3]) % proposals.len();
                let proposal = &proposals[proposal_index];
                if proposal.active {
                    let challenger = (0..ACCOUNT_COUNT)
                        .map(|offset| (actor + offset) % ACCOUNT_COUNT)
                        .find(|candidate| {
                            *candidate != proposal.proposer && *candidate != proposal.counterparty
                        })
                        .expect("at least two accounts are not swap participants");
                    assert_eq!(
                        swap.try_accept_swap(&accounts[challenger], &proposal.id),
                        Err(Ok(ContractError::Unauthorized)),
                        "a non-counterparty cannot fill a proposal"
                    );
                    if env.ledger().timestamp() < proposal.expires_at {
                        assert_eq!(
                            swap.try_cancel_swap(&accounts[challenger], &proposal.id),
                            Err(Ok(ContractError::Unauthorized)),
                            "a third party cannot cancel an unexpired proposal"
                        );
                    }
                }
            }
            _ => {}
        }

        assert_escrow_invariants(&swap, &accounts, &tokens, &proposals);
    }
});
