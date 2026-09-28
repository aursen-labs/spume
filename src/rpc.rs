//! Every RPC method as a transport-free builder.
//!
//! Each function here mirrors the [`WasmClient`](crate::WasmClient) method of
//! the same name — same arguments, same result type — but returns a
//! [`Call`] instead of performing the request, so a host that owns its own
//! transport still gets typed params and a typed response:
//!
//! ```
//! # fn main() -> Result<(), Box<solana_rpc_client_types::request::RpcError>> {
//! let call = spume::rpc::get_slot(None)?;
//! let body = call.body(1);
//! # let (bytes, status) = (br#"{"jsonrpc":"2.0","id":1,"result":42}"#, 200);
//! let slot: u64 = call.parse(bytes, status)?;
//! # Ok(()) }
//! ```
//!
//! Both this module and the client methods are generated from one table, so
//! they cannot drift apart.

#[cfg(feature = "check_address")]
use std::str::FromStr;
use {
    crate::codec::Call,
    serde_json::json,
    solana_address::Address,
    solana_epoch_info::EpochInfo,
    solana_epoch_schedule::EpochSchedule,
    solana_rpc_client_types::{
        config::{
            CommitmentConfig, RpcAccountInfoConfig, RpcBlockConfig, RpcBlockProductionConfig,
            RpcContextConfig, RpcEncodingConfigWrapper, RpcEpochConfig, RpcGetVoteAccountsConfig,
            RpcLargestAccountsConfig, RpcLeaderScheduleConfig, RpcProgramAccountsConfig,
            RpcRequestAirdropConfig, RpcSendTransactionConfig, RpcSignatureStatusConfig,
            RpcSignaturesForAddressConfig, RpcSimulateTransactionConfig, RpcSupplyConfig,
            RpcTokenAccountsFilter, RpcTransactionConfig,
        },
        request::{RpcError, RpcRequest},
        response::{
            OptionalContext, Response, RpcAccountBalance, RpcBlockCommitment, RpcBlockProduction,
            RpcBlockhash, RpcConfirmedTransactionStatusWithSignature, RpcContactInfo, RpcIdentity,
            RpcInflationGovernor, RpcInflationRate, RpcInflationReward, RpcKeyedAccount,
            RpcLeaderSchedule, RpcPerfSample, RpcPrioritizationFee, RpcSimulateTransactionResult,
            RpcSnapshotSlotInfo, RpcSupply, RpcTokenAccountBalance, RpcVersionInfo,
            RpcVoteAccountStatus, UiAccount, UiConfirmedBlock, UiTokenAmount,
        },
    },
    solana_transaction_status_client_types::{
        EncodedConfirmedTransactionWithStatusMeta, TransactionStatus,
    },
    std::borrow::Cow,
};

type RpcResult<T> = Result<T, Box<RpcError>>;

