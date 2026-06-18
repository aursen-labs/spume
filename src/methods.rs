use {
    crate::{BatchHandle, BatchRequest, RpcMethod, WasmClient},
    serde_json::{Value, json},
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
};

type RpcResult<T> = Result<T, Box<RpcError>>;

// ── getAccountInfo ──────────────────────────────────────────────────────────

pub struct GetAccountInfo<'a> {
    pub address: &'a Address,
    pub config: Option<RpcAccountInfoConfig>,
}

impl RpcMethod for GetAccountInfo<'_> {
    type Output = Response<Option<UiAccount>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetAccountInfo
    }
    fn params(&self) -> Value {
        json!([self.address.to_string(), self.config])
    }
}

impl WasmClient {
    /// Fetch all information associated with the account of the given address.
    pub async fn get_account_info(
        &self,
        address: &Address,
        config: Option<RpcAccountInfoConfig>,
    ) -> RpcResult<Response<Option<UiAccount>>> {
        self.call(GetAccountInfo { address, config }).await
    }
}

impl BatchRequest {
    /// Fetch all information associated with the account of the given address.
    pub fn get_account_info(
        &mut self,
        address: &Address,
        config: Option<RpcAccountInfoConfig>,
    ) -> BatchHandle<Response<Option<UiAccount>>> {
        self.add(GetAccountInfo { address, config })
    }
}

// ── getBalance ──────────────────────────────────────────────────────────────

pub struct GetBalance<'a> {
    pub address: &'a Address,
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetBalance<'_> {
    type Output = Response<u64>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBalance
    }
    fn params(&self) -> Value {
        json!([self.address.to_string(), self.config])
    }
}

impl WasmClient {
    /// Fetch the lamport balance of an account.
    pub async fn get_balance(
        &self,
        address: &Address,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Response<u64>> {
        self.call(GetBalance { address, config }).await
    }
}

impl BatchRequest {
    /// Fetch the lamport balance of an account.
    pub fn get_balance(
        &mut self,
        address: &Address,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Response<u64>> {
        self.add(GetBalance { address, config })
    }
}

// ── getLargestAccounts ──────────────────────────────────────────────────────

pub struct GetLargestAccounts {
    pub config: Option<RpcLargestAccountsConfig>,
}

impl RpcMethod for GetLargestAccounts {
    type Output = Response<Vec<RpcAccountBalance>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetLargestAccounts
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the 20 largest accounts by lamport balance.
    pub async fn get_largest_accounts(
        &self,
        config: Option<RpcLargestAccountsConfig>,
    ) -> RpcResult<Response<Vec<RpcAccountBalance>>> {
        self.call(GetLargestAccounts { config }).await
    }
}

impl BatchRequest {
    /// Return the 20 largest accounts by lamport balance.
    pub fn get_largest_accounts(
        &mut self,
        config: Option<RpcLargestAccountsConfig>,
    ) -> BatchHandle<Response<Vec<RpcAccountBalance>>> {
        self.add(GetLargestAccounts { config })
    }
}

// ── getMinimumBalanceForRentExemption ───────────────────────────────────────

pub struct GetMinimumBalanceForRentExemption {
    pub data_size: u64,
    pub config: Option<CommitmentConfig>,
}

impl RpcMethod for GetMinimumBalanceForRentExemption {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetMinimumBalanceForRentExemption
    }
    fn params(&self) -> Value {
        json!([self.data_size, self.config])
    }
}

impl WasmClient {
    /// Return the minimum balance required to make an account of `data_size` rent-exempt.
    pub async fn get_minimum_balance_for_rent_exemption(
        &self,
        data_size: u64,
        config: Option<CommitmentConfig>,
    ) -> RpcResult<u64> {
        self.call(GetMinimumBalanceForRentExemption { data_size, config })
            .await
    }
}

impl BatchRequest {
    /// Return the minimum balance required to make an account of `data_size` rent-exempt.
    pub fn get_minimum_balance_for_rent_exemption(
        &mut self,
        data_size: u64,
        config: Option<CommitmentConfig>,
    ) -> BatchHandle<u64> {
        self.add(GetMinimumBalanceForRentExemption { data_size, config })
    }
}

// ── getMultipleAccounts ─────────────────────────────────────────────────────

pub struct GetMultipleAccounts<'a> {
    pub addresses: &'a [Address],
    pub config: Option<RpcAccountInfoConfig>,
}

impl RpcMethod for GetMultipleAccounts<'_> {
    type Output = Response<Vec<Option<UiAccount>>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetMultipleAccounts
    }
    fn params(&self) -> Value {
        let addresses: Vec<String> = self.addresses.iter().map(Address::to_string).collect();
        json!([addresses, self.config])
    }
}

impl WasmClient {
    /// Fetch info for several accounts in a single request.
    pub async fn get_multiple_accounts(
        &self,
        addresses: &[Address],
        config: Option<RpcAccountInfoConfig>,
    ) -> RpcResult<Response<Vec<Option<UiAccount>>>> {
        self.call(GetMultipleAccounts { addresses, config }).await
    }
}

impl BatchRequest {
    /// Fetch info for several accounts in a single request.
    pub fn get_multiple_accounts(
        &mut self,
        addresses: &[Address],
        config: Option<RpcAccountInfoConfig>,
    ) -> BatchHandle<Response<Vec<Option<UiAccount>>>> {
        self.add(GetMultipleAccounts { addresses, config })
    }
}

// ── getProgramAccounts ──────────────────────────────────────────────────────

pub struct GetProgramAccounts<'a> {
    pub program_id: &'a Address,
    pub config: Option<RpcProgramAccountsConfig>,
}

impl RpcMethod for GetProgramAccounts<'_> {
    type Output = OptionalContext<Vec<RpcKeyedAccount>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetProgramAccounts
    }
    fn params(&self) -> Value {
        json!([self.program_id.to_string(), self.config])
    }
}

