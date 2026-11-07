use std::collections::BTreeMap;

use serde_json::json;
use velo_engine::{
    ImportFormat, NameValue, RequestAuth, RequestBody, RequestMethod, RunConfig, RunManager,
    TestDefinition, import_requests,
};

fn definition() -> TestDefinition {
    TestDefinition {
        config: RunConfig {
            url: "{{base}}/items/{{id}}?q={{query}}".into(),
            method: RequestMethod::POST,
            headers: vec![
                NameValue {
                    name: "x-value".into(),
                    value: "{{secret:token}}".into(),
                },
                NameValue {
                    name: "x-value".into(),
                    value: "second".into(),
                },
            ],
            query: vec![
                NameValue {
                    name: "q".into(),
                    value: "{{query}}".into(),
                },
                NameValue {
                    name: "q".into(),
                    value: "again".into(),
                },
            ],
            body: RequestBody::Json(r#"{"value":"{{query}}","{{id}}":"{{secret:token}}"}"#.into()),
            ..Default::default()
        },
        variables: BTreeMap::from([
            ("base".into(), "https://example.com".into()),
            ("id".into(), "shared".into()),
            ("query".into(), "a\"b&x=1/é".into()),
        ]),
        environments: BTreeMap::from([(
            "staging".into(),
            BTreeMap::from([("id".into(), "a/b".into())]),
        )]),
        ..Default::default()
    }
}

#[test]
fn portable_round_trip_preserves_engine_settings_and_never_resolves_secrets() {
    let definition = definition();
    let source = serde_json::to_string(&definition).unwrap();
    let parsed = TestDefinition::parse(&source).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        serde_json::to_value(&definition).unwrap()
    );
    let config = parsed
        .resolve(Some("staging"), |_| Ok("PRIVATE_TOKEN".into()))
        .unwrap();
    assert!(!source.contains("PRIVATE_TOKEN"));
    assert_eq!(
        config.url,
        "https://example.com/items/a%2Fb?q=a%22b%26x%3D1%2F%C3%A9"
    );
    assert_eq!(config.headers[0].value, "PRIVATE_TOKEN");
    assert_eq!(config.headers[1].value, "second");
    assert_eq!(config.query[0].value, "a\"b&x=1/é");
    let RequestBody::Json(body) = &config.body else {
        panic!("JSON body lost")
    };
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["value"], "a\"b&x=1/é");
    assert_eq!(body["a/b"], "PRIVATE_TOKEN");
    RunManager::prepare(config).unwrap();
}

