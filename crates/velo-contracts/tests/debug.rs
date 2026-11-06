use serde_json::json;
use velo_contracts::{DiagnosticInput, SavedConfiguration};

#[test]
fn loaded_configuration_debug_omits_request_content_and_user_name() {
    let mut saved = SavedConfiguration {
        id: "private-id-sentinel".into(),
        name: "private-name-sentinel".into(),
        remembered: true,
        config: json!({
            "url": "https://example.com/private-url-sentinel",
            "headers": [{"name": "authorization", "value": "private-header-sentinel"}],
            "query": [{"name": "key", "value": "private-query-sentinel"}],
            "auth": {"kind": "bearer", "token": "private-token-sentinel"},
            "body": {"kind": "text", "value": "private-body-sentinel"}
        }),
    };
    for auth in [
        json!({"kind": "bearer", "token": "private-token-sentinel"}),
        json!({"kind": "basic", "username": "private-user-sentinel", "password": "private-password-sentinel"}),
    ] {
        saved.config["auth"] = auth.clone();
        for output in [format!("{saved:?}"), format!("{saved:#?}")] {
            assert!(
                !output.contains("private-"),
                "sensitive debug output: {output}"
            );
        }
        assert_eq!(
            serde_json::to_value(&saved).unwrap()["config"]["auth"],
            auth
        );
    }
}

#[test]
fn diagnostic_debug_omits_the_untrusted_label() {
    let input = DiagnosticInput {
        label: "private-label-sentinel".into(),
    };
    for output in [format!("{input:?}"), format!("{input:#?}")] {
        assert!(!output.contains("private-label-sentinel"));
    }
    assert_eq!(
        serde_json::to_value(input).unwrap()["label"],
        "private-label-sentinel"
    );
}
