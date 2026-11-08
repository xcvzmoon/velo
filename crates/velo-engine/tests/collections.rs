use std::collections::BTreeMap;

use velo_engine::{ApiCollection, CollectionDefaults, CollectionEntry, NameValue, RequestAuth};

fn request(id: &str) -> CollectionEntry {
    CollectionEntry::Request {
        id: id.into(),
        name: format!("Request {id}"),
        description: String::new(),
        notes: String::new(),
        definition: Box::default(),
        inherit_auth: true,
        extractions: Vec::new(),
    }
}

fn pair(name: &str, value: &str) -> NameValue {
    NameValue {
        name: name.into(),
        value: value.into(),
    }
}

#[test]
fn scopes_preserve_duplicates_order_and_explicit_auth_overrides() {
    let mut nested = request("nested");
    if let CollectionEntry::Request { definition, .. } = &mut nested {
        definition
            .variables
            .insert("scope".into(), "request".into());
        definition.config.headers = vec![
            pair("X-Mode", "request-first"),
            pair("x-mode", "request-second"),
        ];
        definition.environments.insert(
            "staging".into(),
            BTreeMap::from([("scope".into(), "request-environment".into())]),
        );
    }
    let collection = ApiCollection {
        defaults: CollectionDefaults {
            headers: vec![pair("x-mode", "root"), pair("accept", "application/json")],
            auth: Some(RequestAuth::Bearer {
                token: "{{secret:root-token}}".into(),
            }),
            variables: BTreeMap::from([
                ("scope".into(), "root".into()),
                ("parent".into(), "retained".into()),
            ]),
        },
        environments: BTreeMap::from([(
            "staging".into(),
            BTreeMap::from([("origin".into(), "https://staging.example.com".into())]),
        )]),
        entries: vec![
            request("first"),
            CollectionEntry::Folder {
                id: "folder".into(),
                name: "Folder".into(),
                description: String::new(),
                notes: String::new(),
                defaults: CollectionDefaults {
                    headers: vec![pair("X-Mode", "folder")],
                    variables: BTreeMap::from([("scope".into(), "folder".into())]),
                    ..Default::default()
                },
                entries: vec![nested, request("last")],
            },
        ],
        ..Default::default()
    };
    let requests = collection.requests(&[]).unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.id.as_str())
            .collect::<Vec<_>>(),
        ["first", "nested", "last"]
    );
    let nested = collection.request("nested").unwrap();
    assert_eq!(nested.definition.variables["scope"], "request");
    assert_eq!(nested.definition.variables["parent"], "retained");
    assert_eq!(
        nested.definition.environments["staging"]["scope"],
        "request-environment"
    );
    assert_eq!(
        nested.definition.environments["staging"]["origin"],
        "https://staging.example.com"
    );
    assert_eq!(
        nested
            .definition
            .config
            .headers
            .iter()
            .map(|pair| pair.value.as_str())
            .collect::<Vec<_>>(),
        ["application/json", "request-first", "request-second"]
    );
    assert!(matches!(
        nested.definition.config.auth,
        RequestAuth::Bearer { .. }
    ));
    let selected = collection
        .requests(&["folder".into(), "nested".into()])
        .unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|request| request.id.as_str())
            .collect::<Vec<_>>(),
        ["nested", "last"]
    );
    assert!(collection.requests(&["missing".into()]).is_err());
}

#[test]
fn explicit_none_and_header_authorization_override_inherited_auth() {
    let mut disabled = request("disabled");
    if let CollectionEntry::Request { inherit_auth, .. } = &mut disabled {
        *inherit_auth = false;
    }
    let mut header = request("header");
    if let CollectionEntry::Request { definition, .. } = &mut header {
        definition
            .config
            .headers
            .push(pair("Authorization", "{{secret:custom-authorization}}"));
    }
    let collection = ApiCollection {
        defaults: CollectionDefaults {
            auth: Some(RequestAuth::Bearer {
                token: "{{secret:parent-token}}".into(),
            }),
            ..Default::default()
        },
        entries: vec![disabled, header],
        ..Default::default()
    };
    let disabled = collection.request("disabled").unwrap();
    assert!(matches!(disabled.definition.config.auth, RequestAuth::None));
    let header = collection.request("header").unwrap();
    assert!(matches!(header.definition.config.auth, RequestAuth::None));
    assert_eq!(
        header.definition.config.headers[0].value,
        "{{secret:custom-authorization}}"
    );
}

