//! Portable test definitions and deterministic request-variable resolution.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{MultipartPart, RequestAuth, RequestBody, RunConfig, RunError};

/// Portable authoring document. Version 1 retains all engine settings and ordered pairs.
/// Secret values belong in the OS store, addressed with `{{secret:name}}`.
///
/// # Examples
/// ```
/// let definition = velo_engine::TestDefinition::default();
/// assert_eq!(definition.version, 1);
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TestDefinition {
    /// Format version; only 1 is supported.
    pub version: u32,
    /// Optional dataset recipe; prepared rows are transient and must be rebound after loading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dataset: Option<crate::DataDefinition>,
    /// Editable request and workload, with optional `{{name}}` placeholders.
    pub config: RunConfig,
    /// Shared literal values or OS secret references, overridden by the selected environment.
    #[ts(type = "Record<string, string>")]
    pub variables: BTreeMap<String, String>,
    /// Named values or `{{secret:name}}` references. Resolved secrets are never stored here.
    #[ts(type = "Record<string, Record<string, string>>")]
    pub environments: BTreeMap<String, BTreeMap<String, String>>,
}

impl Default for TestDefinition {
    fn default() -> Self {
        Self {
            version: 1,
            dataset: None,
            config: RunConfig::default(),
            variables: BTreeMap::new(),
            environments: BTreeMap::new(),
        }
    }
}

impl std::fmt::Debug for TestDefinition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TestDefinition([redacted])")
    }
}

fn invalid(field: &str, message: &str) -> RunError {
    RunError::new("INVALID_TEST_DEFINITION", message, Some(field))
}

struct Variable {
    text: String,
    literal: bool,
}

#[derive(Clone, Copy)]
enum BindingPhase {
    Standalone,
    Scenario,
    Session,
}

struct Bindings {
    values: BTreeMap<String, Variable>,
    phase: BindingPhase,
}

fn substitute(
    value: &str,
    field: &str,
    variables: &Bindings,
    secrets: &mut impl FnMut(&str) -> Result<String, RunError>,
) -> Result<String, RunError> {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        let end = rest
            .find("}}")
            .ok_or_else(|| invalid(field, "Close the variable reference with }}."))?;
        let name = &rest[..end];
        if matches!(variables.phase, BindingPhase::Scenario) && name.starts_with("session:") {
            output.push_str("{{");
            output.push_str(name);
            output.push_str("}}");
            rest = &rest[end + 2..];
            continue;
        }
        if matches!(variables.phase, BindingPhase::Session) && !name.starts_with("session:") {
            return Err(RunError::new(
                "VARIABLE_UNRESOLVED",
                "Only session references may remain during execution.",
                Some(field),
            ));
        }
        let variable;
        let secret_name = if let Some(secret) = name.strip_prefix("secret:") {
            Some(secret)
        } else {
            variable = variables.values.get(name).ok_or_else(|| RunError::new("VARIABLE_UNRESOLVED", "Define the referenced variable in the selected environment or shared variables.", Some(field)))?;
            if variable.literal {
                None
            } else {
                variable
                    .text
                    .strip_prefix("{{secret:")
                    .and_then(|value| value.strip_suffix("}}"))
            }
        };
        let replacement = if let Some(secret) = secret_name {
            secrets(secret).map_err(|_| {
                RunError::new(
                    "SECRET_UNAVAILABLE",
                    "Set the referenced value in the OS credential store.",
                    Some(field),
                )
            })?
        } else {
            let value = variables
                .values
                .get(name)
                .ok_or_else(|| invalid(field, "Define the referenced variable."))?;
            if !value.literal && value.text.contains("{{") {
                return Err(invalid(
                    field,
                    "Variable values may reference one OS secret, but cannot contain nested variables.",
                ));
            }
            value.text.clone()
        };
        if matches!(variables.phase, BindingPhase::Scenario) && replacement.contains("{{") {
            return Err(invalid(
                field,
                "Scenario variable values cannot introduce new references.",
            ));
        }
        if output.len().saturating_add(replacement.len()) > 8_388_608 {
            return Err(invalid(field, "Resolved field exceeds 8 MiB."));
        }
        output.push_str(&replacement);
        rest = &rest[end + 2..];
    }
    if output.len().saturating_add(rest.len()) > 8_388_608 {
        return Err(invalid(field, "Resolved field exceeds 8 MiB."));
    }
    output.push_str(rest);
    Ok(output)
}

