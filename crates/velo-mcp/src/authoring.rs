//! Offline canonical authoring and bounded draft files awaiting desktop review.

use std::{io::Write, path::PathBuf};

use serde_json::{Value, json};
use velo_engine::{
    ImportFormat, MultipartPart, NameValue, RequestAuth, RequestBody, RunConfig, RunError,
    RunManager, TestDefinition,
};

const DEFINITION_BYTES: usize = 256 * 1024;
const IMPORT_BYTES: usize = 512 * 1024;
const MAX_CANDIDATES: usize = 32;
const MAX_DRAFTS: usize = 8;
const MAX_DRAFT_BYTES: usize = 2 * 1024 * 1024;

pub(crate) fn template() -> Result<Value, &'static str> {
    serde_json::to_value(TestDefinition::default()).map_err(|_| "MCP_ENCODING_FAILED")
}

fn parse(source: &str) -> Result<TestDefinition, &'static str> {
    if source.len() > DEFINITION_BYTES {
        return Err("MCP_DEFINITION_TOO_LARGE");
    }
    TestDefinition::parse(source).map_err(|_| "MCP_INVALID_DEFINITION")
}

fn status(schema_valid: bool, preparation: &str, code: &str) -> Value {
    json!({
        "schema_valid": schema_valid,
        "preparation": preparation,
        "code": code,
        "execution_authorized": false,
        "endpoints_verified": false,
    })
}

pub(crate) fn validate(source: &str, environment: Option<&str>) -> Result<Value, &'static str> {
    if source.len() > DEFINITION_BYTES {
        return Err("MCP_DEFINITION_TOO_LARGE");
    }
    let definition = match TestDefinition::parse(source) {
        Ok(definition) => definition,
        Err(error) => return Ok(status(false, "invalid", &error.code)),
    };
    Ok(validate_definition(&definition, environment))
}

fn validate_definition(definition: &TestDefinition, environment: Option<&str>) -> Value {
    if requests(definition).any(|request| has_upload(request.body)) {
        return status(true, "needs_input", "UPLOAD_REBIND_REQUIRED");
    }
    if definition.dataset.is_some() {
        return status(true, "needs_input", "DATASET_INPUT_REQUIRED");
    }
    let config = match definition.resolve(environment, |_| {
        Err(RunError {
            code: "SECRET_INPUT_REQUIRED".into(),
            message: "Bind secrets in Velo before preparing this definition.".into(),
            field: None,
        })
    }) {
        Ok(config) => config,
        Err(error) => {
            if error.code == "SECRET_UNAVAILABLE" {
                return status(true, "needs_input", "SECRET_INPUT_REQUIRED");
            }
            let preparation = if matches!(
                error.code.as_str(),
                "SECRET_INPUT_REQUIRED" | "VARIABLE_UNRESOLVED" | "UPLOAD_REBIND_REQUIRED"
            ) {
                "needs_input"
            } else {
                "invalid"
            };
            return status(true, preparation, &error.code);
        }
    };
    match RunManager::prepare(config) {
        Ok(_) => status(true, "ready", "MCP_VALID_DEFINITION"),
        Err(error) => status(true, "invalid", &error.code),
    }
}

pub(crate) fn import(format: &str, source: &str) -> Result<Value, &'static str> {
    if source.len() > IMPORT_BYTES {
        return Err("MCP_IMPORT_TOO_LARGE");
    }
    let format = match format {
        "curl" => ImportFormat::Curl,
        "openapi" => ImportFormat::Openapi,
        "postman" => ImportFormat::Postman,
        _ => return Err("MCP_UNSUPPORTED_IMPORT"),
    };
    let candidates =
        velo_engine::import_requests(format, source).map_err(|_| "MCP_IMPORT_FAILED")?;
    if candidates.len() > MAX_CANDIDATES {
        return Err("MCP_TOO_MANY_CANDIDATES");
    }
    let mut definitions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let mut definition = TestDefinition {
            config: candidate.config,
            ..Default::default()
        };
        clear_definition_uploads(&mut definition);
        definitions.push(json!({
            "definition": definition,
            "warnings": candidate.warnings,
            "review_required": true,
            "endpoints_verified": false,
        }));
    }
    let output = json!({"candidates": definitions});
    if output.to_string().len() > IMPORT_BYTES {
        return Err("MCP_IMPORT_TOO_LARGE");
    }
    Ok(output)
}

struct Request<'a> {
    url: &'a str,
    auth: &'a RequestAuth,
    headers: &'a [NameValue],
    query: &'a [NameValue],
    body: &'a RequestBody,
}

