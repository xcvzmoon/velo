use velo_engine::{MultipartPart, NameValue, RequestAuth, RequestBody, RunConfig};

#[test]
fn request_debug_formatting_omits_sensitive_data_without_changing_serialization() {
    let pair = NameValue {
        name: "private-name-sentinel".into(),
        value: "private-value-sentinel".into(),
    };
    let parts = vec![
        MultipartPart::Text {
            name: pair.name.clone(),
            value: pair.value.clone(),
        },
        MultipartPart::File {
            name: pair.name.clone(),
            path: "/private/path-sentinel".into(),
            filename: "private-filename-sentinel".into(),
            content_type: "private-content-type-sentinel".into(),
        },
    ];
    let bodies = vec![
        RequestBody::None,
        RequestBody::Text("private-text-sentinel".into()),
        RequestBody::Json("private-json-sentinel".into()),
        RequestBody::Form(vec![pair.clone()]),
        RequestBody::Multipart(parts.clone()),
    ];
    let authentications = vec![
        RequestAuth::None,
        RequestAuth::Bearer {
            token: "private-token-sentinel".into(),
        },
        RequestAuth::Basic {
            username: "private-username-sentinel".into(),
            password: "private-password-sentinel".into(),
        },
    ];
    let mut rendered = vec![format!("{pair:?}"), format!("{pair:#?}")];
    for part in &parts {
        rendered.push(format!("{part:?}"));
        rendered.push(format!("{part:#?}"));
    }
    for body in bodies {
        rendered.push(format!("{body:?}"));
        rendered.push(format!("{body:#?}"));
        for auth in &authentications {
            rendered.push(format!("{auth:?}"));
            rendered.push(format!("{auth:#?}"));
            let config = RunConfig {
                url: "https://example.com/private-url-sentinel?key=private-query-sentinel".into(),
                headers: vec![pair.clone()],
                query: vec![pair.clone()],
                body: body.clone(),
                auth: auth.clone(),
                ..RunConfig::default()
            };
            let serialized = serde_json::to_value(&config).unwrap();
            rendered.push(format!("{config:?}"));
            rendered.push(format!("{config:#?}"));
            assert!(
                serialized["url"]
                    .as_str()
                    .unwrap()
                    .contains("private-url-sentinel")
            );
            assert_eq!(serialized["headers"][0]["value"], pair.value);
        }
    }
    for output in rendered {
        assert!(
            !output.contains("private-"),
            "sensitive debug output: {output}"
        );
    }
}