#[test]
fn missing_references_and_unsupported_versions_fail_without_input_values() {
    let mut definition = definition();
    definition.variables.remove("base");
    let error = definition
        .resolve(None, |_| Ok("PRIVATE_TOKEN".into()))
        .unwrap_err();
    assert_eq!(error.code, "VARIABLE_UNRESOLVED");
    assert_eq!(error.field.as_deref(), Some("url"));
    assert!(!format!("{definition:?}").contains("example.com"));
    assert_eq!(
        TestDefinition::parse(r#"{"version":2}"#).unwrap_err().code,
        "UNSUPPORTED_TEST_VERSION"
    );
    assert_eq!(
        TestDefinition::parse("invalid PRIVATE_TOKEN")
            .unwrap_err()
            .code,
        "INVALID_TEST_DEFINITION"
    );
}

#[test]
fn all_body_modes_and_workloads_round_trip_with_ordered_pairs() {
    let bodies = vec![
        RequestBody::None,
        RequestBody::Text("{{value}}".into()),
        RequestBody::Json("{\"a\":\"{{value}}\"}".into()),
        RequestBody::Form(vec![
            NameValue {
                name: "a".into(),
                value: "1".into(),
            },
            NameValue {
                name: "a".into(),
                value: "2".into(),
            },
        ]),
        RequestBody::Multipart(vec![velo_engine::MultipartPart::Text {
            name: "a".into(),
            value: "{{value}}".into(),
        }]),
    ];
    let workloads = vec![
        velo_engine::Workload::Constant {
            vus: 2,
            duration_ms: 5000,
        },
        velo_engine::Workload::Ramping {
            start_vus: 0,
            stages: vec![velo_engine::RampStage {
                duration_ms: 1000,
                target_vus: 3,
            }],
        },
        velo_engine::Workload::Arrival {
            rate: 10,
            capacity: 3,
            duration_ms: 5000,
        },
    ];
    for workload in workloads {
        for body in &bodies {
            let definition = TestDefinition {
                config: RunConfig {
                    body: body.clone(),
                    workload: workload.clone(),
                    thresholds: vec![velo_engine::Threshold {
                        metric: velo_engine::ThresholdMetric::ResponseP95Ms,
                        operator: velo_engine::ThresholdOperator::LessThan,
                        limit: 500.0,
                    }],
                    ..Default::default()
                },
                ..Default::default()
            };
            let value = serde_json::to_value(&definition).unwrap();
            assert_eq!(
                serde_json::to_value(TestDefinition::parse(&value.to_string()).unwrap()).unwrap(),
                value
            );
        }
    }
}

#[test]
fn curl_is_offline_and_rejects_executable_syntax_and_unsupported_flags() {
    let requests = import_requests(ImportFormat::Curl, "curl -X POST -H 'x-a: one' -H 'x-a: two' --data-raw '$(touch /tmp/never)' -u user:pass --max-time 1.5 https://example.com").unwrap();
    assert_eq!(requests[0].config.headers.len(), 3);
    assert_eq!(requests[0].config.timeout_ms, 1500);
    assert!(
        matches!(&requests[0].config.body, RequestBody::Text(value) if value == "$(touch /tmp/never)")
    );
    assert!(
        matches!(&requests[0].config.auth, RequestAuth::Basic { username, .. } if username == "user")
    );
    for command in [
        "curl https://example.com ; touch file",
        "curl -k https://example.com",
        "curl -d @/private/file https://example.com",
        "curl --retry 3 https://example.com",
    ] {
        assert_eq!(
            import_requests(ImportFormat::Curl, command)
                .unwrap_err()
                .code,
            "REQUEST_IMPORT_FAILED"
        );
    }
}

#[test]
fn postman_operations_retain_duplicates_and_report_inherited_scripts_and_auth() {
    let collection = json!({ "info": {"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"}, "auth": {"type":"oauth2"}, "event": [{"script":{"exec":["throw new Error('never')"]}}], "item": [{"name":"folder", "event":[], "item":[{"name":"one", "request":{"method":"GET", "url":{"raw":"https://example.com?q=one&q=two", "query":[{"key":"q","value":"one"},{"key":"q","value":"two"}]}, "header":[{"key":"x-a","value":"1"},{"key":"x-a","value":"2"}]}}]}, {"name":"two", "request":{"method":"POST", "url":"https://example.com", "body":{"mode":"raw","raw":"{}","options":{"raw":{"language":"json"}}}}}] });
    let requests = import_requests(ImportFormat::Postman, &collection.to_string()).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].config.query.len(), 2);
    assert_eq!(requests[0].config.headers.len(), 2);
    assert!(
        requests[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("auth"))
    );
    assert!(
        requests[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("event"))
    );
    assert!(matches!(requests[1].config.body, RequestBody::Json(_)));
}

#[test]
fn openapi_lists_operations_and_reports_security_references_and_bodies() {
    let source = json!({"openapi":"3.1.0", "servers":[{"url":"https://example.com"}], "security":[{"oauth":[]}], "components":{}, "paths":{"/items/{id}":{"parameters":[{"name":"id","in":"path","required":true}], "get":{"parameters":[{"$ref":"https://never.example/parameter"}]}, "post":{"requestBody":{"$ref":"#/components/schemas/Input"}, "security":[{"key":[]}]}}}});
    let requests = import_requests(ImportFormat::Openapi, &source.to_string()).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].config.url, "https://example.com/items/{{id}}");
    assert!(
        requests[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("security"))
    );
    assert!(
        requests[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("$ref"))
    );
    assert!(
        requests[1]
            .warnings
            .iter()
            .any(|warning| warning.contains("body"))
    );
}

