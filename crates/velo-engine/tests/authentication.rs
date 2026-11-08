use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{Router, extract::Request, routing::get};
use velo_engine::{ApiKeyPlacement, NameValue, RequestAuth, RunConfig, RunManager, TestDefinition};

#[tokio::test]
async fn api_key_header_and_query_use_shared_transport_once() {
    for placement in [ApiKeyPlacement::Header, ApiKeyPlacement::Query] {
        let count = Arc::new(AtomicUsize::new(0));
        let requests = count.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/",
                    get(move |request: Request| {
                        let count = requests.clone();
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            match placement {
                                ApiKeyPlacement::Header => {
                                    assert_eq!(request.headers()["x-api-key"], "secret/value")
                                }
                                ApiKeyPlacement::Query => assert_eq!(
                                    request.uri().query(),
                                    Some("x-api-key=secret%2Fvalue")
                                ),
                            }
                            "echo: secret/value"
                        }
                    }),
                ),
            )
            .await
            .unwrap();
        });
        let response = velo_engine::preview_request(
            RunConfig {
                url: format!("http://{address}/"),
                auth: RequestAuth::ApiKey {
                    name: "x-api-key".into(),
                    value: "secret/value".into(),
                    placement,
                },
                ..Default::default()
            },
            &[],
        )
        .await
        .unwrap();
        assert_eq!(response.status, 200);
        assert!(!response.body.contains("secret/value"));
        assert!(response.body.contains("[redacted]"));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        server.abort();
        let _ = server.await;
    }
}

#[test]
fn api_keys_reject_ambiguous_fields_before_execution_and_mask_debug() {
    for (placement, headers, query, url) in [
        (
            ApiKeyPlacement::Header,
            vec![NameValue {
                name: "X-Api-Key".into(),
                value: "second".into(),
            }],
            vec![],
            "https://example.com/",
        ),
        (
            ApiKeyPlacement::Query,
            vec![],
            vec![NameValue {
                name: "key".into(),
                value: "second".into(),
            }],
            "https://example.com/",
        ),
        (
            ApiKeyPlacement::Query,
            vec![],
            vec![],
            "https://example.com/?key=second",
        ),
    ] {
        let auth = RequestAuth::ApiKey {
            name: if placement == ApiKeyPlacement::Header {
                "x-api-key".into()
            } else {
                "key".into()
            },
            value: "PRIVATE_API_KEY".into(),
            placement,
        };
        assert!(!format!("{auth:?}").contains("PRIVATE_API_KEY"));
        let error = RunManager::prepare(RunConfig {
            url: url.into(),
            auth,
            headers,
            query,
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(error.field.as_deref(), Some("auth"));
        assert!(!error.message.contains("PRIVATE_API_KEY"));
    }
}

#[test]
fn functional_resolution_preserves_session_fields_and_binds_api_key_secrets() {
    let definition = TestDefinition {
        config: RunConfig {
            url: "{{origin}}/item/{{session:item}}".into(),
            auth: RequestAuth::ApiKey {
                name: "x-api-key".into(),
                value: "{{secret:api-key}}".into(),
                placement: ApiKeyPlacement::Header,
            },
            ..Default::default()
        },
        variables: BTreeMap::from([("origin".into(), "https://example.com".into())]),
        ..Default::default()
    };
    let config = definition
        .resolve_api(None, &BTreeMap::new(), |name| {
            assert_eq!(name, "api-key");
            Ok("PRIVATE_API_KEY".into())
        })
        .unwrap();
    assert_eq!(config.url, "https://example.com/item/{{session:item}}");
    let RequestAuth::ApiKey { value, .. } = config.auth else {
        panic!("API key was lost")
    };
    assert_eq!(value, "PRIVATE_API_KEY");
    assert_eq!(
        definition
            .resolve(None, |_| Ok("PRIVATE_API_KEY".into()))
            .unwrap_err()
            .code,
        "VARIABLE_UNRESOLVED"
    );
}