impl WasmClient {
    /// Fetch all accounts owned by the given program, optionally filtered.
    pub async fn get_program_accounts(
        &self,
        program_id: &Address,
        config: Option<RpcProgramAccountsConfig>,
    ) -> RpcResult<OptionalContext<Vec<RpcKeyedAccount>>> {
        self.call(GetProgramAccounts { program_id, config }).await
    }
}

impl BatchRequest {
    /// Fetch all accounts owned by the given program, optionally filtered.
    pub fn get_program_accounts(
        &mut self,
        program_id: &Address,
        config: Option<RpcProgramAccountsConfig>,
    ) -> BatchHandle<OptionalContext<Vec<RpcKeyedAccount>>> {
        self.add(GetProgramAccounts { program_id, config })
    }
}

// ── getTokenAccountBalance ──────────────────────────────────────────────────

pub struct GetTokenAccountBalance<'a> {
    pub account: &'a Address,
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetTokenAccountBalance<'_> {
    type Output = Response<UiTokenAmount>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTokenAccountBalance
    }
    fn params(&self) -> Value {
        json!([self.account.to_string(), self.config])
    }
}

impl WasmClient {
    /// Return the SPL token balance held by a token account.
    pub async fn get_token_account_balance(
        &self,
        account: &Address,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Response<UiTokenAmount>> {
        self.call(GetTokenAccountBalance { account, config }).await
    }
}

impl BatchRequest {
    /// Return the SPL token balance held by a token account.
    pub fn get_token_account_balance(
        &mut self,
        account: &Address,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Response<UiTokenAmount>> {
        self.add(GetTokenAccountBalance { account, config })
    }
}

// ── getTokenAccountsByDelegate ──────────────────────────────────────────────

pub struct GetTokenAccountsByDelegate<'a> {
    pub delegate: &'a Address,
    pub filter: RpcTokenAccountsFilter,
    pub config: Option<RpcAccountInfoConfig>,
}

impl RpcMethod for GetTokenAccountsByDelegate<'_> {
    type Output = Response<Vec<RpcKeyedAccount>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTokenAccountsByDelegate
    }
    fn params(&self) -> Value {
        json!([self.delegate.to_string(), self.filter, self.config])
    }
}

impl WasmClient {
    /// Fetch SPL token accounts delegated to the given address.
    pub async fn get_token_accounts_by_delegate(
        &self,
        delegate: &Address,
        filter: RpcTokenAccountsFilter,
        config: Option<RpcAccountInfoConfig>,
    ) -> RpcResult<Response<Vec<RpcKeyedAccount>>> {
        self.call(GetTokenAccountsByDelegate {
            delegate,
            filter,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// Fetch SPL token accounts delegated to the given address.
    pub fn get_token_accounts_by_delegate(
        &mut self,
        delegate: &Address,
        filter: RpcTokenAccountsFilter,
        config: Option<RpcAccountInfoConfig>,
    ) -> BatchHandle<Response<Vec<RpcKeyedAccount>>> {
        self.add(GetTokenAccountsByDelegate {
            delegate,
            filter,
            config,
        })
    }
}

// ── getTokenAccountsByOwner ─────────────────────────────────────────────────

pub struct GetTokenAccountsByOwner<'a> {
    pub owner: &'a Address,
    pub filter: RpcTokenAccountsFilter,
    pub config: Option<RpcAccountInfoConfig>,
}

impl RpcMethod for GetTokenAccountsByOwner<'_> {
    type Output = Response<Vec<RpcKeyedAccount>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTokenAccountsByOwner
    }
    fn params(&self) -> Value {
        json!([self.owner.to_string(), self.filter, self.config])
    }
}

impl WasmClient {
    /// Fetch SPL token accounts owned by the given address.
    pub async fn get_token_accounts_by_owner(
        &self,
        owner: &Address,
        filter: RpcTokenAccountsFilter,
        config: Option<RpcAccountInfoConfig>,
    ) -> RpcResult<Response<Vec<RpcKeyedAccount>>> {
        self.call(GetTokenAccountsByOwner {
            owner,
            filter,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// Fetch SPL token accounts owned by the given address.
    pub fn get_token_accounts_by_owner(
        &mut self,
        owner: &Address,
        filter: RpcTokenAccountsFilter,
        config: Option<RpcAccountInfoConfig>,
    ) -> BatchHandle<Response<Vec<RpcKeyedAccount>>> {
        self.add(GetTokenAccountsByOwner {
            owner,
            filter,
            config,
        })
    }
}

// ── getTokenLargestAccounts ─────────────────────────────────────────────────

pub struct GetTokenLargestAccounts<'a> {
    pub mint: &'a Address,
    pub config: Option<CommitmentConfig>,
}

impl RpcMethod for GetTokenLargestAccounts<'_> {
    type Output = Response<Vec<RpcTokenAccountBalance>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTokenLargestAccounts
    }
    fn params(&self) -> Value {
        json!([self.mint.to_string(), self.config])
    }
}

impl WasmClient {
    /// Return the 20 largest token accounts for a given mint.
    pub async fn get_token_largest_accounts(
        &self,
        mint: &Address,
        config: Option<CommitmentConfig>,
    ) -> RpcResult<Response<Vec<RpcTokenAccountBalance>>> {
        self.call(GetTokenLargestAccounts { mint, config }).await
    }
}

impl BatchRequest {
    /// Return the 20 largest token accounts for a given mint.
    pub fn get_token_largest_accounts(
        &mut self,
        mint: &Address,
        config: Option<CommitmentConfig>,
    ) -> BatchHandle<Response<Vec<RpcTokenAccountBalance>>> {
        self.add(GetTokenLargestAccounts { mint, config })
    }
}

// ── getTokenSupply ──────────────────────────────────────────────────────────

pub struct GetTokenSupply<'a> {
    pub mint: &'a Address,
    pub config: Option<CommitmentConfig>,
}

impl RpcMethod for GetTokenSupply<'_> {
    type Output = Response<UiTokenAmount>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTokenSupply
    }
    fn params(&self) -> Value {
        json!([self.mint.to_string(), self.config])
    }
}