fn substitute_url(
    value: &str,
    variables: &Bindings,
    secrets: &mut impl FnMut(&str) -> Result<String, RunError>,
) -> Result<String, RunError> {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest
            .find("}}")
            .ok_or_else(|| invalid("url", "Close the variable reference with }}."))?
            + 2;
        let replacement = substitute(&rest[..end], "url", variables, secrets)?;
        let in_path_or_query = output
            .split_once("://")
            .is_some_and(|(_, authority)| authority.contains('/') || authority.contains('?'));
        if rest[..end].starts_with("{{session:") && !in_path_or_query {
            return Err(invalid(
                "url",
                "Session references cannot change a target origin.",
            ));
        }
        if matches!(variables.phase, BindingPhase::Scenario)
            && rest[..end].starts_with("{{session:")
        {
            output.push_str(&replacement);
        } else if in_path_or_query {
            output.push_str(
                &percent_encoding::utf8_percent_encode(
                    &replacement,
                    percent_encoding::NON_ALPHANUMERIC,
                )
                .to_string(),
            );
        } else {
            output.push_str(&replacement);
        }
        if output.len() > 8_388_608 {
            return Err(invalid("url", "Resolved URL exceeds 8 MiB."));
        }
        rest = &rest[end..];
    }
    output.push_str(rest);
    Ok(output)
}

fn json_strings(
    value: &mut serde_json::Value,
    variables: &Bindings,
    secrets: &mut impl FnMut(&str) -> Result<String, RunError>,
) -> Result<(), RunError> {
    match value {
        serde_json::Value::String(text) => {
            *text = substitute(text, "body.value", variables, secrets)?
        }
        serde_json::Value::Array(values) => {
            for value in values {
                json_strings(value, variables, secrets)?;
            }
        }
        serde_json::Value::Object(values) => {
            let mut resolved = serde_json::Map::new();
            for (key, mut value) in std::mem::take(values) {
                json_strings(&mut value, variables, secrets)?;
                let key = substitute(&key, "body.value", variables, secrets)?;
                if resolved.insert(key, value).is_some() {
                    return Err(invalid("body.value", "Resolved JSON keys must be unique."));
                }
            }
            *values = resolved;
        }
        _ => {}
    }
    Ok(())
}

