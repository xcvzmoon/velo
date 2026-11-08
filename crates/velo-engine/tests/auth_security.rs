use std::collections::BTreeMap;

use axum::{Router, routing::get};
use velo_engine::{
    ApiCollection, ApiKeyPlacement, CollectionDefaults, CollectionEntry, NameValue, RequestAuth,
    RequestBody, RequestMethod, RunConfig, RunManager, TestDefinition,
};

async fn response_body(auth: RequestAuth, body: String) -> velo_engine::PreviewResponse {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/",
                get(move || {
                    let body = body.clone();
                    async move { body }
                }),
            ),
        )
        .await
        .unwrap();
    });
    let response = velo_engine::preview_request(
        RunConfig {
            url: format!("http://{address}/"),
            auth,
            ..Default::default()
        },
        &[],
    )
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    response
}

#[tokio::test]
async fn basic_credentials_are_redacted_when_response_contains_only_the_base64_payload() {
    let response = response_body(
        RequestAuth::Basic {
            username: "username".into(),
            password: "password".into(),
        },
        "dXNlcm5hbWU6cGFzc3dvcmQ=".into(),
    )
    .await;
    assert!(!response.body.contains("dXNlcm5hbWU6cGFzc3dvcmQ="));
    assert!(response.redacted);
}

#[tokio::test]
async fn truncated_literal_auth_responses_do_not_expose_partial_credentials() {
    for auth in [
        RequestAuth::Basic {
            username: "username".into(),
            password: "CREDENTIALFRAGMENT".into(),
        },
        RequestAuth::Bearer {
            token: "CREDENTIALFRAGMENT".into(),
        },
        RequestAuth::ApiKey {
            name: "x-api-key".into(),
            value: "CREDENTIALFRAGMENT".into(),
            placement: ApiKeyPlacement::Header,
        },
        RequestAuth::ApiKey {
            name: "key".into(),
            value: "CREDENTIALFRAGMENT".into(),
            placement: ApiKeyPlacement::Query,
        },
    ] {
        let body = format!("{}CREDENTIALFRAGMENT", "x".repeat(65_530));
        let response = response_body(auth, body).await;
        assert!(response.truncated);
        assert!(response.redacted);
        assert!(!response.body.contains("CREDEN"));
        assert!(response.body.contains("withheld"));
    }
}

