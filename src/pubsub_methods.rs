use {
    crate::rpc::CheckAddress,
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

// One method table for both transports, just like the HTTP RPC methods.
macro_rules! pubsub_methods {
    ($(
        $(#[$meta:meta])*
        fn $name:ident ($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty
            = $subscribe:literal, $unsubscribe:literal, $params:expr;
    )*) => {
        #[cfg(feature = "pubsub")]
        impl crate::WasmPubsubClient {
            $(
                $(#[$meta])*
                pub async fn $name(&self, $($arg: $ty),*)
                    -> Result<crate::pubsub_provider::Subscription<$ret>, Box<RpcError>>
                {
                    self.provider.subscribe($subscribe, $unsubscribe, $params).await
                }
            )*
        }

        #[cfg(feature = "crux")]
        impl<Effect, Event> crate::CruxPubsubClient<Effect, Event>
        where
            Effect: From<crux_core::Request<crate::crux::WebSocketRequest>> + Send + 'static,
            Event: Send + 'static,
        {
            $(
                $(#[$meta])*
                pub async fn $name(&self, $($arg: $ty),*)
                    -> Result<crate::crux::CruxSubscription<$ret>, Box<RpcError>>
                {
                    self.subscribe($subscribe, $unsubscribe, $params).await
                }
            )*
        }
    };
}

pubsub_methods! {
    /// Subscribe to changes of a given account's lamports or data.
    fn account_subscribe(address: impl CheckAddress, config: Option<RpcAccountInfoConfig>)
        -> Response<Option<UiAccount>>
        = "accountSubscribe", "accountUnsubscribe", json!([address.parse()?, config]);

    /// Subscribe to incoming blocks reaching the configured commitment.
    fn block_subscribe(filter: RpcBlockSubscribeFilter, config: Option<RpcBlockSubscribeConfig>)
        -> Response<RpcBlockUpdate>
        = "blockSubscribe", "blockUnsubscribe", json!([filter, config]);

    /// Subscribe to transaction log messages matching the given filter.
    fn logs_subscribe(filter: RpcTransactionLogsFilter, config: Option<RpcTransactionLogsConfig>)
        -> Response<RpcLogsResponse>
        = "logsSubscribe", "logsUnsubscribe", json!([filter, config]);

    /// Subscribe to account changes for accounts owned by the given program.
    fn program_subscribe(program_id: impl CheckAddress, config: Option<RpcProgramAccountsConfig>)
        -> Response<RpcKeyedAccount>
        = "programSubscribe", "programUnsubscribe", json!([program_id.parse()?, config]);

    /// Subscribe to the validator setting a new root slot.
    fn root_subscribe() -> u64 = "rootSubscribe", "rootUnsubscribe", json!([]);

    /// Subscribe to status notifications for a single transaction signature.
    ///
    /// The server auto-unsubscribes once the configured commitment is reached.
    fn signature_subscribe(signature: String, config: Option<RpcSignatureSubscribeConfig>)
        -> Response<RpcSignatureResult>
        = "signatureSubscribe", "signatureUnsubscribe", json!([signature, config]);

    /// Subscribe to new slots being processed by the validator.
    fn slot_subscribe() -> SlotInfo = "slotSubscribe", "slotUnsubscribe", json!([]);

    /// Subscribe to tagged slot lifecycle updates.
    fn slots_updates_subscribe() -> SlotUpdate
        = "slotsUpdatesSubscribe", "slotsUpdatesUnsubscribe", json!([]);

    /// Subscribe to gossip vote notifications.
    fn vote_subscribe() -> RpcVote = "voteSubscribe", "voteUnsubscribe", json!([]);
}