fn requests(definition: &TestDefinition) -> impl Iterator<Item = Request<'_>> {
    let config = &definition.config;
    std::iter::once(Request {
        url: &config.url,
        auth: &config.auth,
        headers: &config.headers,
        query: &config.query,
        body: &config.body,
    })
    .chain(
        config
            .scenarios
            .iter()
            .flat_map(|plan| {
                plan.setup
                    .iter()
                    .chain(&plan.cleanup)
                    .chain(plan.scenarios.iter().flat_map(|scenario| &scenario.steps))
            })
            .map(|step| Request {
                url: &step.request.url,
                auth: &step.request.auth,
                headers: &step.request.headers,
                query: &step.request.query,
                body: &step.request.body,
            }),
    )
}

fn has_upload(body: &RequestBody) -> bool {
    matches!(body, RequestBody::Multipart(parts) if parts.iter().any(|part| matches!(part, MultipartPart::File { .. })))
}

fn clear_uploads(body: &mut RequestBody) {
    if let RequestBody::Multipart(parts) = body {
        for part in parts {
            if let MultipartPart::File { path, .. } = part {
                path.clear();
            }
        }
    }
}

fn clear_definition_uploads(definition: &mut TestDefinition) {
    clear_uploads(&mut definition.config.body);
    if let Some(plan) = &mut definition.config.scenarios {
        for step in plan.setup.iter_mut().chain(&mut plan.cleanup).chain(
            plan.scenarios
                .iter_mut()
                .flat_map(|scenario| &mut scenario.steps),
        ) {
            clear_uploads(&mut step.request.body);
        }
    }
}

fn credential_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "authorization",
        "cookie",
        "token",
        "secret",
        "password",
        "api-key",
        "api_key",
        "apikey",
        "credential",
    ]
    .iter()
    .any(|term| name.contains(term))
}

fn secret_reference(value: &str) -> bool {
    let Some(name) = value
        .strip_prefix("{{secret:")
        .and_then(|value| value.strip_suffix("}}"))
    else {
        return false;
    };
    !name.is_empty()
        && name.len() <= 80
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn protected_value(value: &str) -> bool {
    secret_reference(value)
        || value.strip_prefix("Bearer ").is_some_and(secret_reference)
        || value.strip_prefix("Basic ").is_some_and(secret_reference)
}

fn json_credentials(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(name, value)| {
            credential_name(name) && !value.as_str().is_some_and(secret_reference)
                || json_credentials(value)
        }),
        Value::Array(items) => items.iter().any(json_credentials),
        _ => false,
    }
}

fn body_credentials(body: &RequestBody) -> bool {
    match body {
        RequestBody::Json(source) => serde_json::from_str::<Value>(source)
            .is_ok_and(|value| json_credentials(&value)),
        RequestBody::Form(pairs) => pairs
            .iter()
            .any(|pair| credential_name(&pair.name) && !secret_reference(&pair.value)),
        RequestBody::Multipart(parts) => parts.iter().any(|part| {
            matches!(part, MultipartPart::Text { name, value } if credential_name(name) && !secret_reference(value))
        }),
        RequestBody::None | RequestBody::Text(_) => false,
    }
}

fn literal_credentials(definition: &TestDefinition) -> bool {
    let request_credentials = requests(definition).any(|request| {
        let auth_literal = match request.auth {
            RequestAuth::None => false,
            RequestAuth::Bearer { token } => !secret_reference(token),
            RequestAuth::ApiKey { value, .. } => !secret_reference(value),
            RequestAuth::Basic { username, password } => {
                (!username.is_empty() && !secret_reference(username)) || !secret_reference(password)
            }
        };
        let pair_literal = request
            .headers
            .iter()
            .chain(request.query)
            .any(|pair| credential_name(&pair.name) && !protected_value(&pair.value));
        let authority_literal = request.url.split_once("://").is_some_and(|(_, rest)| {
            rest.split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
        });
        let query_literal = request.url.split_once('?').is_some_and(|(_, query)| {
            url::form_urlencoded::parse(query.split('#').next().unwrap_or_default().as_bytes())
                .any(|(name, value)| credential_name(&name) && !protected_value(&value))
        });
        auth_literal
            || pair_literal
            || authority_literal
            || query_literal
            || body_credentials(request.body)
    });
    request_credentials
        || definition
            .variables
            .iter()
            .chain(
                definition
                    .environments
                    .values()
                    .flat_map(|values| values.iter()),
            )
            .any(|(name, value)| credential_name(name) && !secret_reference(value))
}

pub(crate) struct Drafts {
    directory: Option<PathBuf>,
    published: usize,
    published_bytes: usize,
}

