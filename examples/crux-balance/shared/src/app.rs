//! A Crux core that reads a Solana account balance.
//!
//! The core is side-effect free: it never performs the request, it asks the
//! shell to. `spume::CruxClient` supplies the typed RPC methods and sends
//! HTTP effects through `crux_http`.
//!
//! Run `cargo test` to drive it without a shell.

use {
    crux_core::{
        App, Command,
        macros::effect,
        render::{RenderOperation, render},
    },
    crux_http::HttpRequest,
    facet::Facet,
    serde::{Deserialize, Serialize},
    spume::CruxClient,
};

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

#[effect(facet_typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    Http(HttpRequest),
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum Event {
    // From the shell.
    GetBalance(String),

    // Local to the core: never crosses the FFI boundary.
    #[serde(skip)]
    #[facet(skip)]
    Balance(u64),
    #[serde(skip)]
    #[facet(skip)]
    Failed(String),
}

#[derive(Default)]
pub struct Model {
    pub lamports: Option<u64>,
    pub error: Option<String>,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewModel {
    pub balance: String,
}

#[derive(Default)]
pub struct Balance;

impl App for Balance {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::GetBalance(address) => {
                // Typed params, and a malformed address is rejected here rather
                // than by the RPC — no request leaves the core.
                let call = match spume::rpc::get_balance(&address, None) {
                    Ok(call) => call,
                    Err(err) => return self.update(Event::Failed(err.to_string()), model),
                };

                model.error = None;
                Command::new(|ctx| async move {
                    // Nothing here names `Response<u64>`; the call carries it.
                    let event = match CruxClient::new(RPC_URL, ctx.clone()).send(call).await {
                        Ok(balance) => Event::Balance(balance.value),
                        Err(err) => Event::Failed(err.to_string()),
                    };
                    ctx.send_event(event);
                })
            }
            Event::Balance(lamports) => {
                model.lamports = Some(lamports);
                render()
            }
            Event::Failed(error) => {
                model.error = Some(error);
                render()
            }
        }
    }

    fn view(&self, model: &Model) -> ViewModel {
        let balance = match (model.lamports, &model.error) {
            (_, Some(error)) => format!("error: {error}"),
            (Some(lamports), _) => format!("{} SOL", lamports as f64 / 1e9),
            (None, _) => "—".to_string(),
        };
        ViewModel { balance }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crux_core::Request,
        crux_http::protocol::{HttpResponse, HttpResult},
    };

    const ADDRESS: &str = "11111111111111111111111111111111";

    /// Drive one round trip: run the event, answer the HTTP effect the core
    /// asked for, then feed the resulting events back in. This is the whole
    /// shell, and it needs no network.
    fn round_trip(model: &mut Model, event: Event, answer: HttpResponse) -> Request<HttpRequest> {
        let app = Balance;
        let mut command = app.update(event, model);

        let Some(Effect::Http(mut request)) = command.effects().next() else {
            panic!("expected one http effect");
        };
        request
            .resolve(HttpResult::Ok(answer))
            .expect("request resolves");

        for event in command.events() {
            let _ = app.update(event, model);
        }
        request
    }

    #[test]
    fn a_balance_reaches_the_view() {
        let mut model = Model::default();

        let request = round_trip(
            &mut model,
            Event::GetBalance(ADDRESS.to_string()),
            HttpResponse::ok()
                .body(
                    br#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":2500000000}}"#
                        .to_vec(),
                )
                .build(),
        );

        // The core asked for exactly the call `spume::rpc::get_balance` built.
        let body: serde_json::Value =
            serde_json::from_slice(&request.operation.body).expect("valid json");
        assert_eq!(body["method"], "getBalance");
        assert_eq!(body["params"][0], ADDRESS);

        // Solana rejects anything else with `415 Invalid content-type`, and
        // `crux_http`'s `body()` sets `text/plain` unless it is corrected after.
        let content_type = request
            .operation
            .headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case("content-type"))
            .map(|header| header.value.as_str());
        assert_eq!(content_type, Some("application/json"));

        assert_eq!(model.lamports, Some(2_500_000_000));
        assert_eq!(Balance.view(&model).balance, "2.5 SOL");
    }

    #[test]
    fn a_json_rpc_error_survives_a_non_2xx() {
        let mut model = Model::default();

        round_trip(
            &mut model,
            Event::GetBalance(ADDRESS.to_string()),
            HttpResponse::status(500)
                .body(
                    br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"Node is behind"}}"#
                        .to_vec(),
                )
                .build(),
        );

        let error = model.error.expect("the rpc error, not a bare HTTP 500");
        assert!(error.contains("Node is behind"), "unexpected: {error}");
    }

    #[test]
    fn a_bad_address_never_reaches_the_shell() {
        let mut model = Model::default();
        let mut command = Balance.update(Event::GetBalance("not-an-address".into()), &mut model);

        assert!(
            command.effects().all(|e| !matches!(e, Effect::Http(_))),
            "no request should be issued"
        );
        assert!(model.error.expect("an error").contains("not-an-address"));
    }
}