impl WasmClient {
    /// Return the total supply of an SPL token mint.
    pub async fn get_token_supply(
        &self,
        mint: &Address,
        config: Option<CommitmentConfig>,
    ) -> RpcResult<Response<UiTokenAmount>> {
        self.call(GetTokenSupply { mint, config }).await
    }
}

impl BatchRequest {
    /// Return the total supply of an SPL token mint.
    pub fn get_token_supply(
        &mut self,
        mint: &Address,
        config: Option<CommitmentConfig>,
    ) -> BatchHandle<Response<UiTokenAmount>> {
        self.add(GetTokenSupply { mint, config })
    }
}

// ── getFeeForMessage ────────────────────────────────────────────────────────

pub struct GetFeeForMessage {
    pub message: String,
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetFeeForMessage {
    type Output = Response<Option<u64>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetFeeForMessage
    }
    fn params(&self) -> Value {
        json!([self.message, self.config])
    }
}

impl WasmClient {
    /// Return the fee the cluster would charge to process a base64-encoded transaction message.
    pub async fn get_fee_for_message(
        &self,
        message: String,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Response<Option<u64>>> {
        self.call(GetFeeForMessage { message, config }).await
    }
}

impl BatchRequest {
    /// Return the fee the cluster would charge to process a base64-encoded transaction message.
    pub fn get_fee_for_message(
        &mut self,
        message: String,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Response<Option<u64>>> {
        self.add(GetFeeForMessage { message, config })
    }
}

// ── getLatestBlockhash ──────────────────────────────────────────────────────

pub struct GetLatestBlockhash {
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetLatestBlockhash {
    type Output = Response<RpcBlockhash>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetLatestBlockhash
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Fetch the latest blockhash and the last block height at which it is still valid.
    pub async fn get_latest_blockhash(
        &self,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Response<RpcBlockhash>> {
        self.call(GetLatestBlockhash { config }).await
    }
}

impl BatchRequest {
    /// Fetch the latest blockhash and the last block height at which it is still valid.
    pub fn get_latest_blockhash(
        &mut self,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Response<RpcBlockhash>> {
        self.add(GetLatestBlockhash { config })
    }
}

// ── getRecentPrioritizationFees ─────────────────────────────────────────────

pub struct GetRecentPrioritizationFees<'a> {
    pub addresses: Option<&'a [Address]>,
}

impl RpcMethod for GetRecentPrioritizationFees<'_> {
    type Output = Vec<RpcPrioritizationFee>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetRecentPrioritizationFees
    }
    fn params(&self) -> Value {
        // Omit the positional entirely when `None` — the RPC rejects `[null]`.
        match self.addresses {
            Some(addresses) => {
                let addresses: Vec<String> = addresses.iter().map(Address::to_string).collect();
                json!([addresses])
            }
            None => json!([]),
        }
    }
}

impl WasmClient {
    /// Return recent per-slot prioritization fees, optionally restricted to accounts that must be
    /// writable.
    pub async fn get_recent_prioritization_fees(
        &self,
        addresses: Option<&[Address]>,
    ) -> RpcResult<Vec<RpcPrioritizationFee>> {
        self.call(GetRecentPrioritizationFees { addresses }).await
    }
}

impl BatchRequest {
    /// Return recent per-slot prioritization fees, optionally restricted to accounts that must be
    /// writable.
    pub fn get_recent_prioritization_fees(
        &mut self,
        addresses: Option<&[Address]>,
    ) -> BatchHandle<Vec<RpcPrioritizationFee>> {
        self.add(GetRecentPrioritizationFees { addresses })
    }
}

// ── getSignaturesForAddress ─────────────────────────────────────────────────

pub struct GetSignaturesForAddress<'a> {
    pub address: &'a Address,
    pub config: Option<RpcSignaturesForAddressConfig>,
}

impl RpcMethod for GetSignaturesForAddress<'_> {
    type Output = Vec<RpcConfirmedTransactionStatusWithSignature>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetSignaturesForAddress
    }
    fn params(&self) -> Value {
        json!([self.address.to_string(), self.config])
    }
}

impl WasmClient {
    /// Return confirmed transaction signatures involving the given address, most recent first.
    pub async fn get_signatures_for_address(
        &self,
        address: &Address,
        config: Option<RpcSignaturesForAddressConfig>,
    ) -> RpcResult<Vec<RpcConfirmedTransactionStatusWithSignature>> {
        self.call(GetSignaturesForAddress { address, config }).await
    }
}

impl BatchRequest {
    /// Return confirmed transaction signatures involving the given address, most recent first.
    pub fn get_signatures_for_address(
        &mut self,
        address: &Address,
        config: Option<RpcSignaturesForAddressConfig>,
    ) -> BatchHandle<Vec<RpcConfirmedTransactionStatusWithSignature>> {
        self.add(GetSignaturesForAddress { address, config })
    }
}

// ── getSignatureStatuses ────────────────────────────────────────────────────

pub struct GetSignatureStatuses {
    pub signatures: Vec<String>,
    pub config: Option<RpcSignatureStatusConfig>,
}

impl RpcMethod for GetSignatureStatuses {
    type Output = Response<Vec<Option<TransactionStatus>>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetSignatureStatuses
    }
    fn params(&self) -> Value {
        json!([self.signatures, self.config])
    }
}

impl WasmClient {
    /// Return the processing status of one or more transaction signatures.
    pub async fn get_signature_statuses(
        &self,
        signatures: Vec<String>,
        config: Option<RpcSignatureStatusConfig>,
    ) -> RpcResult<Response<Vec<Option<TransactionStatus>>>> {
        self.call(GetSignatureStatuses { signatures, config }).await
    }
}

impl BatchRequest {
    /// Return the processing status of one or more transaction signatures.
    pub fn get_signature_statuses(
        &mut self,
        signatures: Vec<String>,
        config: Option<RpcSignatureStatusConfig>,
    ) -> BatchHandle<Response<Vec<Option<TransactionStatus>>>> {
        self.add(GetSignatureStatuses { signatures, config })
    }
}