#[test]
fn ambiguous_auth_headers_and_api_key_reserved_names_are_rejected_before_traffic() {
    for name in ["authorization", "proxy-authorization", "cookie"] {
        let error = RunManager::prepare(RunConfig {
            url: "https://example.com".into(),
            headers: vec![
                pair(name, "first"),
                pair(&name.to_ascii_uppercase(), "second"),
            ],
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(error.field.as_deref(), Some("headers"));
        assert!(!error.to_string().contains("first"));
        assert!(
            RunManager::prepare(RunConfig {
                url: "https://example.com".into(),
                auth: RequestAuth::ApiKey {
                    name: name.into(),
                    value: "private".into(),
                    placement: ApiKeyPlacement::Header
                },
                ..Default::default()
            })
            .is_err()
        );
    }
}

fn pair(name: &str, value: &str) -> NameValue {
    NameValue {
        name: name.into(),
        value: value.into(),
    }
}

fn collection(config: RunConfig, variables: BTreeMap<String, String>) -> ApiCollection {
    ApiCollection {
        entries: vec![CollectionEntry::Request {
            id: "request".into(),
            name: "Request".into(),
            description: String::new(),
            notes: String::new(),
            definition: Box::new(TestDefinition {
                config,
                variables,
                ..Default::default()
            }),
            inherit_auth: true,
            extractions: vec![],
        }],
        ..Default::default()
    }
}

#[test]
fn collection_sensitive_dynamic_names_cannot_hide_literal_credentials() {
    for config in [
        RunConfig {
            headers: vec![pair("{{field}}", "RAW_SECRET")],
            ..Default::default()
        },
        RunConfig {
            query: vec![pair("{{field}}", "RAW_SECRET")],
            ..Default::default()
        },
        RunConfig {
            method: RequestMethod::POST,
            body: RequestBody::Json(r#"{"{{field}}":"RAW_SECRET"}"#.into()),
            ..Default::default()
        },
    ] {
        let document = collection(config, [("field".into(), "authorization".into())].into());
        assert!(document.portable().is_err());
    }
    let document = collection(
        RunConfig {
            headers: vec![pair("{{field}}", "application/json")],
            ..Default::default()
        },
        [("field".into(), "Accept".into())].into(),
    );
    assert!(document.portable().is_ok());
}

#[test]
fn environment_or_unknown_name_bindings_cannot_hide_literal_credentials() {
    let mut document = collection(
        RunConfig {
            headers: vec![pair("{{field}}", "RAW_SECRET")],
            ..Default::default()
        },
        [("field".into(), "Accept".into())].into(),
    );
    document.environments.insert(
        "production".into(),
        [("field".into(), "Authorization".into())].into(),
    );
    assert!(document.portable().is_err());
    document.environments.clear();
    document.defaults.variables.clear();
    if let CollectionEntry::Request { definition, .. } = &mut document.entries[0] {
        definition.variables.clear();
    }
    assert!(document.portable().is_err());
}

#[test]
fn json_payload_variables_and_empty_collection_defaults_cannot_embed_credentials() {
    let document = collection(
        RunConfig {
            method: RequestMethod::POST,
            body: RequestBody::Json("{{payload}}".into()),
            ..Default::default()
        },
        [("payload".into(), r#"{"password":"RAW_SECRET"}"#.into())].into(),
    );
    assert!(document.portable().is_err());
    let document = ApiCollection {
        defaults: CollectionDefaults {
            auth: Some(RequestAuth::Bearer {
                token: "{{alias}}".into(),
            }),
            variables: [("alias".into(), "RAW_SECRET".into())].into(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(document.portable().is_err());
}

#[test]
fn remote_auth_requires_https_and_only_literal_loopback_allows_cleartext() {
    let mut authentications = vec![
        (
            RequestAuth::Basic {
                username: "user".into(),
                password: "private".into(),
            },
            vec![],
        ),
        (
            RequestAuth::Bearer {
                token: "private".into(),
            },
            vec![],
        ),
        (
            RequestAuth::ApiKey {
                name: "key".into(),
                value: "private".into(),
                placement: ApiKeyPlacement::Query,
            },
            vec![],
        ),
        (
            RequestAuth::ApiKey {
                name: "x-api-key".into(),
                value: "private".into(),
                placement: ApiKeyPlacement::Header,
            },
            vec![],
        ),
    ];
    for name in ["Authorization", "Proxy-Authorization", "Cookie"] {
        authentications.push((RequestAuth::None, vec![pair(name, "private")]));
    }
    for (auth, headers) in authentications {
        for url in [
            "http://example.com/",
            "http://10.0.0.1/",
            "http://localhost/",
            "http://127.0.0.1.example.com/",
        ] {
            let error = RunManager::prepare(RunConfig {
                url: url.into(),
                auth: auth.clone(),
                headers: headers.clone(),
                ..Default::default()
            })
            .unwrap_err();
            assert_eq!(error.code, "INSECURE_AUTH_TARGET");
            assert_eq!(error.field.as_deref(), Some("url"));
            assert!(!error.to_string().contains("private"));
        }
        for url in ["https://example.com/", "http://127.0.0.1/", "http://[::1]/"] {
            assert!(
                RunManager::prepare(RunConfig {
                    url: url.into(),
                    auth: auth.clone(),
                    headers: headers.clone(),
                    ..Default::default()
                })
                .is_ok()
            );
        }
    }
    assert!(
        RunManager::prepare(RunConfig {
            url: "http://example.com/".into(),
            ..Default::default()
        })
        .is_ok()
    );
}

#[tokio::test]
async fn functional_session_cookie_policy_failure_keeps_its_stable_error_without_sending() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let received = calls.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/",
                get(move || {
                    let received = received.clone();
                    async move {
                        received.fetch_add(1, Ordering::SeqCst);
                        ([("set-cookie", "session=PRIVATE_COOKIE; Path=/")], "ok")
                    }
                }),
            ),
        )
        .await
        .unwrap();
    });
    let config = RunConfig {
        url: format!("http://localhost:{port}/"),
        ..Default::default()
    };
    let mut session = velo_engine::ApiSession::default();
    let cancel = velo_engine::ApiCancellation::default();
    velo_engine::send_api_request(config.clone(), &[], &[], &mut session, &cancel)
        .await
        .unwrap();
    assert_eq!(session.view().cookies.len(), 1);
    let error = velo_engine::send_api_request(config, &[], &[], &mut session, &cancel)
        .await
        .unwrap_err();
    assert_eq!(error.code, "INSECURE_AUTH_TARGET");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!error.to_string().contains("PRIVATE_COOKIE"));
    server.abort();
    let _ = server.await;
}

#[test]
fn shared_definition_credential_validation_covers_root_and_scenario_aliases() {
    use velo_engine::{
        Scenario, ScenarioPlan, ScenarioRequest, ScenarioStep, StepFailure,
        validate_definition_credentials,
    };
    fn step(auth: RequestAuth) -> ScenarioStep {
        ScenarioStep {
            id: "step".into(),
            request: ScenarioRequest {
                url: "https://example.com".into(),
                method: RequestMethod::GET,
                headers: vec![],
                query: vec![],
                body: RequestBody::None,
                auth,
                assertions: vec![],
            },
            extractions: vec![],
            think_ms: 0,
            on_failure: StepFailure::Stop,
        }
    }
    let reference = RequestAuth::Bearer {
        token: "{{alias}}".into(),
    };
    let mut definition = TestDefinition {
        config: RunConfig {
            auth: reference.clone(),
            ..Default::default()
        },
        variables: [("alias".into(), "{{secret:api-token}}".into())].into(),
        ..Default::default()
    };
    assert!(validate_definition_credentials(&definition).is_ok());
    definition.environments.insert(
        "production".into(),
        [("alias".into(), "RAW_SECRET".into())].into(),
    );
    assert_eq!(
        validate_definition_credentials(&definition)
            .unwrap_err()
            .code,
        "DEFINITION_SECRET_REQUIRED"
    );
    definition.config.auth = RequestAuth::None;
    for phase in 0..3 {
        definition.config.scenarios = Some(ScenarioPlan {
            setup: vec![step(if phase == 0 {
                reference.clone()
            } else {
                RequestAuth::None
            })],
            scenarios: vec![Scenario {
                id: "journey".into(),
                weight: 1,
                steps: vec![step(if phase == 1 {
                    reference.clone()
                } else {
                    RequestAuth::None
                })],
            }],
            cleanup: vec![step(if phase == 2 {
                reference.clone()
            } else {
                RequestAuth::None
            })],
            ..Default::default()
        });
        let error = validate_definition_credentials(&definition).unwrap_err();
        assert_eq!(error.code, "DEFINITION_SECRET_REQUIRED");
        assert!(!error.to_string().contains("RAW_SECRET"));
        let mut safe = definition.clone();
        safe.environments.clear();
        assert!(validate_definition_credentials(&safe).is_ok());
    }
}

#[test]
fn placeholder_origins_do_not_hide_literal_query_credentials_from_definition_exports() {
    let mut definition = TestDefinition::default();
    definition.config.url = "{{origin}}/?password=RAW_SECRET".into();
    let error = velo_engine::validate_definition_credentials(&definition).unwrap_err();
    assert_eq!(error.code, "DEFINITION_SECRET_REQUIRED");
    definition.config.url = "{{origin}}/?password={{secret:password}}".into();
    assert!(velo_engine::validate_definition_credentials(&definition).is_ok());
}

#[test]
fn url_credential_templates_and_origin_variables_are_rejected_before_persistence() {
    let mut definition = TestDefinition::default();
    definition.config.url = "https://user:RAW_SECRET@{{host}}/".into();
    assert_eq!(
        velo_engine::validate_definition_credentials(&definition)
            .unwrap_err()
            .code,
        "INVALID_DEFINITION_CREDENTIALS"
    );
    definition.config.url = "{{origin}}/".into();
    definition.variables.insert(
        "origin".into(),
        "https://user:RAW_SECRET@example.com".into(),
    );
    assert_eq!(
        velo_engine::validate_definition_credentials(&definition)
            .unwrap_err()
            .code,
        "INVALID_DEFINITION_CREDENTIALS"
    );
}

#[test]
fn literal_dataset_credentials_cannot_hide_behind_copied_auth_aliases() {
    use velo_engine::{
        DataAllocation, DataDefinition, DataExhaustion, DataField, DataGenerator, DataLifetime,
        DataSource, validate_definition_credentials,
    };
    let mut definition = TestDefinition {
        config: RunConfig {
            auth: RequestAuth::Bearer {
                token: "{{authInput}}".into(),
            },
            ..Default::default()
        },
        dataset: Some(DataDefinition {
            lifetime: DataLifetime::Iteration,
            allocation: DataAllocation::Sequential,
            exhaustion: DataExhaustion::Stop,
            seed: 0,
            source: DataSource::Faker {
                version: "10.6.0".into(),
                locale: "en".into(),
                count: 1,
                fields: vec![
                    DataField {
                        name: "rawValue".into(),
                        generator: DataGenerator::Literal {
                            value: "RAW_SECRET".into(),
                        },
                    },
                    DataField {
                        name: "authInput".into(),
                        generator: DataGenerator::Copy {
                            field: "rawValue".into(),
                        },
                    },
                ],
            },
        }),
        ..Default::default()
    };
    assert_eq!(
        validate_definition_credentials(&definition)
            .unwrap_err()
            .code,
        "DEFINITION_SECRET_REQUIRED"
    );
    if let Some(DataDefinition {
        source: DataSource::Faker { fields, .. },
        ..
    }) = &mut definition.dataset
    {
        fields[0].generator = DataGenerator::Uuid;
    }
    assert!(validate_definition_credentials(&definition).is_ok());
}

#[test]
fn manual_credential_headers_and_query_fields_cannot_bypass_the_https_policy() {
    for (header, query, url) in [
        (Some("X-Api-Key"), None, "http://example.com/"),
        (Some("X-Access-Token"), None, "http://example.com/"),
        (None, Some("access_token"), "http://example.com/"),
        (None, None, "http://example.com/?api_key=private"),
    ] {
        let mut config = RunConfig {
            url: url.into(),
            ..Default::default()
        };
        if let Some(name) = header {
            config.headers.push(NameValue {
                name: name.into(),
                value: "private".into(),
            });
        }
        if let Some(name) = query {
            config.query.push(NameValue {
                name: name.into(),
                value: "private".into(),
            });
        }
        let error = RunManager::prepare(config).unwrap_err();
        assert_eq!(error.code, "INSECURE_AUTH_TARGET");
        assert!(!error.to_string().contains("private"));
    }
    assert!(
        RunManager::prepare(RunConfig {
            url: "http://example.com/?page=1".into(),
            ..Default::default()
        })
        .is_ok()
    );
}