impl std::fmt::Debug for Drafts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Drafts([redacted])")
    }
}

impl Drafts {
    pub(crate) fn new(directory: Option<PathBuf>) -> Result<Self, &'static str> {
        let directory = directory
            .map(|directory| {
                let canonical = directory
                    .canonicalize()
                    .map_err(|_| "MCP_DRAFT_DIRECTORY_UNAVAILABLE")?;
                if !canonical.is_dir() || canonical.to_str().is_none() {
                    return Err("MCP_DRAFT_DIRECTORY_UNAVAILABLE");
                }
                Ok(canonical)
            })
            .transpose()?;
        Ok(Self {
            directory,
            published: 0,
            published_bytes: 0,
        })
    }

    pub(crate) fn enabled(&self) -> bool {
        self.directory.is_some()
    }

    pub(crate) fn propose(
        &mut self,
        source: &str,
        cancelled: &tokio_util::sync::CancellationToken,
    ) -> Result<Value, &'static str> {
        if cancelled.is_cancelled() {
            return Err("MCP_CANCELLED");
        }
        let directory = self.directory.as_ref().ok_or("MCP_DRAFTS_DISABLED")?;
        if self.published >= MAX_DRAFTS {
            return Err("MCP_DRAFT_QUOTA_EXCEEDED");
        }
        let mut definition = parse(source)?;
        if literal_credentials(&definition) {
            return Err("MCP_LITERAL_CREDENTIALS_REJECTED");
        }
        let defaults = RunConfig::default().budget;
        definition.config.budget.max_vus = definition.config.budget.max_vus.min(defaults.max_vus);
        definition.config.budget.max_request_bytes = definition
            .config
            .budget
            .max_request_bytes
            .min(defaults.max_request_bytes);
        clear_definition_uploads(&mut definition);
        let validation = validate_definition(&definition, None);
        if validation["preparation"] == "invalid" {
            return Err("MCP_INVALID_DEFINITION");
        }
        let content = serde_json::to_vec_pretty(&definition).map_err(|_| "MCP_ENCODING_FAILED")?;
        if content.len() > DEFINITION_BYTES
            || self.published_bytes.saturating_add(content.len()) > MAX_DRAFT_BYTES
        {
            return Err("MCP_DRAFT_QUOTA_EXCEEDED");
        }
        let id = uuid::Uuid::new_v4().to_string();
        let filename = format!("proposal-{id}.velo.test.json");
        let path = directory.join(&filename);
        if cancelled.is_cancelled() {
            return Err("MCP_CANCELLED");
        }
        let mut file =
            tempfile::NamedTempFile::new_in(directory).map_err(|_| "MCP_DRAFT_WRITE_FAILED")?;
        file.write_all(&content)
            .and_then(|()| file.as_file().sync_all())
            .map_err(|_| "MCP_DRAFT_WRITE_FAILED")?;
        if cancelled.is_cancelled() {
            return Err("MCP_CANCELLED");
        }
        file.persist_noclobber(&path)
            .map_err(|_| "MCP_DRAFT_WRITE_FAILED")?;
        self.published += 1;
        self.published_bytes += content.len();
        Ok(json!({
            "id": id,
            "filename": filename,
            "path": path,
            "validation": validation,
            "review_required": true,
            "resource_budgets": "Capped at Velo defaults; lower requested limits are preserved. Edit budgets in Velo after review.",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> TestDefinition {
        TestDefinition {
            config: RunConfig {
                url: "http://127.0.0.1:1/health".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn encoded(definition: &TestDefinition) -> String {
        serde_json::to_string(definition).unwrap()
    }

    #[test]
    fn api_key_proposals_require_secret_references() {
        let mut definition = TestDefinition::default();
        definition.config.auth = RequestAuth::ApiKey {
            name: "x-api-key".into(),
            value: "private-key".into(),
            placement: velo_engine::ApiKeyPlacement::Header,
        };
        assert!(literal_credentials(&definition));
        definition.config.auth = RequestAuth::ApiKey {
            name: "x-api-key".into(),
            value: "{{secret:api-key}}".into(),
            placement: velo_engine::ApiKeyPlacement::Header,
        };
        assert!(!literal_credentials(&definition));
    }

    #[test]
    fn validation_uses_engine_rules_without_contacting_the_target() {
        let mut definition = definition();
        assert_eq!(
            validate(&encoded(&definition), None).unwrap()["preparation"],
            "ready"
        );
        definition.config.timeout_ms = 0;
        let result = validate(&encoded(&definition), None).unwrap();
        assert_eq!(result["schema_valid"], true);
        assert_eq!(result["preparation"], "invalid");
        assert_eq!(result["execution_authorized"], false);
        assert!(!result.to_string().contains("127.0.0.1"));
        assert_eq!(
            validate("PRIVATE_INVALID_INPUT", None).unwrap()["schema_valid"],
            false
        );
    }

    #[test]
    fn secrets_and_uploads_are_missing_inputs_and_never_accessed() {
        let mut definition = definition();
        definition.config.auth = RequestAuth::Bearer {
            token: "{{secret:private}}".into(),
        };
        assert_eq!(
            validate(&encoded(&definition), None).unwrap()["code"],
            "SECRET_INPUT_REQUIRED"
        );
        definition.config.body = RequestBody::Multipart(vec![MultipartPart::File {
            name: "file".into(),
            path: "/THIS_FILE_MUST_NEVER_BE_OPENED".into(),
            filename: "file.bin".into(),
            content_type: "application/octet-stream".into(),
        }]);
        definition.config.method = velo_engine::RequestMethod::POST;
        assert_eq!(
            validate(&encoded(&definition), None).unwrap()["code"],
            "UPLOAD_REBIND_REQUIRED"
        );
    }

    #[test]
    fn import_is_offline_and_returns_canonical_reviewable_definitions() {
        let result = import("curl", "curl https://example.com/health").unwrap();
        let candidate = &result["candidates"][0];
        assert_eq!(candidate["review_required"], true);
        assert_eq!(candidate["endpoints_verified"], false);
        TestDefinition::parse(&candidate["definition"].to_string()).unwrap();
        assert_eq!(
            import("script", "anything").unwrap_err(),
            "MCP_UNSUPPORTED_IMPORT"
        );
        assert_eq!(
            import("curl", "curl $(touch /tmp/private)").unwrap_err(),
            "MCP_IMPORT_FAILED"
        );
    }

    #[test]
    fn proposal_publication_is_opt_in_bounded_and_preserves_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("existing.velo.test.json");
        std::fs::write(&existing, "DO_NOT_REPLACE").unwrap();
        let source = encoded(&definition());
        let cancelled = tokio_util::sync::CancellationToken::new();
        assert_eq!(
            Drafts::new(None)
                .unwrap()
                .propose(&source, &cancelled)
                .unwrap_err(),
            "MCP_DRAFTS_DISABLED"
        );
        let mut drafts = Drafts::new(Some(directory.path().to_owned())).unwrap();
        let mut paths = std::collections::BTreeSet::new();
        for _ in 0..MAX_DRAFTS {
            let result = drafts.propose(&source, &cancelled).unwrap();
            let path = PathBuf::from(result["path"].as_str().unwrap());
            assert!(paths.insert(path.clone()));
            assert_eq!(result["review_required"], true);
            TestDefinition::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        }
        assert_eq!(
            drafts.propose(&source, &cancelled).unwrap_err(),
            "MCP_DRAFT_QUOTA_EXCEEDED"
        );
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "DO_NOT_REPLACE");
        assert_eq!(
            std::fs::read_dir(directory.path()).unwrap().count(),
            MAX_DRAFTS + 1
        );
    }

    #[test]
    fn proposals_cannot_raise_budgets_or_persist_literal_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let cancelled = tokio_util::sync::CancellationToken::new();
        let mut drafts = Drafts::new(Some(directory.path().to_owned())).unwrap();
        let mut definition = definition();
        definition.config.budget.max_vus = 100_000;
        definition.config.budget.max_request_bytes = u32::MAX;
        let result = drafts.propose(&encoded(&definition), &cancelled).unwrap();
        let source = std::fs::read_to_string(result["path"].as_str().unwrap()).unwrap();
        let published = TestDefinition::parse(&source).unwrap();
        assert_eq!(
            published.config.budget.max_vus,
            RunConfig::default().budget.max_vus
        );
        assert_eq!(
            published.config.budget.max_request_bytes,
            RunConfig::default().budget.max_request_bytes
        );
        definition.config.headers.push(NameValue {
            name: "X-API-Key".into(),
            value: "PRIVATE_KEY".into(),
        });
        assert_eq!(
            drafts
                .propose(&encoded(&definition), &cancelled)
                .unwrap_err(),
            "MCP_LITERAL_CREDENTIALS_REJECTED"
        );
        definition.config.headers.clear();
        definition.config.url = "https://example.com?%74oken=PRIVATE_KEY".into();
        assert_eq!(
            drafts
                .propose(&encoded(&definition), &cancelled)
                .unwrap_err(),
            "MCP_LITERAL_CREDENTIALS_REJECTED"
        );
        definition.config.url = "https://{{host}}?%74oken=PRIVATE_KEY".into();
        assert_eq!(
            drafts
                .propose(&encoded(&definition), &cancelled)
                .unwrap_err(),
            "MCP_LITERAL_CREDENTIALS_REJECTED"
        );
    }

    #[test]
    fn lower_budgets_are_preserved_and_invalid_engine_rules_do_not_publish() {
        let directory = tempfile::tempdir().unwrap();
        let cancelled = tokio_util::sync::CancellationToken::new();
        let mut drafts = Drafts::new(Some(directory.path().to_owned())).unwrap();
        let mut definition = definition();
        definition.config.budget.max_vus = 1;
        definition.config.budget.max_request_bytes = 65_536;
        let result = drafts.propose(&encoded(&definition), &cancelled).unwrap();
        let source = std::fs::read_to_string(result["path"].as_str().unwrap()).unwrap();
        let published = TestDefinition::parse(&source).unwrap();
        assert_eq!(published.config.budget.max_vus, 1);
        assert_eq!(published.config.budget.max_request_bytes, 65_536);
        let mut invalid = serde_json::to_value(&definition).unwrap();
        invalid["config"]["workload"] = json!({"kind":"constant", "vus": 2, "duration_ms":1000});
        assert_eq!(
            validate(&invalid.to_string(), None).unwrap()["preparation"],
            "invalid"
        );
        assert_eq!(
            drafts
                .propose(&invalid.to_string(), &cancelled)
                .unwrap_err(),
            "MCP_INVALID_DEFINITION"
        );
        invalid["config"]["workload"] = json!({"kind":"constant", "vus":1, "duration_ms":1000});
        invalid["config"]["assertions"] =
            json!([{"id":"invalid id", "rule":{"kind":"status", "allowed":[200]}}]);
        assert_eq!(
            validate(&invalid.to_string(), None).unwrap()["preparation"],
            "invalid"
        );
        assert_eq!(
            drafts
                .propose(&invalid.to_string(), &cancelled)
                .unwrap_err(),
            "MCP_INVALID_DEFINITION"
        );
        assert_eq!(drafts.published, 1);
    }

    #[test]
    fn literal_body_credentials_are_rejected_but_secret_references_remain_unresolved() {
        let directory = tempfile::tempdir().unwrap();
        let cancelled = tokio_util::sync::CancellationToken::new();
        let mut drafts = Drafts::new(Some(directory.path().to_owned())).unwrap();
        let mut definition = definition();
        definition.config.method = velo_engine::RequestMethod::POST;
        definition.config.body =
            RequestBody::Json(r#"{"account":{"password":"PRIVATE_PASSWORD"}}"#.into());
        assert_eq!(
            drafts
                .propose(&encoded(&definition), &cancelled)
                .unwrap_err(),
            "MCP_LITERAL_CREDENTIALS_REJECTED"
        );
        definition.config.body =
            RequestBody::Json(r#"{"account":{"password":"{{secret:password}}"}}"#.into());
        let result = drafts.propose(&encoded(&definition), &cancelled).unwrap();
        assert_eq!(result["validation"]["code"], "SECRET_INPUT_REQUIRED");
        assert_eq!(result["validation"]["preparation"], "needs_input");
    }

    #[test]
    fn cancelled_proposals_publish_nothing_and_do_not_consume_quota() {
        let directory = tempfile::tempdir().unwrap();
        let mut drafts = Drafts::new(Some(directory.path().to_owned())).unwrap();
        let cancelled = tokio_util::sync::CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            drafts
                .propose(&encoded(&definition()), &cancelled)
                .unwrap_err(),
            "MCP_CANCELLED"
        );
        assert_eq!(drafts.published, 0);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn uploads_are_cleared_in_published_drafts_and_file_permissions_are_private() {
        let directory = tempfile::tempdir().unwrap();
        let mut drafts = Drafts::new(Some(directory.path().to_owned())).unwrap();
        let cancelled = tokio_util::sync::CancellationToken::new();
        let mut definition = definition();
        definition.config.body = RequestBody::Multipart(vec![MultipartPart::File {
            name: "file".into(),
            path: "/PRIVATE_LOCAL_PATH".into(),
            filename: "file.bin".into(),
            content_type: "application/octet-stream".into(),
        }]);
        let result = drafts.propose(&encoded(&definition), &cancelled).unwrap();
        assert_eq!(result["validation"]["code"], "UPLOAD_REBIND_REQUIRED");
        let path = PathBuf::from(result["path"].as_str().unwrap());
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("PRIVATE_LOCAL_PATH")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