impl TestDefinition {
    /// Parses a bounded document without resolving values or sending requests.
    ///
    /// # Arguments
    /// * `source` - UTF-8 JSON, at most 8 MiB.
    ///
    /// # Returns
    /// A definition with a supported version and strict engine field shapes.
    ///
    /// # Errors
    /// Returns `INVALID_TEST_DEFINITION` for malformed/oversized input, or
    /// `UNSUPPORTED_TEST_VERSION` for other format versions. Input content is omitted.
    ///
    /// # Examples
    /// ```
    /// let json = serde_json::to_string(&velo_engine::TestDefinition::default())?;
    /// let parsed = velo_engine::TestDefinition::parse(&json)?;
    /// assert_eq!(parsed.version, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(source: &str) -> Result<Self, RunError> {
        if source.len() > 8_388_608 {
            return Err(invalid("definition", "Definition exceeds 8 MiB."));
        }
        let value: serde_json::Value =
            serde_json::from_str(source).map_err(|_| invalid("definition", "Enter valid JSON."))?;
        if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
            return Err(RunError::new(
                "UNSUPPORTED_TEST_VERSION",
                "Use velo.test.json version 1.",
                Some("version"),
            ));
        }
        let mut deserializer = serde_json::Deserializer::from_str(source);
        let definition: Self = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let field = error.path().to_string();
            RunError::new(
                "INVALID_TEST_DEFINITION",
                "Check the field type, required properties, and supported values for version 1.",
                Some(&field),
            )
        })?;
        if let Some(data) = &definition.dataset {
            data.validate()?;
        }
        if definition.config.data.is_some() {
            return Err(invalid(
                "config.data",
                "Prepared requests cannot be saved in a test definition.",
            ));
        }
        Ok(definition)
    }

    /// Resolves bounded records through the existing variable binding rules before any traffic.
    ///
    /// # Arguments
    /// * `environment` - Environment override, or None for shared values.
    /// * `rows` - Prepared scalar string records; empty only when no dataset is configured.
    /// * `cancelled` - Cooperative cancellation check, sampled before each record.
    /// * `secrets` - OS secret lookup. Callers should cache values across records.
    ///
    /// # Returns
    /// A transient run configuration. Dataset values override environment/shared variables.
    /// All records must resolve successfully before the configuration can be executed.
    /// Scenario requests retain authored `{{session:name}}` references for execution.
    /// Shared, environment and dataset keys cannot contain colons. Scenario replacement
    /// values cannot introduce references, preventing later interpolation of data as templates.
    ///
    /// # Errors
    /// Returns `INVALID_DATASET`, `DATASET_BUDGET_EXCEEDED`, `PREPARATION_CANCELLED`, or variable-resolution errors.
    /// No requests are sent and owned temporary allocations are dropped on failure.
    ///
    /// # Examples
    /// ```
    /// let definition = velo_engine::TestDefinition::default();
    /// let config = definition.resolve_data(None, &[], || false, |_| unreachable!())?;
    /// assert!(config.data.is_none());
    /// # Ok::<(), velo_engine::RunError>(())
    /// ```
    pub fn resolve_data(
        &self,
        environment: Option<&str>,
        rows: &[BTreeMap<String, String>],
        mut cancelled: impl FnMut() -> bool,
        mut secrets: impl FnMut(&str) -> Result<String, RunError>,
    ) -> Result<RunConfig, RunError> {
        if self.config.data.is_some() {
            return Err(invalid(
                "config.data",
                "Prepared requests do not belong in test definitions.",
            ));
        }
        let Some(data) = &self.dataset else {
            if !rows.is_empty() {
                return Err(crate::dataset::invalid(
                    "Configure a dataset before supplying rows.",
                ));
            }
            return self.resolve(environment, secrets);
        };
        data.validate()?;
        if rows.is_empty() || rows.len() > 10_000 {
            return Err(crate::dataset::invalid(
                "Prepare 1 to 10,000 records before review.",
            ));
        }
        if let crate::DataSource::Faker { count, .. } = &data.source
            && rows.len() != *count as usize
        {
            return Err(crate::dataset::invalid(
                "Prepared count does not match the Faker recipe.",
            ));
        }
        let names: Vec<&str> = match &data.source {
            crate::DataSource::Faker { fields, .. } => {
                fields.iter().map(|field| field.name.as_str()).collect()
            }
            crate::DataSource::Csv { mapping } | crate::DataSource::Json { mapping } => {
                mapping.keys().map(String::as_str).collect()
            }
        };
        let mut bytes = 0usize;
        let mut requests = Vec::with_capacity(rows.len());
        let mut first = None;
        let mut row_bytes = 0usize;
        for (index, row) in rows.iter().enumerate() {
            if cancelled() {
                return Err(RunError::new(
                    "PREPARATION_CANCELLED",
                    "Prepared data was discarded.",
                    Some("dataset"),
                ));
            }
            if row.len() != names.len() || names.iter().any(|name| !row.contains_key(*name)) {
                return Err(crate::dataset::invalid(
                    "Prepared fields do not match the dataset recipe.",
                ));
            }
            row_bytes = row.iter().fold(row_bytes, |size, (name, value)| {
                size.saturating_add(name.len()).saturating_add(value.len())
            });
            if row_bytes > self.config.budget.max_request_bytes.min(8_388_608) as usize {
                return Err(RunError::new(
                    "DATASET_BUDGET_EXCEEDED",
                    "Prepared rows exceed the byte budget or 8 MiB.",
                    Some("dataset"),
                ));
            }
            let config = self
                .resolve_record(environment, row, &mut secrets)
                .map_err(|mut error| {
                    error.field = Some(format!(
                        "dataset.rows.{index}.{}",
                        error.field.as_deref().unwrap_or("request")
                    ));
                    error
                })?;
            if first.is_none() {
                first = Some(config.clone());
            }
            let request = crate::DataRequest {
                scenarios: config.scenarios,
                url: config.url,
                headers: config.headers,
                query: config.query,
                body: config.body,
                auth: config.auth,
            };
            bytes = bytes
                .saturating_add(
                    serde_json::to_vec(&request)
                        .map_err(|_| crate::dataset::invalid("Record encoding failed."))?
                        .len(),
                )
                .saturating_add(std::mem::size_of::<crate::DataRequest>());
            if bytes > self.config.budget.max_request_bytes.min(16_777_216) as usize {
                return Err(RunError::new(
                    "DATASET_BUDGET_EXCEEDED",
                    "Prepared requests exceed the request budget or 16 MiB hard limit.",
                    Some("dataset"),
                ));
            }
            requests.push(request);
        }
        let mut config =
            first.ok_or_else(|| crate::dataset::invalid("Prepare at least one record."))?;
        config.data = Some(crate::PreparedData {
            definition: data.clone(),
            requests,
        });
        Ok(config)
    }

    /// Resolves supported request fields once, preserving JSON escaping and query encoding.
    /// Whole JSON bodies may use one reference that resolves to valid JSON.
    /// File paths are machine-local bindings and never accept substitutions.
    /// Definitions with datasets must use [`Self::resolve_data`].
    ///
    /// # Arguments
    /// * `environment` - Named environment, or `None` for shared variables only.
    /// * `secrets` - Retrieves each referenced OS-store value; never persisted here.
    ///
    /// # Returns
    /// A resolved engine configuration. Call engine preparation before authorization.
    /// Scenario requests retain authored `{{session:name}}` references. Shared/environment
    /// keys cannot contain colons, and scenario replacement values cannot introduce references.
    ///
    /// # Errors
    /// Returns `UNSUPPORTED_TEST_VERSION`, `VARIABLE_UNRESOLVED`, `SECRET_UNAVAILABLE`,
    /// `INVALID_DATASET`, or `INVALID_TEST_DEFINITION`, with the affected field and no resolved values.
    ///
    /// # Examples
    /// ```
    /// let definition = velo_engine::TestDefinition::default();
    /// let config = definition.resolve(None, |_| unreachable!())?;
    /// assert!(config.url.is_empty());
    /// # Ok::<(), velo_engine::RunError>(())
    /// ```
    pub fn resolve(
        &self,
        environment: Option<&str>,
        mut secrets: impl FnMut(&str) -> Result<String, RunError>,
    ) -> Result<RunConfig, RunError> {
        if self.dataset.is_some() {
            return Err(crate::dataset::invalid(
                "Call resolve_data to prepare dataset records.",
            ));
        }
        self.resolve_record(environment, &BTreeMap::new(), &mut secrets)
    }

    fn resolve_record(
        &self,
        environment: Option<&str>,
        row: &BTreeMap<String, String>,
        mut secrets: impl FnMut(&str) -> Result<String, RunError>,
    ) -> Result<RunConfig, RunError> {
        if self.version != 1 {
            return Err(RunError::new(
                "UNSUPPORTED_TEST_VERSION",
                "Use velo.test.json version 1.",
                Some("version"),
            ));
        }
        if self
            .variables
            .keys()
            .chain(self.environments.values().flat_map(|values| values.keys()))
            .chain(row.keys())
            .any(|name| name.contains(':'))
        {
            return Err(invalid(
                "variables",
                "Colons are reserved for secret and session namespaces.",
            ));
        }
        let mut variables = self.variables.clone();
        if let Some(name) = environment {
            variables.extend(
                self.environments
                    .get(name)
                    .ok_or_else(|| invalid("environment", "Select an existing environment."))?
                    .clone(),
            );
        }
        let mut variables: BTreeMap<String, Variable> = variables
            .into_iter()
            .map(|(name, text)| {
                (
                    name,
                    Variable {
                        text,
                        literal: false,
                    },
                )
            })
            .collect();
        variables.extend(row.iter().map(|(name, text)| {
            (
                name.clone(),
                Variable {
                    text: text.clone(),
                    literal: true,
                },
            )
        }));
        let bindings = Bindings {
            values: variables,
            phase: BindingPhase::Standalone,
        };
        let mut config = self.config.clone();
        resolve_request_fields(&mut config, &bindings, &mut secrets)?;
        if let Some(plan) = &mut config.scenarios {
            let bindings = Bindings {
                values: bindings.values,
                phase: BindingPhase::Scenario,
            };
            for step in plan.setup.iter_mut().chain(&mut plan.cleanup).chain(
                plan.scenarios
                    .iter_mut()
                    .flat_map(|scenario| &mut scenario.steps),
            ) {
                step.request = resolve_scenario_request(&step.request, &bindings, &mut secrets)?;
            }
        }
        Ok(config)
    }
}