/// Expand the method table twice: once into transport-free builders in this
/// module, once into `async` methods on [`WasmClient`](crate::WasmClient).
///
/// One entry per RPC method — signature, result type, request, params — so the
/// two views are the same list by construction.
macro_rules! rpc_methods {
    ($(
        $(#[$meta:meta])*
        fn $name:ident $(<$gen:ident: $bound:ident>)? ($($arg:ident: $ty:ty),* $(,)?)
            -> $ret:ty = $request:expr, $params:expr;
    )*) => {
        $(
            $(#[$meta])*
            pub fn $name $(<$gen: $bound>)? ($($arg: $ty),*) -> RpcResult<Call<$ret>> {
                Ok(Call::new($request, $params))
            }
        )*

        #[cfg(feature = "http")]
        impl crate::WasmClient {
            $(
                $(#[$meta])*
                pub async fn $name $(<$gen: $bound>)? (&self, $($arg: $ty),*) -> RpcResult<$ret> {
                    self.provider.send($request, $params).await
                }
            )*
        }
    };
}

rpc_methods! {
    /// Fetch all information associated with the account of the given address.
    fn get_account_info(address: impl CheckAddress, config: Option<RpcAccountInfoConfig>)
        -> Response<Option<UiAccount>>
        = RpcRequest::GetAccountInfo, json!([address.parse()?, config]);

    /// Fetch the lamport balance of an account.
    fn get_balance(address: impl CheckAddress, config: Option<RpcContextConfig>)
        -> Response<u64>
        = RpcRequest::GetBalance, json!([address.parse()?, config]);

    /// Return the 20 largest accounts by lamport balance.
    fn get_largest_accounts(config: Option<RpcLargestAccountsConfig>)
        -> Response<Vec<RpcAccountBalance>>
        = RpcRequest::GetLargestAccounts, json!([config]);

    /// Return the minimum balance required to make an account of `data_size` rent-exempt.
    fn get_minimum_balance_for_rent_exemption(data_size: u64, config: Option<CommitmentConfig>)
        -> u64
        = RpcRequest::GetMinimumBalanceForRentExemption, json!([data_size, config]);

    /// Fetch info for several accounts in a single request.
    fn get_multiple_accounts<T: CheckAddress>(addresses: &[T], config: Option<RpcAccountInfoConfig>)
        -> Response<Vec<Option<UiAccount>>>
        = RpcRequest::GetMultipleAccounts, {
            let addresses: Vec<Cow<'_, str>> = addresses
                .iter()
                .map(CheckAddress::parse)
                .collect::<RpcResult<_>>()?;
            json!([addresses, config])
        };

    /// Fetch all accounts owned by the given program, optionally filtered.
    fn get_program_accounts(program_id: impl CheckAddress, config: Option<RpcProgramAccountsConfig>)
        -> OptionalContext<Vec<RpcKeyedAccount>>
        = RpcRequest::GetProgramAccounts, json!([program_id.parse()?, config]);

    /// Return the SPL token balance held by a token account.
    fn get_token_account_balance(account: impl CheckAddress, config: Option<RpcContextConfig>)
        -> Response<UiTokenAmount>
        = RpcRequest::GetTokenAccountBalance, json!([account.parse()?, config]);

    /// Fetch SPL token accounts delegated to the given address.
    fn get_token_accounts_by_delegate(
        delegate: impl CheckAddress,
        filter: RpcTokenAccountsFilter,
        config: Option<RpcAccountInfoConfig>,
    )
        -> Response<Vec<RpcKeyedAccount>>
        = RpcRequest::GetTokenAccountsByDelegate, json!([delegate.parse()?, filter, config]);

    /// Fetch SPL token accounts owned by the given address.
    fn get_token_accounts_by_owner(
        owner: impl CheckAddress,
        filter: RpcTokenAccountsFilter,
        config: Option<RpcAccountInfoConfig>,
    )
        -> Response<Vec<RpcKeyedAccount>>
        = RpcRequest::GetTokenAccountsByOwner, json!([owner.parse()?, filter, config]);

    /// Return the 20 largest token accounts for a given mint.
    fn get_token_largest_accounts(mint: impl CheckAddress, config: Option<CommitmentConfig>)
        -> Response<Vec<RpcTokenAccountBalance>>
        = RpcRequest::GetTokenLargestAccounts, json!([mint.parse()?, config]);

    /// Return the total supply of an SPL token mint.
    fn get_token_supply(mint: impl CheckAddress, config: Option<CommitmentConfig>)
        -> Response<UiTokenAmount>
        = RpcRequest::GetTokenSupply, json!([mint.parse()?, config]);

    /// Return the fee the cluster would charge to process a base64-encoded transaction message.
    fn get_fee_for_message(message: String, config: Option<RpcContextConfig>)
        -> Response<Option<u64>>
        = RpcRequest::GetFeeForMessage, json!([message, config]);

    /// Fetch the latest blockhash and the last block height at which it is still valid.
    fn get_latest_blockhash(config: Option<RpcContextConfig>)
        -> Response<RpcBlockhash>
        = RpcRequest::GetLatestBlockhash, json!([config]);

    /// Return recent per-slot prioritization fees, optionally restricted to accounts that must be
    /// writable.
    fn get_recent_prioritization_fees(addresses: Option<&[Address]>)
        -> Vec<RpcPrioritizationFee>
        // Omit the positional entirely when `None` — the RPC rejects `[null]`.
        = RpcRequest::GetRecentPrioritizationFees, match addresses {
            Some(addresses) => {
                let addresses: Vec<String> = addresses.iter().map(Address::to_string).collect();
                json!([addresses])
            }
            None => json!([]),
        };

    /// Return confirmed transaction signatures involving the given address, most recent first.
    fn get_signatures_for_address(
        address: impl CheckAddress,
        config: Option<RpcSignaturesForAddressConfig>,
    )
        -> Vec<RpcConfirmedTransactionStatusWithSignature>
        = RpcRequest::GetSignaturesForAddress, json!([address.parse()?, config]);

    /// Return the processing status of one or more transaction signatures.
    fn get_signature_statuses(signatures: Vec<String>, config: Option<RpcSignatureStatusConfig>)
        -> Response<Vec<Option<TransactionStatus>>>
        = RpcRequest::GetSignatureStatuses, json!([signatures, config]);

    /// Fetch a confirmed transaction by its signature.
    fn get_transaction(
        signature: String,
        config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
    )
        -> Option<EncodedConfirmedTransactionWithStatusMeta>
        = RpcRequest::GetTransaction, json!([signature, config]);

    /// Return the cumulative number of transactions processed by the cluster.
    fn get_transaction_count(config: Option<RpcContextConfig>)
        -> u64
        = RpcRequest::GetTransactionCount, json!([config]);

    /// Report whether the given blockhash is still valid.
    fn is_blockhash_valid(blockhash: String, config: Option<RpcContextConfig>)
        -> Response<bool>
        = RpcRequest::IsBlockhashValid, json!([blockhash, config]);

    /// Request an airdrop of lamports to an address (devnet/testnet only). Returns the signature.
    fn request_airdrop(
        address: impl CheckAddress,
        lamports: u64,
        config: Option<RpcRequestAirdropConfig>,
    )
        -> String
        = RpcRequest::RequestAirdrop, json!([address.parse()?, lamports, config]);

    /// Submit a signed transaction. Does not wait for confirmation; returns the signature.
    fn send_transaction(transaction: String, config: Option<RpcSendTransactionConfig>)
        -> String
        = RpcRequest::SendTransaction, json!([transaction, config]);

    /// Simulate a transaction without submitting it. Returns logs, accounts, and any error.
    fn simulate_transaction(transaction: String, config: Option<RpcSimulateTransactionConfig>)
        -> Response<RpcSimulateTransactionResult>
        = RpcRequest::SimulateTransaction, json!([transaction, config]);

    /// Fetch a confirmed block by slot.
    fn get_block(slot: u64, config: Option<RpcEncodingConfigWrapper<RpcBlockConfig>>)
        -> UiConfirmedBlock
        = RpcRequest::GetBlock, json!([slot, config]);

    /// Return per-stake vote commitment for a block at the given slot.
    fn get_block_commitment(slot: u64)
        -> RpcBlockCommitment<Vec<usize>>
        = RpcRequest::Custom { method: "getBlockCommitment" }, json!([slot]);

    /// Return the current block height of the node.
    fn get_block_height(config: Option<RpcContextConfig>)
        -> u64
        = RpcRequest::GetBlockHeight, json!([config]);

    /// Return recent block production information, broken down by validator identity.
    fn get_block_production(config: Option<RpcBlockProductionConfig>)
        -> Response<RpcBlockProduction>
        = RpcRequest::GetBlockProduction, json!([config]);

    /// List confirmed blocks in the inclusive slot range `[start_slot, end_slot]`.
    fn get_blocks(start_slot: u64, end_slot: Option<u64>, config: Option<RpcContextConfig>)
        -> Vec<u64>
        = RpcRequest::GetBlocks, json!([start_slot, end_slot, config]);

    /// List up to `limit` confirmed blocks starting at `start_slot`.
    fn get_blocks_with_limit(start_slot: u64, limit: u64, config: Option<RpcContextConfig>)
        -> Vec<u64>
        = RpcRequest::GetBlocksWithLimit, json!([start_slot, limit, config]);

    /// Return the estimated UNIX production timestamp of a block, if available.
    fn get_block_time(slot: u64)
        -> Option<i64>
        = RpcRequest::GetBlockTime, json!([slot]);

    /// Return the slot of the lowest confirmed block still retained by the node.
    fn get_first_available_block()
        -> u64
        = RpcRequest::GetFirstAvailableBlock, json!([]);

    /// Return recent slot-time performance samples, up to `limit` (default 720).
    fn get_recent_performance_samples(limit: Option<u32>)
        -> Vec<RpcPerfSample>
        // Omit the positional entirely when `None` — the RPC rejects `[null]`.
        = RpcRequest::GetRecentPerformanceSamples, match limit {
            Some(n) => json!([n]),
            None => json!([]),
        };

    /// Return the lowest slot the node has information about.
    fn minimum_ledger_slot()
        -> u64
        = RpcRequest::MinimumLedgerSlot, json!([]);

    /// Return information about all known cluster nodes.
    fn get_cluster_nodes()
        -> Vec<RpcContactInfo>
        = RpcRequest::GetClusterNodes, json!([]);

    /// Return information about the current epoch.
    fn get_epoch_info(config: Option<RpcEpochConfig>)
        -> EpochInfo
        = RpcRequest::GetEpochInfo, json!([config]);

    /// Return the cluster's epoch schedule parameters from the genesis config.
    fn get_epoch_schedule()
        -> EpochSchedule
        = RpcRequest::GetEpochSchedule, json!([]);

    /// Return the cluster's genesis hash.
    fn get_genesis_hash()
        -> String
        = RpcRequest::GetGenesisHash, json!([]);

    /// Return `"ok"` when the node is caught up with its peers.
    fn get_health()
        -> String
        = RpcRequest::GetHealth, json!([]);

    /// Return the highest full (and optional incremental) snapshot slot the node has stored.
    fn get_highest_snapshot_slot()
        -> RpcSnapshotSlotInfo
        = RpcRequest::GetHighestSnapshotSlot, json!([]);

    /// Return the node's identity pubkey.
    fn get_identity()
        -> RpcIdentity
        = RpcRequest::GetIdentity, json!([]);

    /// Return the leader schedule for the epoch containing `slot` (or the current epoch).
    fn get_leader_schedule(slot: Option<u64>, config: Option<RpcLeaderScheduleConfig>)
        -> Option<RpcLeaderSchedule>
        = RpcRequest::GetLeaderSchedule, json!([slot, config]);

    /// Return the highest slot seen via the retransmit stage.
    fn get_max_retransmit_slot()
        -> u64
        = RpcRequest::GetMaxRetransmitSlot, json!([]);

    /// Return the highest slot for which shreds have been inserted.
    fn get_max_shred_insert_slot()
        -> u64
        = RpcRequest::GetMaxShredInsertSlot, json!([]);

    /// Return the slot the node is currently processing.
    fn get_slot(config: Option<RpcContextConfig>)
        -> u64
        = RpcRequest::GetSlot, json!([config]);

    /// Return the identity of the current slot leader.
    fn get_slot_leader(config: Option<RpcContextConfig>)
        -> String
        = RpcRequest::GetSlotLeader, json!([config]);

    /// Return the slot leaders for the half-open range `[start_slot, start_slot + limit)`.
    fn get_slot_leaders(start_slot: u64, limit: u64)
        -> Vec<String>
        = RpcRequest::GetSlotLeaders, json!([start_slot, limit]);

    /// Return the node's software version.
    fn get_version()
        -> RpcVersionInfo
        = RpcRequest::GetVersion, json!([]);

    /// Return the current and delinquent vote accounts.
    fn get_vote_accounts(config: Option<RpcGetVoteAccountsConfig>)
        -> RpcVoteAccountStatus
        = RpcRequest::GetVoteAccounts, json!([config]);

    /// Return the cluster's current inflation governor.
    fn get_inflation_governor(config: Option<CommitmentConfig>)
        -> RpcInflationGovernor
        = RpcRequest::GetInflationGovernor, json!([config]);

    /// Return the specific inflation values for the current epoch.
    fn get_inflation_rate()
        -> RpcInflationRate
        = RpcRequest::GetInflationRate, json!([]);

    /// Return inflation rewards earned by a list of addresses during an epoch.
    fn get_inflation_reward(addresses: &[Address], config: Option<RpcEpochConfig>)
        -> Vec<Option<RpcInflationReward>>
        = RpcRequest::GetInflationReward, {
            let addresses: Vec<String> = addresses.iter().map(Address::to_string).collect();
            json!([addresses, config])
        };

    /// Return the stake-program minimum delegation in lamports.
    fn get_stake_minimum_delegation(config: Option<CommitmentConfig>)
        -> Response<u64>
        = RpcRequest::GetStakeMinimumDelegation, json!([config]);

    /// Return information about the cluster's circulating and non-circulating supply.
    fn get_supply(config: Option<RpcSupplyConfig>)
        -> Response<RpcSupply>
        = RpcRequest::GetSupply, json!([config]);
}

/// When feature `check_address` is enabled, the address is parsed and an
/// error is returned if it is not a valid address. [CheckAddress] is
/// implemented for `&str`, `&String`, `String` and `Cow<'_, str>`, so those
/// types work automatically.
pub trait CheckAddress {
    fn parse(&self) -> RpcResult<Cow<'_, str>>;
}

macro_rules! impl_check_address_str {
    ($($t:ty),*) => {$(
        impl CheckAddress for $t {
            #[cfg(not(feature = "check_address"))]
            fn parse(&self) -> RpcResult<Cow<'_, str>> {
                let s: &str = self.as_ref();
                Ok(s.into())
            }

            #[cfg(feature = "check_address")]
            fn parse(&self) -> RpcResult<Cow<'_, str>> {
                let s: &str = self.as_ref();
                Address::from_str(s).map_err(|err| {
                    Box::new(RpcError::ParseError(format!("invalid address {s:?}: {err}")))
                })?;
                Ok(s.into())
            }
        }
    )*};
}

impl_check_address_str!(&str, &String, String, Cow<'_, str>);

impl CheckAddress for &Address {
    fn parse(&self) -> RpcResult<Cow<'_, str>> {
        Ok(self.to_string().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_builder_and_its_client_method_agree() {
        let call = get_balance("11111111111111111111111111111111", None).expect("valid address");
        let body: serde_json::Value = serde_json::from_str(&call.body(1)).expect("valid json");

        assert_eq!(call.method(), "getBalance");
        assert_eq!(body["method"], "getBalance");
        assert_eq!(body["params"][0], "11111111111111111111111111111111");
    }

    #[test]
    fn a_call_parses_into_its_own_result_type() {
        let call = get_balance("11111111111111111111111111111111", None).expect("valid address");
        let body = br#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":42}}"#;

        // Typed by the builder — the caller never names `Response<u64>`.
        assert_eq!(call.parse(body, 200).expect("balance").value, 42);
    }

    #[cfg(feature = "check_address")]
    #[test]
    fn a_bad_address_fails_before_any_request_is_built() {
        let err = get_balance("not-an-address", None).expect_err("malformed address");
        let msg = err.to_string();

        // The offending input and the reason it was rejected both survive.
        assert!(msg.contains("not-an-address"), "unexpected error: {msg}");
        assert!(
            msg.contains(&Address::from_str("not-an-address").unwrap_err().to_string()),
            "underlying parse error dropped: {msg}"
        );
    }

    #[test]
    fn an_omitted_optional_positional_is_dropped_entirely() {
        // The RPC rejects `[null]`, so `None` must produce `[]`.
        let call = get_recent_performance_samples(None).expect("no params");
        let body: serde_json::Value = serde_json::from_str(&call.body(1)).expect("valid json");

        assert_eq!(body["params"], json!([]));
    }
}
