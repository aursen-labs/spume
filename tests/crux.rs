#![cfg(feature = "crux")]

use {
    crux_core::{Command, Request, macros::effect},
    crux_http::{
        HttpRequest,
        protocol::{HttpResponse, HttpResult},
    },
    futures::StreamExt,
    serde_json::{Value, json},
    spume::{
        CruxClient, CruxPubsubClient,
        crux::{WebSocketMessage, WebSocketRequest},
    },
};

#[effect]
enum Effect {
    Http(HttpRequest),
    WebSocket(WebSocketRequest),
}

type TestCommand = Command<Effect, Result<u64, String>>;

fn http_request(command: &mut TestCommand) -> Request<HttpRequest> {
    let Some(Effect::Http(request)) = command.effects().next() else {
        panic!("expected HTTP effect");
    };
    request
}

fn socket_request(command: &mut TestCommand) -> Request<WebSocketRequest> {
    let Some(Effect::WebSocket(request)) = command.effects().next() else {
        panic!("expected WebSocket effect");
    };
    request
}

#[test]
fn http_uses_typed_methods_and_preserves_errors() {
    for (status, body, expected) in [
        (
            200,
            json!({"result": {"context": {"slot": 1}, "value": 42}}),
            Ok(42),
        ),
        (
            500,
            json!({"error": {"code": -32005, "message": "Node is behind"}}),
            Err("Node is behind"),
        ),
        (
            500,
            json!({"result": {"context": {"slot": 1}, "value": 42}}),
            Err("HTTP 500"),
        ),
        (200, json!({"result": "wrong type"}), Err("invalid type")),
    ] {
        let mut command = TestCommand::new(|ctx| async move {
            let client = CruxClient::new("https://rpc.example.com", ctx.clone())
                .with_header("x-api-key", "secret");
            let result = client
                .clone()
                .get_balance("11111111111111111111111111111111", None)
                .await;
            ctx.send_event(
                result
                    .map(|balance| balance.value)
                    .map_err(|e| e.to_string()),
            );
        });
        let mut request = http_request(&mut command);
        let body_sent: Value = serde_json::from_slice(&request.operation.body).unwrap();
        assert_eq!(body_sent["method"], "getBalance");
        assert_eq!(
            body_sent["params"],
            json!(["11111111111111111111111111111111", null])
        );
        for (name, value) in [
            ("content-type", "application/json"),
            ("x-api-key", "secret"),
        ] {
            assert!(
                request
                    .operation
                    .headers
                    .iter()
                    .any(|h| h.name == name && h.value == value)
            );
        }
        request
            .resolve(HttpResult::Ok(
                HttpResponse::status(status)
                    .body(body.to_string().into_bytes())
                    .build(),
            ))
            .unwrap();
        let result = command.events().next().unwrap();
        match expected {
            Ok(value) => assert_eq!(result.unwrap(), value),
            Err(message) => assert!(result.unwrap_err().contains(message)),
        }
    }
}