#[tokio::test]
async fn preview_uses_redirect_policy_captures_failures_and_bounds_redacted_content() {
    use axum::{Router, http::StatusCode, response::Redirect, routing::get};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let app = Router::new()
        .route(
            "/redirect",
            get(|| async { Redirect::temporary("/response") }),
        )
        .route(
            "/response",
            get(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async {
                    (
                        StatusCode::BAD_REQUEST,
                        [
                            ("set-cookie", "server-private"),
                            ("x-echo", "PRIVATE_TOKEN"),
                        ],
                        format!("PRIVATE_TOKEN {}", "x".repeat(80_000)),
                    )
                }
            }),
        )
        .route(
            "/echo",
            get(|| async { ([("private_token", "public")], "PRIVATE_TOKEN") }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                "slow"
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let config = RunConfig {
        url: format!("http://{address}/redirect"),
        auth: RequestAuth::Bearer {
            token: "PRIVATE_TOKEN".into(),
        },
        ..Default::default()
    };
    let response = velo_engine::preview_request(config.clone(), &[])
        .await
        .unwrap();
    assert_eq!(response.status, 307);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let response = velo_engine::preview_request(
        RunConfig {
            max_redirects: 1,
            ..config.clone()
        },
        &[],
    )
    .await
    .unwrap();
    assert_eq!(response.status, 400);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(response.truncated);
    assert!(response.redacted);
    assert!(response.body.len() <= 65_536);
    let encoded = serde_json::to_string(&response).unwrap();
    assert!(!encoded.contains("PRIVATE_TOKEN"));
    assert!(!encoded.contains("server-private"));
    assert!(!format!("{response:?}").contains("PRIVATE_TOKEN"));
    let secret = "PRIVATE_TOKEN".to_owned();
    let response = velo_engine::preview_request(
        RunConfig {
            url: format!("http://{address}/echo"),
            headers: vec![NameValue {
                name: "x-private".into(),
                value: format!("prefix-{secret}-suffix"),
            }],
            ..Default::default()
        },
        std::slice::from_ref(&secret),
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    assert!(response.redacted);
    assert!(!response.body.contains(&secret));
    let response = velo_engine::preview_request(
        RunConfig {
            url: format!("http://{address}/echo"),
            ..Default::default()
        },
        &["private_token".into()],
    )
    .await
    .unwrap();
    assert!(
        response
            .headers
            .iter()
            .any(|pair| pair.name == "[redacted]")
    );
    let response = velo_engine::preview_request(
        RunConfig {
            max_redirects: 1,
            ..config.clone()
        },
        std::slice::from_ref(&secret),
    )
    .await
    .unwrap();
    assert!(response.truncated);
    assert!(response.body.contains("withheld"));
    assert!(!response.body.contains("PRIVATE"));
    let error = velo_engine::preview_request(
        RunConfig {
            url: format!("http://{address}/slow"),
            timeout_ms: 10,
            ..config
        },
        &[],
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "PREVIEW_TIMEOUT");
    server.abort();
    let _ = server.await;
}

#[test]
fn resolution_errors_name_fields_and_cannot_expand_without_a_bound() {
    let mut definition = definition();
    let error = definition
        .resolve(None, |_| {
            Err(RunManager::prepare(Default::default()).unwrap_err())
        })
        .unwrap_err();
    assert_eq!(error.code, "SECRET_UNAVAILABLE");
    assert_eq!(error.field.as_deref(), Some("headers.0.value"));
    definition
        .variables
        .insert("base".into(), "{{other}}".into());
    assert_eq!(
        definition
            .resolve(None, |_| Ok("token".into()))
            .unwrap_err()
            .code,
        "INVALID_TEST_DEFINITION"
    );
    let mut value = serde_json::to_value(TestDefinition::default()).unwrap();
    value["config"]["timeout_ms"] = json!("invalid");
    let error = TestDefinition::parse(&value.to_string()).unwrap_err();
    assert_eq!(error.field.as_deref(), Some("config.timeout_ms"));
}

#[test]
fn environment_secret_references_and_arbitrary_secret_bytes_are_not_reinterpreted() {
    let mut definition = definition();
    definition.config.auth = RequestAuth::Basic {
        username: "{{username}}".into(),
        password: "{{password}}".into(),
    };
    definition
        .variables
        .insert("username".into(), "user".into());
    definition
        .variables
        .insert("password".into(), "{{secret:password}}".into());
    let config = definition
        .resolve(None, |_| Ok("secret-{{literal}}-bytes".into()))
        .unwrap();
    assert!(
        matches!(config.auth, RequestAuth::Basic { password, .. } if password == "secret-{{literal}}-bytes")
    );
}

#[test]
fn checked_in_example_round_trips_and_prepares_with_local_secret_binding() {
    let source = include_str!("../../../docs/examples/velo.test.json");
    let definition = TestDefinition::parse(source).unwrap();
    for environment in [None, Some("staging")] {
        let config = definition
            .resolve(environment, |_| Ok("test-token".into()))
            .unwrap();
        RunManager::prepare(config).unwrap();
    }
}

fn scenario_definition() -> TestDefinition {
    let mut definition = TestDefinition::default();
    definition.config.url = "https://example.com/".into();
    definition.variables = BTreeMap::from([
        ("base".into(), "https://example.com".into()),
        ("customer".into(), "shared".into()),
    ]);
    definition.environments = BTreeMap::from([(
        "staging".into(),
        BTreeMap::from([("customer".into(), "environment".into())]),
    )]);
    definition.config.scenarios = Some(serde_json::from_value(json!({
        "scenarios": [{"id": "checkout", "weight": 1, "steps": [{
            "id": "order", "on_failure": "stop", "request": {
                "url": "{{base}}/orders/{{session:id}}", "method": "POST",
                "headers": [{"name": "x-customer", "value": "{{customer}}"}],
                "query": [], "body": {"kind": "json", "value": "{\"token\":\"{{session:token}}\",\"customer\":\"{{customer}}\"}"},
                "auth": {"kind": "bearer", "token": "{{secret:token}}"}
            }
        }]}]
    })).unwrap());
    definition
}

#[test]
fn scenario_static_resolution_preserves_session_tokens_and_environment_precedence() {
    let definition = scenario_definition();
    let config = definition
        .resolve(Some("staging"), |_| Ok("PRIVATE_TOKEN".into()))
        .unwrap();
    let plan = config.scenarios.unwrap();
    let request = &plan.scenarios[0].steps[0].request;
    assert_eq!(request.url, "https://example.com/orders/{{session:id}}");
    assert_eq!(request.headers[0].value, "environment");
    let RequestBody::Json(body) = &request.body else {
        panic!("expected JSON")
    };
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["token"], "{{session:token}}");
    assert_eq!(body["customer"], "environment");
    assert!(
        !serde_json::to_string(&definition)
            .unwrap()
            .contains("PRIVATE_TOKEN")
    );
}

#[test]
fn scenario_binding_rejects_reference_injection_and_reserved_namespaces() {
    let mut definition = scenario_definition();
    definition
        .variables
        .insert("customer".into(), "{{session:injected}}".into());
    assert_eq!(
        definition
            .resolve(None, |_| Ok("PRIVATE_TOKEN".into()))
            .unwrap_err()
            .code,
        "INVALID_TEST_DEFINITION"
    );
    definition
        .variables
        .insert("session:token".into(), "spoofed".into());
    assert_eq!(
        definition
            .resolve(None, |_| Ok("PRIVATE_TOKEN".into()))
            .unwrap_err()
            .field
            .as_deref(),
        Some("variables")
    );
}

#[test]
fn scenario_dataset_records_resolve_independently_and_keep_journeys() {
    let mut definition = scenario_definition();
    definition.dataset = Some(
        serde_json::from_value(json!({
            "lifetime": "iteration", "allocation": "sequential", "exhaustion": "stop", "seed": 0,
            "source": {"kind": "json", "mapping": {"customer": "customer"}}
        }))
        .unwrap(),
    );
    let rows = vec![
        BTreeMap::from([("customer".into(), "first".into())]),
        BTreeMap::from([("customer".into(), "second".into())]),
    ];
    let config = definition
        .resolve_data(
            Some("staging"),
            &rows,
            || false,
            |_| Ok("PRIVATE_TOKEN".into()),
        )
        .unwrap();
    let data = config.data.unwrap();
    assert_eq!(
        data.requests[0].scenarios.as_ref().unwrap().scenarios[0].steps[0]
            .request
            .headers[0]
            .value,
        "first"
    );
    assert_eq!(
        data.requests[1].scenarios.as_ref().unwrap().scenarios[0].steps[0]
            .request
            .headers[0]
            .value,
        "second"
    );
}