// ── getTransaction ──────────────────────────────────────────────────────────

pub struct GetTransaction {
    pub signature: String,
    pub config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
}

impl RpcMethod for GetTransaction {
    type Output = Option<EncodedConfirmedTransactionWithStatusMeta>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTransaction
    }
    fn params(&self) -> Value {
        json!([self.signature, self.config])
    }
}

impl WasmClient {
    /// Fetch a confirmed transaction by its signature.
    pub async fn get_transaction(
        &self,
        signature: String,
        config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
    ) -> RpcResult<Option<EncodedConfirmedTransactionWithStatusMeta>> {
        self.call(GetTransaction { signature, config }).await
    }
}

impl BatchRequest {
    /// Fetch a confirmed transaction by its signature.
    pub fn get_transaction(
        &mut self,
        signature: String,
        config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
    ) -> BatchHandle<Option<EncodedConfirmedTransactionWithStatusMeta>> {
        self.add(GetTransaction { signature, config })
    }
}

// ── getTransactionCount ─────────────────────────────────────────────────────

pub struct GetTransactionCount {
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetTransactionCount {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetTransactionCount
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the cumulative number of transactions processed by the cluster.
    pub async fn get_transaction_count(&self, config: Option<RpcContextConfig>) -> RpcResult<u64> {
        self.call(GetTransactionCount { config }).await
    }
}

impl BatchRequest {
    /// Return the cumulative number of transactions processed by the cluster.
    pub fn get_transaction_count(&mut self, config: Option<RpcContextConfig>) -> BatchHandle<u64> {
        self.add(GetTransactionCount { config })
    }
}

// ── isBlockhashValid ────────────────────────────────────────────────────────

pub struct IsBlockhashValid {
    pub blockhash: String,
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for IsBlockhashValid {
    type Output = Response<bool>;
    fn request(&self) -> RpcRequest {
        RpcRequest::IsBlockhashValid
    }
    fn params(&self) -> Value {
        json!([self.blockhash, self.config])
    }
}

impl WasmClient {
    /// Report whether the given blockhash is still valid.
    pub async fn is_blockhash_valid(
        &self,
        blockhash: String,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Response<bool>> {
        self.call(IsBlockhashValid { blockhash, config }).await
    }
}

impl BatchRequest {
    /// Report whether the given blockhash is still valid.
    pub fn is_blockhash_valid(
        &mut self,
        blockhash: String,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Response<bool>> {
        self.add(IsBlockhashValid { blockhash, config })
    }
}

// ── requestAirdrop ──────────────────────────────────────────────────────────

pub struct RequestAirdrop<'a> {
    pub address: &'a Address,
    pub lamports: u64,
    pub config: Option<RpcRequestAirdropConfig>,
}

impl RpcMethod for RequestAirdrop<'_> {
    type Output = String;
    fn request(&self) -> RpcRequest {
        RpcRequest::RequestAirdrop
    }
    fn params(&self) -> Value {
        json!([self.address.to_string(), self.lamports, self.config])
    }
}

impl WasmClient {
    /// Request an airdrop of lamports to an address (devnet/testnet only). Returns the signature.
    pub async fn request_airdrop(
        &self,
        address: &Address,
        lamports: u64,
        config: Option<RpcRequestAirdropConfig>,
    ) -> RpcResult<String> {
        self.call(RequestAirdrop {
            address,
            lamports,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// Request an airdrop of lamports to an address (devnet/testnet only). Returns the signature.
    pub fn request_airdrop(
        &mut self,
        address: &Address,
        lamports: u64,
        config: Option<RpcRequestAirdropConfig>,
    ) -> BatchHandle<String> {
        self.add(RequestAirdrop {
            address,
            lamports,
            config,
        })
    }
}

// ── sendTransaction ─────────────────────────────────────────────────────────

pub struct SendTransaction {
    pub transaction: String,
    pub config: Option<RpcSendTransactionConfig>,
}

impl RpcMethod for SendTransaction {
    type Output = String;
    fn request(&self) -> RpcRequest {
        RpcRequest::SendTransaction
    }
    fn params(&self) -> Value {
        json!([self.transaction, self.config])
    }
}

impl WasmClient {
    /// Submit a signed transaction. Does not wait for confirmation; returns the signature.
    pub async fn send_transaction(
        &self,
        transaction: String,
        config: Option<RpcSendTransactionConfig>,
    ) -> RpcResult<String> {
        self.call(SendTransaction {
            transaction,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// Submit a signed transaction. Does not wait for confirmation; returns the signature.
    pub fn send_transaction(
        &mut self,
        transaction: String,
        config: Option<RpcSendTransactionConfig>,
    ) -> BatchHandle<String> {
        self.add(SendTransaction {
            transaction,
            config,
        })
    }
}

// ── simulateTransaction ─────────────────────────────────────────────────────

pub struct SimulateTransaction {
    pub transaction: String,
    pub config: Option<RpcSimulateTransactionConfig>,
}

impl RpcMethod for SimulateTransaction {
    type Output = Response<RpcSimulateTransactionResult>;
    fn request(&self) -> RpcRequest {
        RpcRequest::SimulateTransaction
    }
    fn params(&self) -> Value {
        json!([self.transaction, self.config])
    }
}

impl WasmClient {
    /// Simulate a transaction without submitting it. Returns logs, accounts, and any error.
    pub async fn simulate_transaction(
        &self,
        transaction: String,
        config: Option<RpcSimulateTransactionConfig>,
    ) -> RpcResult<Response<RpcSimulateTransactionResult>> {
        self.call(SimulateTransaction {
            transaction,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// Simulate a transaction without submitting it. Returns logs, accounts, and any error.
    pub fn simulate_transaction(
        &mut self,
        transaction: String,
        config: Option<RpcSimulateTransactionConfig>,
    ) -> BatchHandle<Response<RpcSimulateTransactionResult>> {
        self.add(SimulateTransaction {
            transaction,
            config,
        })
    }
}

// ── getBlock ────────────────────────────────────────────────────────────────

pub struct GetBlock {
    pub slot: u64,
    pub config: Option<RpcEncodingConfigWrapper<RpcBlockConfig>>,
}

impl RpcMethod for GetBlock {
    type Output = UiConfirmedBlock;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBlock
    }
    fn params(&self) -> Value {
        json!([self.slot, self.config])
    }
}

impl WasmClient {
    /// Fetch a confirmed block by slot.
    pub async fn get_block(
        &self,
        slot: u64,
        config: Option<RpcEncodingConfigWrapper<RpcBlockConfig>>,
    ) -> RpcResult<UiConfirmedBlock> {
        self.call(GetBlock { slot, config }).await
    }
}

impl BatchRequest {
    /// Fetch a confirmed block by slot.
    pub fn get_block(
        &mut self,
        slot: u64,
        config: Option<RpcEncodingConfigWrapper<RpcBlockConfig>>,
    ) -> BatchHandle<UiConfirmedBlock> {
        self.add(GetBlock { slot, config })
    }
}

// ── getBlockCommitment ──────────────────────────────────────────────────────

pub struct GetBlockCommitment {
    pub slot: u64,
}

impl RpcMethod for GetBlockCommitment {
    type Output = RpcBlockCommitment<Vec<usize>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::Custom {
            method: "getBlockCommitment",
        }
    }
    fn params(&self) -> Value {
        json!([self.slot])
    }
}

impl WasmClient {
    /// Return per-stake vote commitment for a block at the given slot.
    pub async fn get_block_commitment(
        &self,
        slot: u64,
    ) -> RpcResult<RpcBlockCommitment<Vec<usize>>> {
        self.call(GetBlockCommitment { slot }).await
    }
}

impl BatchRequest {
    /// Return per-stake vote commitment for a block at the given slot.
    pub fn get_block_commitment(
        &mut self,
        slot: u64,
    ) -> BatchHandle<RpcBlockCommitment<Vec<usize>>> {
        self.add(GetBlockCommitment { slot })
    }
}

// ── getBlockHeight ──────────────────────────────────────────────────────────

pub struct GetBlockHeight {
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetBlockHeight {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBlockHeight
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the current block height of the node.
    pub async fn get_block_height(&self, config: Option<RpcContextConfig>) -> RpcResult<u64> {
        self.call(GetBlockHeight { config }).await
    }
}

impl BatchRequest {
    /// Return the current block height of the node.
    pub fn get_block_height(&mut self, config: Option<RpcContextConfig>) -> BatchHandle<u64> {
        self.add(GetBlockHeight { config })
    }
}

// ── getBlockProduction ──────────────────────────────────────────────────────

pub struct GetBlockProduction {
    pub config: Option<RpcBlockProductionConfig>,
}

impl RpcMethod for GetBlockProduction {
    type Output = Response<RpcBlockProduction>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBlockProduction
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return recent block production information, broken down by validator identity.
    pub async fn get_block_production(
        &self,
        config: Option<RpcBlockProductionConfig>,
    ) -> RpcResult<Response<RpcBlockProduction>> {
        self.call(GetBlockProduction { config }).await
    }
}

impl BatchRequest {
    /// Return recent block production information, broken down by validator identity.
    pub fn get_block_production(
        &mut self,
        config: Option<RpcBlockProductionConfig>,
    ) -> BatchHandle<Response<RpcBlockProduction>> {
        self.add(GetBlockProduction { config })
    }
}

// ── getBlocks ───────────────────────────────────────────────────────────────

pub struct GetBlocks {
    pub start_slot: u64,
    pub end_slot: Option<u64>,
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetBlocks {
    type Output = Vec<u64>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBlocks
    }
    fn params(&self) -> Value {
        json!([self.start_slot, self.end_slot, self.config])
    }
}

impl WasmClient {
    /// List confirmed blocks in the inclusive slot range `[start_slot, end_slot]`.
    pub async fn get_blocks(
        &self,
        start_slot: u64,
        end_slot: Option<u64>,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Vec<u64>> {
        self.call(GetBlocks {
            start_slot,
            end_slot,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// List confirmed blocks in the inclusive slot range `[start_slot, end_slot]`.
    pub fn get_blocks(
        &mut self,
        start_slot: u64,
        end_slot: Option<u64>,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Vec<u64>> {
        self.add(GetBlocks {
            start_slot,
            end_slot,
            config,
        })
    }
}

// ── getBlocksWithLimit ──────────────────────────────────────────────────────

pub struct GetBlocksWithLimit {
    pub start_slot: u64,
    pub limit: u64,
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetBlocksWithLimit {
    type Output = Vec<u64>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBlocksWithLimit
    }
    fn params(&self) -> Value {
        json!([self.start_slot, self.limit, self.config])
    }
}

impl WasmClient {
    /// List up to `limit` confirmed blocks starting at `start_slot`.
    pub async fn get_blocks_with_limit(
        &self,
        start_slot: u64,
        limit: u64,
        config: Option<RpcContextConfig>,
    ) -> RpcResult<Vec<u64>> {
        self.call(GetBlocksWithLimit {
            start_slot,
            limit,
            config,
        })
        .await
    }
}

impl BatchRequest {
    /// List up to `limit` confirmed blocks starting at `start_slot`.
    pub fn get_blocks_with_limit(
        &mut self,
        start_slot: u64,
        limit: u64,
        config: Option<RpcContextConfig>,
    ) -> BatchHandle<Vec<u64>> {
        self.add(GetBlocksWithLimit {
            start_slot,
            limit,
            config,
        })
    }
}

// ── getBlockTime ────────────────────────────────────────────────────────────

pub struct GetBlockTime {
    pub slot: u64,
}

impl RpcMethod for GetBlockTime {
    type Output = Option<i64>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetBlockTime
    }
    fn params(&self) -> Value {
        json!([self.slot])
    }
}

impl WasmClient {
    /// Return the estimated UNIX production timestamp of a block, if available.
    pub async fn get_block_time(&self, slot: u64) -> RpcResult<Option<i64>> {
        self.call(GetBlockTime { slot }).await
    }
}

impl BatchRequest {
    /// Return the estimated UNIX production timestamp of a block, if available.
    pub fn get_block_time(&mut self, slot: u64) -> BatchHandle<Option<i64>> {
        self.add(GetBlockTime { slot })
    }
}

// ── getFirstAvailableBlock ──────────────────────────────────────────────────

pub struct GetFirstAvailableBlock;

impl RpcMethod for GetFirstAvailableBlock {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetFirstAvailableBlock
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the slot of the lowest confirmed block still retained by the node.
    pub async fn get_first_available_block(&self) -> RpcResult<u64> {
        self.call(GetFirstAvailableBlock).await
    }
}

impl BatchRequest {
    /// Return the slot of the lowest confirmed block still retained by the node.
    pub fn get_first_available_block(&mut self) -> BatchHandle<u64> {
        self.add(GetFirstAvailableBlock)
    }
}

// ── getRecentPerformanceSamples ─────────────────────────────────────────────

pub struct GetRecentPerformanceSamples {
    pub limit: Option<u32>,
}

impl RpcMethod for GetRecentPerformanceSamples {
    type Output = Vec<RpcPerfSample>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetRecentPerformanceSamples
    }
    fn params(&self) -> Value {
        // Omit the positional entirely when `None` — the RPC rejects `[null]`.
        match self.limit {
            Some(n) => json!([n]),
            None => json!([]),
        }
    }
}

impl WasmClient {
    /// Return recent slot-time performance samples, up to `limit` (default 720).
    pub async fn get_recent_performance_samples(
        &self,
        limit: Option<u32>,
    ) -> RpcResult<Vec<RpcPerfSample>> {
        self.call(GetRecentPerformanceSamples { limit }).await
    }
}

impl BatchRequest {
    /// Return recent slot-time performance samples, up to `limit` (default 720).
    pub fn get_recent_performance_samples(
        &mut self,
        limit: Option<u32>,
    ) -> BatchHandle<Vec<RpcPerfSample>> {
        self.add(GetRecentPerformanceSamples { limit })
    }
}

// ── minimumLedgerSlot ───────────────────────────────────────────────────────

pub struct MinimumLedgerSlot;

impl RpcMethod for MinimumLedgerSlot {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::MinimumLedgerSlot
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the lowest slot the node has information about.
    pub async fn minimum_ledger_slot(&self) -> RpcResult<u64> {
        self.call(MinimumLedgerSlot).await
    }
}

impl BatchRequest {
    /// Return the lowest slot the node has information about.
    pub fn minimum_ledger_slot(&mut self) -> BatchHandle<u64> {
        self.add(MinimumLedgerSlot)
    }
}

// ── getClusterNodes ─────────────────────────────────────────────────────────

pub struct GetClusterNodes;

impl RpcMethod for GetClusterNodes {
    type Output = Vec<RpcContactInfo>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetClusterNodes
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return information about all known cluster nodes.
    pub async fn get_cluster_nodes(&self) -> RpcResult<Vec<RpcContactInfo>> {
        self.call(GetClusterNodes).await
    }
}

impl BatchRequest {
    /// Return information about all known cluster nodes.
    pub fn get_cluster_nodes(&mut self) -> BatchHandle<Vec<RpcContactInfo>> {
        self.add(GetClusterNodes)
    }
}

// ── getEpochInfo ────────────────────────────────────────────────────────────

pub struct GetEpochInfo {
    pub config: Option<RpcEpochConfig>,
}

impl RpcMethod for GetEpochInfo {
    type Output = EpochInfo;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetEpochInfo
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return information about the current epoch.
    pub async fn get_epoch_info(&self, config: Option<RpcEpochConfig>) -> RpcResult<EpochInfo> {
        self.call(GetEpochInfo { config }).await
    }
}

impl BatchRequest {
    /// Return information about the current epoch.
    pub fn get_epoch_info(&mut self, config: Option<RpcEpochConfig>) -> BatchHandle<EpochInfo> {
        self.add(GetEpochInfo { config })
    }
}

// ── getEpochSchedule ────────────────────────────────────────────────────────

pub struct GetEpochSchedule;

impl RpcMethod for GetEpochSchedule {
    type Output = EpochSchedule;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetEpochSchedule
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the cluster's epoch schedule parameters from the genesis config.
    pub async fn get_epoch_schedule(&self) -> RpcResult<EpochSchedule> {
        self.call(GetEpochSchedule).await
    }
}

impl BatchRequest {
    /// Return the cluster's epoch schedule parameters from the genesis config.
    pub fn get_epoch_schedule(&mut self) -> BatchHandle<EpochSchedule> {
        self.add(GetEpochSchedule)
    }
}

// ── getGenesisHash ──────────────────────────────────────────────────────────

pub struct GetGenesisHash;

impl RpcMethod for GetGenesisHash {
    type Output = String;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetGenesisHash
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the cluster's genesis hash.
    pub async fn get_genesis_hash(&self) -> RpcResult<String> {
        self.call(GetGenesisHash).await
    }
}

impl BatchRequest {
    /// Return the cluster's genesis hash.
    pub fn get_genesis_hash(&mut self) -> BatchHandle<String> {
        self.add(GetGenesisHash)
    }
}

// ── getHealth ───────────────────────────────────────────────────────────────

pub struct GetHealth;

impl RpcMethod for GetHealth {
    type Output = String;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetHealth
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return `"ok"` when the node is caught up with its peers.
    pub async fn get_health(&self) -> RpcResult<String> {
        self.call(GetHealth).await
    }
}

impl BatchRequest {
    /// Return `"ok"` when the node is caught up with its peers.
    pub fn get_health(&mut self) -> BatchHandle<String> {
        self.add(GetHealth)
    }
}

// ── getHighestSnapshotSlot ──────────────────────────────────────────────────

pub struct GetHighestSnapshotSlot;

impl RpcMethod for GetHighestSnapshotSlot {
    type Output = RpcSnapshotSlotInfo;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetHighestSnapshotSlot
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the highest full (and optional incremental) snapshot slot the node has stored.
    pub async fn get_highest_snapshot_slot(&self) -> RpcResult<RpcSnapshotSlotInfo> {
        self.call(GetHighestSnapshotSlot).await
    }
}

impl BatchRequest {
    /// Return the highest full (and optional incremental) snapshot slot the node has stored.
    pub fn get_highest_snapshot_slot(&mut self) -> BatchHandle<RpcSnapshotSlotInfo> {
        self.add(GetHighestSnapshotSlot)
    }
}

// ── getIdentity ─────────────────────────────────────────────────────────────

pub struct GetIdentity;

impl RpcMethod for GetIdentity {
    type Output = RpcIdentity;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetIdentity
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the node's identity pubkey.
    pub async fn get_identity(&self) -> RpcResult<RpcIdentity> {
        self.call(GetIdentity).await
    }
}

impl BatchRequest {
    /// Return the node's identity pubkey.
    pub fn get_identity(&mut self) -> BatchHandle<RpcIdentity> {
        self.add(GetIdentity)
    }
}

// ── getLeaderSchedule ───────────────────────────────────────────────────────

pub struct GetLeaderSchedule {
    pub slot: Option<u64>,
    pub config: Option<RpcLeaderScheduleConfig>,
}

impl RpcMethod for GetLeaderSchedule {
    type Output = Option<RpcLeaderSchedule>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetLeaderSchedule
    }
    fn params(&self) -> Value {
        json!([self.slot, self.config])
    }
}

impl WasmClient {
    /// Return the leader schedule for the epoch containing `slot` (or the current epoch).
    pub async fn get_leader_schedule(
        &self,
        slot: Option<u64>,
        config: Option<RpcLeaderScheduleConfig>,
    ) -> RpcResult<Option<RpcLeaderSchedule>> {
        self.call(GetLeaderSchedule { slot, config }).await
    }
}

impl BatchRequest {
    /// Return the leader schedule for the epoch containing `slot` (or the current epoch).
    pub fn get_leader_schedule(
        &mut self,
        slot: Option<u64>,
        config: Option<RpcLeaderScheduleConfig>,
    ) -> BatchHandle<Option<RpcLeaderSchedule>> {
        self.add(GetLeaderSchedule { slot, config })
    }
}

// ── getMaxRetransmitSlot ────────────────────────────────────────────────────

pub struct GetMaxRetransmitSlot;

impl RpcMethod for GetMaxRetransmitSlot {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetMaxRetransmitSlot
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the highest slot seen via the retransmit stage.
    pub async fn get_max_retransmit_slot(&self) -> RpcResult<u64> {
        self.call(GetMaxRetransmitSlot).await
    }
}

impl BatchRequest {
    /// Return the highest slot seen via the retransmit stage.
    pub fn get_max_retransmit_slot(&mut self) -> BatchHandle<u64> {
        self.add(GetMaxRetransmitSlot)
    }
}

// ── getMaxShredInsertSlot ───────────────────────────────────────────────────

pub struct GetMaxShredInsertSlot;

impl RpcMethod for GetMaxShredInsertSlot {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetMaxShredInsertSlot
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the highest slot for which shreds have been inserted.
    pub async fn get_max_shred_insert_slot(&self) -> RpcResult<u64> {
        self.call(GetMaxShredInsertSlot).await
    }
}

impl BatchRequest {
    /// Return the highest slot for which shreds have been inserted.
    pub fn get_max_shred_insert_slot(&mut self) -> BatchHandle<u64> {
        self.add(GetMaxShredInsertSlot)
    }
}

// ── getSlot ─────────────────────────────────────────────────────────────────

pub struct GetSlot {
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetSlot {
    type Output = u64;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetSlot
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the slot the node is currently processing.
    pub async fn get_slot(&self, config: Option<RpcContextConfig>) -> RpcResult<u64> {
        self.call(GetSlot { config }).await
    }
}

impl BatchRequest {
    /// Return the slot the node is currently processing.
    pub fn get_slot(&mut self, config: Option<RpcContextConfig>) -> BatchHandle<u64> {
        self.add(GetSlot { config })
    }
}

// ── getSlotLeader ───────────────────────────────────────────────────────────

pub struct GetSlotLeader {
    pub config: Option<RpcContextConfig>,
}

impl RpcMethod for GetSlotLeader {
    type Output = String;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetSlotLeader
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the identity of the current slot leader.
    pub async fn get_slot_leader(&self, config: Option<RpcContextConfig>) -> RpcResult<String> {
        self.call(GetSlotLeader { config }).await
    }
}

impl BatchRequest {
    /// Return the identity of the current slot leader.
    pub fn get_slot_leader(&mut self, config: Option<RpcContextConfig>) -> BatchHandle<String> {
        self.add(GetSlotLeader { config })
    }
}

// ── getSlotLeaders ──────────────────────────────────────────────────────────

pub struct GetSlotLeaders {
    pub start_slot: u64,
    pub limit: u64,
}

impl RpcMethod for GetSlotLeaders {
    type Output = Vec<String>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetSlotLeaders
    }
    fn params(&self) -> Value {
        json!([self.start_slot, self.limit])
    }
}

impl WasmClient {
    /// Return the slot leaders for the half-open range `[start_slot, start_slot + limit)`.
    pub async fn get_slot_leaders(&self, start_slot: u64, limit: u64) -> RpcResult<Vec<String>> {
        self.call(GetSlotLeaders { start_slot, limit }).await
    }
}

impl BatchRequest {
    /// Return the slot leaders for the half-open range `[start_slot, start_slot + limit)`.
    pub fn get_slot_leaders(&mut self, start_slot: u64, limit: u64) -> BatchHandle<Vec<String>> {
        self.add(GetSlotLeaders { start_slot, limit })
    }
}

// ── getVersion ──────────────────────────────────────────────────────────────

pub struct GetVersion;

impl RpcMethod for GetVersion {
    type Output = RpcVersionInfo;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetVersion
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the node's software version.
    pub async fn get_version(&self) -> RpcResult<RpcVersionInfo> {
        self.call(GetVersion).await
    }
}

impl BatchRequest {
    /// Return the node's software version.
    pub fn get_version(&mut self) -> BatchHandle<RpcVersionInfo> {
        self.add(GetVersion)
    }
}

// ── getVoteAccounts ─────────────────────────────────────────────────────────

pub struct GetVoteAccounts {
    pub config: Option<RpcGetVoteAccountsConfig>,
}

impl RpcMethod for GetVoteAccounts {
    type Output = RpcVoteAccountStatus;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetVoteAccounts
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the current and delinquent vote accounts.
    pub async fn get_vote_accounts(
        &self,
        config: Option<RpcGetVoteAccountsConfig>,
    ) -> RpcResult<RpcVoteAccountStatus> {
        self.call(GetVoteAccounts { config }).await
    }
}

impl BatchRequest {
    /// Return the current and delinquent vote accounts.
    pub fn get_vote_accounts(
        &mut self,
        config: Option<RpcGetVoteAccountsConfig>,
    ) -> BatchHandle<RpcVoteAccountStatus> {
        self.add(GetVoteAccounts { config })
    }
}

// ── getInflationGovernor ────────────────────────────────────────────────────

pub struct GetInflationGovernor {
    pub config: Option<CommitmentConfig>,
}

impl RpcMethod for GetInflationGovernor {
    type Output = RpcInflationGovernor;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetInflationGovernor
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the cluster's current inflation governor.
    pub async fn get_inflation_governor(
        &self,
        config: Option<CommitmentConfig>,
    ) -> RpcResult<RpcInflationGovernor> {
        self.call(GetInflationGovernor { config }).await
    }
}

impl BatchRequest {
    /// Return the cluster's current inflation governor.
    pub fn get_inflation_governor(
        &mut self,
        config: Option<CommitmentConfig>,
    ) -> BatchHandle<RpcInflationGovernor> {
        self.add(GetInflationGovernor { config })
    }
}

// ── getInflationRate ────────────────────────────────────────────────────────

pub struct GetInflationRate;

impl RpcMethod for GetInflationRate {
    type Output = RpcInflationRate;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetInflationRate
    }
    fn params(&self) -> Value {
        json!([])
    }
}

impl WasmClient {
    /// Return the specific inflation values for the current epoch.
    pub async fn get_inflation_rate(&self) -> RpcResult<RpcInflationRate> {
        self.call(GetInflationRate).await
    }
}

impl BatchRequest {
    /// Return the specific inflation values for the current epoch.
    pub fn get_inflation_rate(&mut self) -> BatchHandle<RpcInflationRate> {
        self.add(GetInflationRate)
    }
}

// ── getInflationReward ──────────────────────────────────────────────────────

pub struct GetInflationReward<'a> {
    pub addresses: &'a [Address],
    pub config: Option<RpcEpochConfig>,
}

impl RpcMethod for GetInflationReward<'_> {
    type Output = Vec<Option<RpcInflationReward>>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetInflationReward
    }
    fn params(&self) -> Value {
        let addresses: Vec<String> = self.addresses.iter().map(Address::to_string).collect();
        json!([addresses, self.config])
    }
}

impl WasmClient {
    /// Return inflation rewards earned by a list of addresses during an epoch.
    pub async fn get_inflation_reward(
        &self,
        addresses: &[Address],
        config: Option<RpcEpochConfig>,
    ) -> RpcResult<Vec<Option<RpcInflationReward>>> {
        self.call(GetInflationReward { addresses, config }).await
    }
}

impl BatchRequest {
    /// Return inflation rewards earned by a list of addresses during an epoch.
    pub fn get_inflation_reward(
        &mut self,
        addresses: &[Address],
        config: Option<RpcEpochConfig>,
    ) -> BatchHandle<Vec<Option<RpcInflationReward>>> {
        self.add(GetInflationReward { addresses, config })
    }
}

// ── getStakeMinimumDelegation ───────────────────────────────────────────────

pub struct GetStakeMinimumDelegation {
    pub config: Option<CommitmentConfig>,
}

impl RpcMethod for GetStakeMinimumDelegation {
    type Output = Response<u64>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetStakeMinimumDelegation
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return the stake-program minimum delegation in lamports.
    pub async fn get_stake_minimum_delegation(
        &self,
        config: Option<CommitmentConfig>,
    ) -> RpcResult<Response<u64>> {
        self.call(GetStakeMinimumDelegation { config }).await
    }
}

impl BatchRequest {
    /// Return the stake-program minimum delegation in lamports.
    pub fn get_stake_minimum_delegation(
        &mut self,
        config: Option<CommitmentConfig>,
    ) -> BatchHandle<Response<u64>> {
        self.add(GetStakeMinimumDelegation { config })
    }
}

// ── getSupply ───────────────────────────────────────────────────────────────

pub struct GetSupply {
    pub config: Option<RpcSupplyConfig>,
}

impl RpcMethod for GetSupply {
    type Output = Response<RpcSupply>;
    fn request(&self) -> RpcRequest {
        RpcRequest::GetSupply
    }
    fn params(&self) -> Value {
        json!([self.config])
    }
}

impl WasmClient {
    /// Return information about the cluster's circulating and non-circulating supply.
    pub async fn get_supply(
        &self,
        config: Option<RpcSupplyConfig>,
    ) -> RpcResult<Response<RpcSupply>> {
        self.call(GetSupply { config }).await
    }
}

impl BatchRequest {
    /// Return information about the cluster's circulating and non-circulating supply.
    pub fn get_supply(
        &mut self,
        config: Option<RpcSupplyConfig>,
    ) -> BatchHandle<Response<RpcSupply>> {
        self.add(GetSupply { config })
    }
}