#[test]
fn http_transport_errors_and_response_limit() {
    for response in [
        HttpResult::Err(crux_http::HttpError::Io("offline".into())),
        HttpResult::Ok(
            HttpResponse::ok()
                .body(br#"{"result":42}"#.to_vec())
                .build(),
        ),
    ] {
        let mut command = TestCommand::new(|ctx| async move {
            let client =
                CruxClient::new("https://rpc.example.com", ctx.clone()).with_max_response_size(2);
            ctx.send_event(client.get_slot(None).await.map_err(|e| e.to_string()));
        });
        http_request(&mut command).resolve(response).unwrap();
        assert!(command.events().next().unwrap().is_err());
    }
}

#[test]
fn invalid_inputs_never_reach_the_shell() {
    let mut command = TestCommand::new(|ctx| async move {
        for url in ["not a URL", "file:///tmp/rpc", "wss://rpc.example.com"] {
            assert!(
                CruxClient::new(url, ctx.clone())
                    .get_slot(None)
                    .await
                    .is_err()
            );
        }
        for (name, value) in [("bad header", "ok"), ("x-test", "bad\r\nvalue")] {
            assert!(
                CruxClient::new("https://rpc.example.com", ctx.clone())
                    .with_header(name, value)
                    .get_slot(None)
                    .await
                    .is_err()
            );
        }
        assert!(
            CruxPubsubClient::new("https://rpc.example.com", ctx.clone())
                .slot_subscribe()
                .await
                .is_err()
        );
        #[cfg(feature = "check_address")]
        {
            assert!(
                CruxClient::new("https://rpc.example.com", ctx.clone())
                    .get_balance("bad", None)
                    .await
                    .is_err()
            );
            assert!(
                CruxPubsubClient::new("wss://rpc.example.com", ctx.clone())
                    .account_subscribe("bad", None)
                    .await
                    .is_err()
            );
        }
        ctx.send_event(Ok(1));
    });
    assert!(command.effects().next().is_none());
    assert_eq!(command.events().next(), Some(Ok(1)));
}

#[test]
fn websocket_subscription_notification_and_unsubscribe_round_trip() {
    let mut command = TestCommand::new(|ctx| async move {
        let client = CruxPubsubClient::new("wss://rpc.example.com", ctx.clone());
        let mut subscription = client.clone().slot_subscribe().await.unwrap();
        ctx.send_event(
            subscription
                .next()
                .await
                .unwrap()
                .map(|slot| slot.slot)
                .map_err(|e| e.to_string()),
        );
        ctx.send_event(
            subscription
                .unsubscribe()
                .await
                .map(u64::from)
                .map_err(|e| e.to_string()),
        );
    });
    let mut stream = socket_request(&mut command);
    let WebSocketRequest::Open { id, url, message } = &stream.operation else {
        panic!("expected Open")
    };
    let id = *id;
    assert_eq!(url, "wss://rpc.example.com");
    assert_eq!(
        serde_json::from_str::<Value>(message).unwrap(),
        json!({"jsonrpc":"2.0", "id":1, "method":"slotSubscribe", "params":[]})
    );
    // Ignore unrelated replies and notifications, but don't lose a notification
    // queued immediately after the subscribe acknowledgement.
    for value in [
        json!({"id":99, "result":999}),
        json!({"id":1, "result":7}),
        json!({"params":{"subscription":99,"result":{"slot":999,"parent":0,"root":0}}}),
        json!({"params":{"subscription":7,"result":{"slot":42,"parent":41,"root":40}}}),
    ] {
        stream
            .resolve(WebSocketMessage::Text(value.to_string()))
            .unwrap();
    }
    assert_eq!(command.events().next(), Some(Ok(42)));
    let request = socket_request(&mut command);
    let WebSocketRequest::Send {
        id: sent_id,
        message,
    } = request.operation
    else {
        panic!("expected Send")
    };
    assert_eq!(sent_id, id);
    assert_eq!(
        serde_json::from_str::<Value>(&message).unwrap(),
        json!({"jsonrpc":"2.0","id":2,"method":"slotUnsubscribe","params":[7]})
    );
    assert!(
        command.events().next().is_none(),
        "must await unsubscribe ack"
    );
    stream
        .resolve(WebSocketMessage::Text(
            json!({"id":2,"result":true}).to_string(),
        ))
        .unwrap();
    assert_eq!(command.events().next(), Some(Ok(1)));
    assert_eq!(
        socket_request(&mut command).operation,
        WebSocketRequest::Close { id }
    );
}

#[test]
fn dropping_a_subscription_closes_only_its_socket() {
    let mut command = TestCommand::new(|ctx| async move {
        let client = CruxPubsubClient::new("wss://rpc.example.com", ctx.clone());
        let first = client.root_subscribe().await.unwrap();
        let second = client.root_subscribe().await.unwrap();
        drop(first);
        drop(second);
        ctx.send_event(Ok(1));
    });
    let mut ids = Vec::new();
    // Keep the handles alive, just as a shell with two open sockets would.
    let mut streams = Vec::new();
    for _ in 0..2 {
        let mut stream = socket_request(&mut command);
        let WebSocketRequest::Open { id, .. } = stream.operation else {
            panic!("expected Open")
        };
        ids.push(id);
        stream
            .resolve(WebSocketMessage::Text(r#"{"id":1,"result":7}"#.into()))
            .unwrap();
        streams.push(stream);
    }
    assert_ne!(ids[0], ids[1]);
    let closes: Vec<_> = command
        .effects()
        .map(|effect| match effect {
            Effect::WebSocket(request) => request.operation,
            _ => panic!("unexpected HTTP effect"),
        })
        .collect();
    assert_eq!(
        closes,
        ids.into_iter()
            .map(|id| WebSocketRequest::Close { id })
            .collect::<Vec<_>>()
    );
    assert_eq!(command.events().next(), Some(Ok(1)));
}

#[test]
fn subscription_failures_close_the_socket() {
    for message in [
        WebSocketMessage::Text(
            r#"{"id":1,"error":{"code":-32601,"message":"unsupported"}}"#.into(),
        ),
        WebSocketMessage::Closed,
        WebSocketMessage::Error("offline".into()),
    ] {
        let mut command = TestCommand::new(|ctx| async move {
            let result = CruxPubsubClient::new("wss://rpc.example.com", ctx.clone())
                .root_subscribe()
                .await;
            ctx.send_event(result.map(|_| 1).map_err(|e| e.to_string()));
        });
        let mut stream = socket_request(&mut command);
        let WebSocketRequest::Open { id, .. } = stream.operation else {
            panic!("expected Open")
        };
        stream.resolve(message).unwrap();
        assert!(command.events().next().unwrap().is_err());
        assert_eq!(
            socket_request(&mut command).operation,
            WebSocketRequest::Close { id }
        );
    }
}

#[test]
fn bad_text_frames_do_not_interrupt_subscribe_notifications_or_unsubscribe() {
    let mut command = TestCommand::new(|ctx| async move {
        let mut subscription = CruxPubsubClient::new("wss://rpc.example.com", ctx.clone())
            .root_subscribe()
            .await
            .unwrap();
        ctx.send_event(
            subscription
                .next()
                .await
                .unwrap()
                .map_err(|e| e.to_string()),
        );
        ctx.send_event(
            subscription
                .unsubscribe()
                .await
                .map(u64::from)
                .map_err(|e| e.to_string()),
        );
    });
    let mut stream = socket_request(&mut command);
    let WebSocketRequest::Open { id, .. } = stream.operation else {
        panic!("expected Open")
    };
    for valid in [
        json!({"id":1,"result":7}),
        json!({"params":{"subscription":7,"result":42}}),
        json!({"id":2,"result":true}),
    ] {
        stream
            .resolve(WebSocketMessage::Text("not JSON".into()))
            .unwrap();
        assert!(
            command.events().next().is_none(),
            "bad frame must leave the operation pending"
        );
        assert!(
            command.effects().next().is_none(),
            "bad frame must not close the socket"
        );
        stream
            .resolve(WebSocketMessage::Text(valid.to_string()))
            .unwrap();
        if valid.get("params").is_some() {
            assert_eq!(command.events().next(), Some(Ok(42)));
            assert!(matches!(
                socket_request(&mut command).operation,
                WebSocketRequest::Send { .. }
            ));
        }
    }
    assert_eq!(command.events().next(), Some(Ok(1)));
    assert_eq!(
        socket_request(&mut command).operation,
        WebSocketRequest::Close { id }
    );
}

#[test]
fn disconnect_is_one_error_then_end_of_stream() {
    for (message, expected) in [
        (WebSocketMessage::Closed, "closed"),
        (WebSocketMessage::Error("offline".into()), "offline"),
    ] {
        let mut command = TestCommand::new(|ctx| async move {
            let mut subscription = CruxPubsubClient::new("wss://rpc.example.com", ctx.clone())
                .root_subscribe()
                .await
                .unwrap();
            ctx.send_event(
                subscription
                    .next()
                    .await
                    .unwrap()
                    .map_err(|e| e.to_string()),
            );
            assert!(subscription.next().await.is_none());
        });
        let mut stream = socket_request(&mut command);
        stream
            .resolve(WebSocketMessage::Text(r#"{"id":1,"result":7}"#.into()))
            .unwrap();
        stream.resolve(message).unwrap();
        assert!(
            command
                .events()
                .next()
                .unwrap()
                .unwrap_err()
                .contains(expected)
        );
        assert!(matches!(
            socket_request(&mut command).operation,
            WebSocketRequest::Close { .. }
        ));
    }
}

#[test]
fn cancelling_a_subscription_cleans_up_without_panicking() {
    for acknowledged in [false, true] {
        for combined in [false, true] {
            let command = TestCommand::new(|ctx| async move {
                let mut subscription = CruxPubsubClient::new("wss://rpc.example.com", ctx)
                    .root_subscribe()
                    .await
                    .unwrap();
                let _ = subscription.next().await;
            });
            let mut command = if combined {
                Command::all([command])
            } else {
                command
            };
            let mut stream = socket_request(&mut command);
            let WebSocketRequest::Open { id, .. } = stream.operation else {
                panic!("expected Open")
            };
            if acknowledged {
                stream
                    .resolve(WebSocketMessage::Text(r#"{"id":1,"result":7}"#.into()))
                    .unwrap();
                assert!(command.effects().next().is_none());
            }
            command.abort_handle().abort();
            if combined {
                // Destroying the parent drops the child command's effect channel.
                // The shell detects cancellation when resolving the Open stream.
                assert!(command.effects().next().is_none());
            } else {
                assert_eq!(
                    socket_request(&mut command).operation,
                    WebSocketRequest::Close { id }
                );
            }
            assert!(stream.resolve(WebSocketMessage::Closed).is_err());
            assert!(command.is_done());
        }
    }
}

#[test]
fn oversized_notification_reports_an_error_and_keeps_streaming() {
    let mut command = TestCommand::new(|ctx| async move {
        let mut subscription = CruxPubsubClient::new("wss://rpc.example.com", ctx.clone())
            .root_subscribe()
            .await
            .unwrap();
        for _ in 0..2 {
            ctx.send_event(
                subscription
                    .next()
                    .await
                    .unwrap()
                    .map_err(|e| e.to_string()),
            );
        }
    });
    let mut stream = socket_request(&mut command);
    stream
        .resolve(WebSocketMessage::Text(r#"{"id":1,"result":7}"#.into()))
        .unwrap();
    let valid = r#"{"params":{"subscription":7,"result":42}}"#;
    let at_limit = format!("{}{}", " ".repeat(10 * 1024 * 1024 - valid.len()), valid);
    stream
        .resolve(WebSocketMessage::Text(format!(" {at_limit}")))
        .unwrap();
    let first = command.events().next();
    let unexpected_effects = command.effects().count();
    stream.resolve(WebSocketMessage::Text(at_limit)).unwrap();
    let second = command.events().next();
    assert_eq!(
        unexpected_effects, 0,
        "oversized notification must not close the socket"
    );
    assert!(matches!(first, Some(Err(ref error)) if error.contains("websocket message too large")));
    assert_eq!(
        second,
        Some(Ok(42)),
        "a frame at the limit is still accepted"
    );
}

#[test]
fn oversized_acknowledgements_fail_instead_of_waiting_forever() {
    for unsubscribe in [false, true] {
        let mut command = TestCommand::new(move |ctx| async move {
            let result = match CruxPubsubClient::new("wss://rpc.example.com", ctx.clone())
                .root_subscribe()
                .await
            {
                Ok(subscription) if unsubscribe => subscription.unsubscribe().await.map(u64::from),
                Ok(_) => Ok(1),
                Err(error) => Err(error),
            };
            ctx.send_event(result.map_err(|e| e.to_string()));
        });
        let mut stream = socket_request(&mut command);
        if unsubscribe {
            stream
                .resolve(WebSocketMessage::Text(r#"{"id":1,"result":7}"#.into()))
                .unwrap();
            assert!(matches!(
                socket_request(&mut command).operation,
                WebSocketRequest::Send { .. }
            ));
        }
        let ack = if unsubscribe {
            r#"{"id":2,"result":true}"#
        } else {
            r#"{"id":1,"result":7}"#
        };
        stream
            .resolve(WebSocketMessage::Text(format!(
                "{}{}",
                " ".repeat(10 * 1024 * 1024),
                ack
            )))
            .unwrap();
        let result = command.events().next();
        command.abort_handle().abort();
        assert!(matches!(
            socket_request(&mut command).operation,
            WebSocketRequest::Close { .. }
        ));
        assert!(
            matches!(result, Some(Err(ref error)) if error.contains("websocket message too large"))
        );
    }
}

#[test]
fn dropping_a_command_with_a_pending_or_live_subscription_does_not_panic() {
    for acknowledged in [false, true] {
        for unwinding in [false, true] {
            let mut command = TestCommand::new(|ctx| async move {
                let mut subscription = CruxPubsubClient::new("wss://rpc.example.com", ctx)
                    .root_subscribe()
                    .await
                    .unwrap();
                let _ = subscription.next().await;
            });
            let mut stream = socket_request(&mut command);
            if acknowledged {
                stream
                    .resolve(WebSocketMessage::Text(r#"{"id":1,"result":7}"#.into()))
                    .unwrap();
                assert!(command.effects().next().is_none());
            }
            if unwinding {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _command = command;
                    panic!("original panic");
                }));
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"original panic")
                );
            } else {
                drop(command);
            }
            assert!(
                stream.resolve(WebSocketMessage::Closed).is_err(),
                "shell must see that its stream was cancelled"
            );
        }
    }
}
