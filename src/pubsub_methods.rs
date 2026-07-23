use {
    crate::{WasmPubsubClient, methods::CheckAddress, pubsub_provider::Subscription},
    serde_json::json,
    solana_rpc_client_types::{
        config::{
            RpcAccountInfoConfig, RpcBlockSubscribeConfig, RpcBlockSubscribeFilter,
            RpcProgramAccountsConfig, RpcSignatureSubscribeConfig, RpcTransactionLogsConfig,
            RpcTransactionLogsFilter,
        },
        request::RpcError,
        response::{
            Response, RpcBlockUpdate, RpcKeyedAccount, RpcLogsResponse, RpcSignatureResult,
            RpcVote, SlotInfo, SlotUpdate, UiAccount,
        },
    },
};

type SubResult<T> = Result<Subscription<T>, Box<RpcError>>;

impl WasmPubsubClient {
    /// Subscribe to changes of a given account's lamports or data.
    pub async fn account_subscribe(
        &self,
        address: impl CheckAddress,
        config: Option<RpcAccountInfoConfig>,
    ) -> SubResult<Response<Option<UiAccount>>> {
        self.provider
            .subscribe(
                "accountSubscribe",
                "accountUnsubscribe",
                json!([address.parse()?, config]),
            )
            .await
    }

    /// Subscribe to incoming blocks reaching the configured commitment.
    pub async fn block_subscribe(
        &self,
        filter: RpcBlockSubscribeFilter,
        config: Option<RpcBlockSubscribeConfig>,
    ) -> SubResult<Response<RpcBlockUpdate>> {
        self.provider
            .subscribe(
                "blockSubscribe",
                "blockUnsubscribe",
                json!([filter, config]),
            )
            .await
    }

    /// Subscribe to transaction log messages matching the given filter.
    pub async fn logs_subscribe(
        &self,
        filter: RpcTransactionLogsFilter,
        config: Option<RpcTransactionLogsConfig>,
    ) -> SubResult<Response<RpcLogsResponse>> {
        self.provider
            .subscribe("logsSubscribe", "logsUnsubscribe", json!([filter, config]))
            .await
    }

    /// Subscribe to account changes for accounts owned by the given program.
    pub async fn program_subscribe(
        &self,
        program_id: impl CheckAddress,
        config: Option<RpcProgramAccountsConfig>,
    ) -> SubResult<Response<RpcKeyedAccount>> {
        self.provider
            .subscribe(
                "programSubscribe",
                "programUnsubscribe",
                json!([program_id.parse()?, config]),
            )
            .await
    }

    /// Subscribe to the validator setting a new root slot.
    pub async fn root_subscribe(&self) -> SubResult<u64> {
        self.provider
            .subscribe("rootSubscribe", "rootUnsubscribe", json!([]))
            .await
    }

    /// Subscribe to status notifications for a single transaction signature.
    ///
    /// The server auto-unsubscribes once the configured commitment is reached.
    pub async fn signature_subscribe(
        &self,
        signature: String,
        config: Option<RpcSignatureSubscribeConfig>,
    ) -> SubResult<Response<RpcSignatureResult>> {
        self.provider
            .subscribe(
                "signatureSubscribe",
                "signatureUnsubscribe",
                json!([signature, config]),
            )
            .await
    }

    /// Subscribe to new slots being processed by the validator.
    pub async fn slot_subscribe(&self) -> SubResult<SlotInfo> {
        self.provider
            .subscribe("slotSubscribe", "slotUnsubscribe", json!([]))
            .await
    }

    /// Subscribe to tagged slot lifecycle updates.
    pub async fn slots_updates_subscribe(&self) -> SubResult<SlotUpdate> {
        self.provider
            .subscribe(
                "slotsUpdatesSubscribe",
                "slotsUpdatesUnsubscribe",
                json!([]),
            )
            .await
    }

    /// Subscribe to gossip vote notifications.
    pub async fn vote_subscribe(&self) -> SubResult<RpcVote> {
        self.provider
            .subscribe("voteSubscribe", "voteUnsubscribe", json!([]))
            .await
    }
}