fn resolve_request_fields(
    config: &mut RunConfig,
    variables: &Bindings,
    mut secrets: impl FnMut(&str) -> Result<String, RunError>,
) -> Result<(), RunError> {
    config.url = substitute_url(&config.url, variables, &mut secrets)?;
    for (field, pairs) in [
        ("headers", &mut config.headers),
        ("query", &mut config.query),
    ] {
        for (index, pair) in pairs.iter_mut().enumerate() {
            pair.name = substitute(
                &pair.name,
                &format!("{field}.{index}.name"),
                variables,
                &mut secrets,
            )?;
            pair.value = substitute(
                &pair.value,
                &format!("{field}.{index}.value"),
                variables,
                &mut secrets,
            )?;
        }
    }
    match &mut config.auth {
        RequestAuth::None => {}
        RequestAuth::ApiKey { name, value, .. } => {
            *name = substitute(name, "auth.name", variables, &mut secrets)?;
            *value = substitute(value, "auth.value", variables, &mut secrets)?;
        }
        RequestAuth::Bearer { token } => {
            *token = substitute(token, "auth.token", variables, &mut secrets)?
        }
        RequestAuth::Basic { username, password } => {
            *username = substitute(username, "auth.username", variables, &mut secrets)?;
            *password = substitute(password, "auth.password", variables, &mut secrets)?;
        }
    }
    match &mut config.body {
        RequestBody::None => {}
        RequestBody::Text(text) => *text = substitute(text, "body.value", variables, &mut secrets)?,
        RequestBody::Json(text) => {
            if text.starts_with("{{") && text.ends_with("}}") && text.matches("{{").count() == 1 {
                *text = substitute(text, "body.value", variables, &mut secrets)?;
                serde_json::from_str::<serde_json::Value>(text)
                    .map_err(|_| invalid("body.value", "The referenced JSON body is invalid."))?;
                return Ok(());
            }
            let mut value = serde_json::from_str(text).map_err(|_| {
                invalid(
                    "body.value",
                    "Enter valid JSON; use placeholders inside JSON strings.",
                )
            })?;
            json_strings(&mut value, variables, &mut secrets)?;
            *text = serde_json::to_string(&value)
                .map_err(|_| invalid("body.value", "JSON substitution failed."))?;
        }
        RequestBody::Form(pairs) => {
            for pair in pairs {
                pair.name = substitute(&pair.name, "body.name", variables, &mut secrets)?;
                pair.value = substitute(&pair.value, "body.value", variables, &mut secrets)?;
            }
        }
        RequestBody::Multipart(parts) => {
            for part in parts {
                match part {
                    MultipartPart::Text { name, value } => {
                        *name = substitute(name, "body.name", variables, &mut secrets)?;
                        *value = substitute(value, "body.value", variables, &mut secrets)?;
                    }
                    MultipartPart::File {
                        name,
                        path,
                        filename,
                        content_type,
                    } => {
                        *name = substitute(name, "body.name", variables, &mut secrets)?;
                        *filename = substitute(filename, "body.filename", variables, &mut secrets)?;
                        *content_type =
                            substitute(content_type, "body.content_type", variables, &mut secrets)?;
                        if path.is_empty() || path.contains("{{") {
                            return Err(RunError::new(
                                "UPLOAD_REBIND_REQUIRED",
                                "Select the upload file on this machine.",
                                Some("body.path"),
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn resolve_scenario_request(
    request: &crate::ScenarioRequest,
    variables: &Bindings,
    secrets: impl FnMut(&str) -> Result<String, RunError>,
) -> Result<crate::ScenarioRequest, RunError> {
    let mut config = RunConfig {
        url: request.url.clone(),
        method: request.method,
        headers: request.headers.clone(),
        query: request.query.clone(),
        body: request.body.clone(),
        auth: request.auth.clone(),
        ..Default::default()
    };
    resolve_request_fields(&mut config, variables, secrets)?;
    Ok(crate::ScenarioRequest {
        url: config.url,
        method: config.method,
        headers: config.headers,
        query: config.query,
        body: config.body,
        auth: config.auth,
        assertions: request.assertions.clone(),
    })
}

/// Resolves only authored session references after static variable preparation.
/// Session values are literals, never recursively expanded. URL references cannot
/// change the origin, and JSON string substitution preserves escaping. Missing
/// values fail locally with `VARIABLE_UNRESOLVED` before transport starts.
pub(crate) fn bind_scenario_request(
    request: &crate::ScenarioRequest,
    session: &BTreeMap<String, String>,
) -> Result<crate::ScenarioRequest, RunError> {
    let variables = Bindings {
        values: session
            .iter()
            .map(|(name, text)| {
                (
                    format!("session:{name}"),
                    Variable {
                        text: text.clone(),
                        literal: true,
                    },
                )
            })
            .collect(),
        phase: BindingPhase::Session,
    };
    resolve_scenario_request(request, &variables, |_| {
        Err(RunError::new(
            "VARIABLE_UNRESOLVED",
            "Secret references must resolve before execution.",
            None,
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scenario_request() -> crate::ScenarioRequest {
        serde_json::from_value(json!({
            "url": "https://example.com/orders/{{session:id}}",
            "method": "POST", "headers": [], "query": [],
            "body": {"kind": "json", "value": "{\"token\":\"{{session:token}}\"}"},
            "auth": {"kind": "bearer", "token": "{{session:token}}"}
        }))
        .unwrap()
    }

    #[test]
    fn runtime_binding_escapes_values_without_interpreting_them_as_templates() {
        let session = BTreeMap::from([
            ("id".into(), "a/b?admin=true".into()),
            ("token".into(), "quoted\"{{session:other}}".into()),
        ]);
        let request = bind_scenario_request(&scenario_request(), &session).unwrap();
        assert_eq!(
            request.url,
            "https://example.com/orders/a%2Fb%3Fadmin%3Dtrue"
        );
        let RequestBody::Json(body) = request.body else {
            panic!("expected JSON")
        };
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["token"], session["token"]);
        let RequestAuth::Bearer { token } = request.auth else {
            panic!("expected bearer")
        };
        assert_eq!(token, session["token"]);
    }

    #[test]
    fn runtime_binding_rejects_missing_values_and_target_substitution() {
        assert_eq!(
            bind_scenario_request(&scenario_request(), &BTreeMap::new())
                .unwrap_err()
                .code,
            "VARIABLE_UNRESOLVED"
        );
        let mut request = scenario_request();
        request.url = "https://{{session:host}}/orders".into();
        let session = BTreeMap::from([("host".into(), "example.com".into())]);
        assert_eq!(
            bind_scenario_request(&request, &session).unwrap_err().code,
            "INVALID_TEST_DEFINITION"
        );
    }
}