#[test]
fn rejects_literal_credentials_duplicate_ids_and_unsupported_versions() {
    for field in ["auth", "header", "query", "json", "variable", "url"] {
        let mut entry = request("request");
        if let CollectionEntry::Request { definition, .. } = &mut entry {
            match field {
                "auth" => {
                    definition.config.auth = RequestAuth::Bearer {
                        token: "private-token".into(),
                    }
                }
                "header" => definition
                    .config
                    .headers
                    .push(pair("X-Api-Key", "private-token")),
                "query" => definition
                    .config
                    .query
                    .push(pair("api_key", "private-token")),
                "json" => {
                    definition.config.body = velo_engine::RequestBody::Json(
                        r#"{"nested":{"password":"private-token"}}"#.into(),
                    )
                }
                "variable" => {
                    definition
                        .variables
                        .insert("access_token".into(), "private-token".into());
                }
                "url" => {
                    definition.config.url = "https://example.com/?api_key=private-token".into()
                }
                _ => unreachable!(),
            }
        }
        let error = ApiCollection {
            entries: vec![entry],
            ..Default::default()
        }
        .validate()
        .unwrap_err();
        assert_eq!(error.code, "COLLECTION_SECRET_REQUIRED");
        assert!(!error.to_string().contains("private-token"));
    }
    assert!(
        ApiCollection {
            entries: vec![request("duplicate"), request("duplicate")],
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert_eq!(
        ApiCollection {
            version: 2,
            ..Default::default()
        }
        .validate()
        .unwrap_err()
        .code,
        "UNSUPPORTED_COLLECTION_VERSION"
    );
}

#[test]
fn portable_documents_strip_local_uploads_and_keep_refs_and_order() {
    let mut entry = request("upload");
    if let CollectionEntry::Request { definition, .. } = &mut entry {
        definition.config.auth = RequestAuth::Bearer {
            token: "{{secret:token}}".into(),
        };
        definition.config.body =
            velo_engine::RequestBody::Multipart(vec![velo_engine::MultipartPart::File {
                name: "upload".into(),
                path: "/private/local.txt".into(),
                filename: "document.txt".into(),
                content_type: "text/plain".into(),
            }]);
    }
    let collection = ApiCollection {
        entries: vec![entry, request("second")],
        ..Default::default()
    };
    let local = serde_json::to_string(&collection).unwrap();
    assert!(local.contains("/private/local.txt"));
    let portable = ApiCollection::parse(&local).unwrap();
    let exported = serde_json::to_string(&portable).unwrap();
    assert!(!exported.contains("/private/local.txt"));
    assert!(exported.contains("{{secret:token}}"));
    assert_eq!(
        portable
            .requests(&[])
            .unwrap()
            .iter()
            .map(|request| request.id.as_str())
            .collect::<Vec<_>>(),
        ["upload", "second"]
    );
}

#[test]
fn scenario_conversion_preserves_request_semantics_and_rejects_unrepresentable_scopes() {
    let mut first = request("first");
    if let CollectionEntry::Request {
        definition,
        extractions,
        ..
    } = &mut first
    {
        definition.config.url = "https://example.com/first".into();
        definition.config.query = vec![pair("id", "1"), pair("id", "2")];
        extractions.push(velo_engine::Extraction {
            name: "id".into(),
            source: velo_engine::ExtractionSource::Json {
                pointer: "/id".into(),
            },
            required: true,
            overwrite: true,
        });
    }
    let mut second = request("second");
    if let CollectionEntry::Request { definition, .. } = &mut second {
        definition.config.url = "https://example.com/{{session:id}}".into();
    }
    let mut collection = ApiCollection {
        entries: vec![first, second],
        ..Default::default()
    };
    let converted = collection.scenario(&[]).unwrap();
    let scenario = &converted.config.scenarios.unwrap().scenarios[0];
    assert_eq!(scenario.steps.len(), 2);
    assert_eq!(scenario.steps[0].request.query.len(), 2);
    assert_eq!(scenario.steps[0].extractions[0].name, "id");
    assert_eq!(
        scenario.steps[1].request.url,
        "https://example.com/{{session:id}}"
    );
    if let CollectionEntry::Request { definition, .. } = &mut collection.entries[1] {
        definition.config.timeout_ms += 1;
    }
    assert_eq!(
        collection.scenario(&[]).unwrap_err().code,
        "COLLECTION_SCENARIO_UNSUPPORTED"
    );
}

#[test]
fn collection_debug_omits_authored_content_and_bounds_sensitive_response_fields() {
    let collection = ApiCollection {
        name: "private-name".into(),
        defaults: CollectionDefaults {
            variables: BTreeMap::from([("user".into(), "private-value".into())]),
            ..Default::default()
        },
        entries: vec![request("private-identity")],
        sensitive_response_fields: vec!["/account/password".into(), "/tokens/0".into()],
        ..Default::default()
    };
    collection.validate().unwrap();
    for rendered in [
        format!("{collection:?}"),
        format!("{:?}", collection.defaults),
        format!("{:?}", collection.entries[0]),
        format!("{:?}", collection.request("private-identity").unwrap()),
    ] {
        assert!(!rendered.contains("private-"));
    }
    for pointer in ["invalid", "/invalid~escape", "/{{dynamic}}"] {
        let invalid = ApiCollection {
            sensitive_response_fields: vec![pointer.into()],
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn sensitive_variable_aliases_cannot_hide_literal_credentials_in_outer_or_environment_scopes() {
    let mut entry = request("request");
    if let CollectionEntry::Request { definition, .. } = &mut entry {
        definition.config.auth = RequestAuth::Bearer {
            token: "{{credential}}".into(),
        };
    }
    let mut collection = ApiCollection {
        defaults: CollectionDefaults {
            variables: BTreeMap::from([("credential".into(), "private-token".into())]),
            ..Default::default()
        },
        entries: vec![entry],
        ..Default::default()
    };
    assert_eq!(
        collection.validate().unwrap_err().code,
        "COLLECTION_SECRET_REQUIRED"
    );
    collection
        .defaults
        .variables
        .insert("credential".into(), "{{secret:api-token}}".into());
    collection.validate().unwrap();
    collection.environments.insert(
        "staging".into(),
        BTreeMap::from([("credential".into(), "private-environment-token".into())]),
    );
    assert_eq!(
        collection.validate().unwrap_err().code,
        "COLLECTION_SECRET_REQUIRED"
    );
}

#[test]
fn inherited_defaults_cannot_amplify_a_small_document_into_unbounded_preparation() {
    let collection = ApiCollection {
        defaults: CollectionDefaults {
            variables: BTreeMap::from([("payload".into(), "a".repeat(1_048_576))]),
            ..Default::default()
        },
        entries: (0..17)
            .map(|index| request(&format!("request-{index}")))
            .collect(),
        ..Default::default()
    };
    assert!(serde_json::to_string(&collection).unwrap().len() < 8_388_608);
    assert_eq!(
        collection.validate().unwrap_err().code,
        "COLLECTION_BUDGET_EXCEEDED"
    );
}

#[test]
fn document_validation_rejects_excessive_depth_entry_count_and_invalid_pointer_escapes() {
    let mut nested = request("leaf");
    for index in 0..9 {
        nested = CollectionEntry::Folder {
            id: format!("folder-{index}"),
            name: "Folder".into(),
            description: String::new(),
            notes: String::new(),
            defaults: CollectionDefaults::default(),
            entries: vec![nested],
        };
    }
    assert!(
        ApiCollection {
            entries: vec![nested],
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    let oversized = ApiCollection {
        entries: (0..1001)
            .map(|index| request(&format!("request-{index}")))
            .collect(),
        ..Default::default()
    };
    assert!(oversized.validate().is_err());
    for pointer in ["/~", "/~2", "/path/~01/~3"] {
        assert!(
            ApiCollection {
                sensitive_response_fields: vec![pointer.into()],
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    ApiCollection {
        sensitive_response_fields: vec!["/~01/~0/~1".into(), String::new()],
        ..Default::default()
    }
    .validate()
    .unwrap();
}
