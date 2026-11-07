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

fn substitute(
    value: &str,
    field: &str,
    variables: &BTreeMap<String, String>,
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
        let variable;
        let secret_name = if let Some(secret) = name.strip_prefix("secret:") {
            Some(secret)
        } else {
            variable = variables.get(name).ok_or_else(|| RunError::new("VARIABLE_UNRESOLVED", "Define the referenced variable in the selected environment or shared variables.", Some(field)))?;
            variable
                .strip_prefix("{{secret:")
                .and_then(|value| value.strip_suffix("}}"))
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
                .get(name)
                .ok_or_else(|| invalid(field, "Define the referenced variable."))?;
            if value.contains("{{") {
                return Err(invalid(
                    field,
                    "Variable values may reference one OS secret, but cannot contain nested variables.",
                ));
            }
            value.clone()
        };
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
    variables: &BTreeMap<String, String>,
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
        if in_path_or_query {
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
    variables: &BTreeMap<String, String>,
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
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let field = error.path().to_string();
            RunError::new(
                "INVALID_TEST_DEFINITION",
                "Check the field type, required properties, and supported values for version 1.",
                Some(&field),
            )
        })
    }

    /// Resolves supported request fields once, preserving JSON escaping and query encoding.
    /// Whole JSON bodies may use one reference that resolves to valid JSON.
    /// File paths are machine-local bindings and never accept substitutions.
    ///
    /// # Arguments
    /// * `environment` - Named environment, or `None` for shared variables only.
    /// * `secrets` - Retrieves each referenced OS-store value; never persisted here.
    ///
    /// # Returns
    /// A resolved engine configuration. Call engine preparation before authorization.
    ///
    /// # Errors
    /// Returns `UNSUPPORTED_TEST_VERSION`, `VARIABLE_UNRESOLVED`, `SECRET_UNAVAILABLE`,
    /// or `INVALID_TEST_DEFINITION`, with the affected field and no resolved values.
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
        if self.version != 1 {
            return Err(RunError::new(
                "UNSUPPORTED_TEST_VERSION",
                "Use velo.test.json version 1.",
                Some("version"),
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
        let mut config = self.config.clone();
        config.url = substitute_url(&config.url, &variables, &mut secrets)?;
        for (field, pairs) in [
            ("headers", &mut config.headers),
            ("query", &mut config.query),
        ] {
            for (index, pair) in pairs.iter_mut().enumerate() {
                pair.name = substitute(
                    &pair.name,
                    &format!("{field}.{index}.name"),
                    &variables,
                    &mut secrets,
                )?;
                pair.value = substitute(
                    &pair.value,
                    &format!("{field}.{index}.value"),
                    &variables,
                    &mut secrets,
                )?;
            }
        }
        match &mut config.auth {
            RequestAuth::None => {}
            RequestAuth::Bearer { token } => {
                *token = substitute(token, "auth.token", &variables, &mut secrets)?
            }
            RequestAuth::Basic { username, password } => {
                *username = substitute(username, "auth.username", &variables, &mut secrets)?;
                *password = substitute(password, "auth.password", &variables, &mut secrets)?;
            }
        }
        match &mut config.body {
            RequestBody::None => {}
            RequestBody::Text(text) => {
                *text = substitute(text, "body.value", &variables, &mut secrets)?
            }
            RequestBody::Json(text) => {
                if text.starts_with("{{") && text.ends_with("}}") && text.matches("{{").count() == 1
                {
                    *text = substitute(text, "body.value", &variables, &mut secrets)?;
                    serde_json::from_str::<serde_json::Value>(text).map_err(|_| {
                        invalid("body.value", "The referenced JSON body is invalid.")
                    })?;
                    return Ok(config);
                }
                let mut value = serde_json::from_str(text).map_err(|_| {
                    invalid(
                        "body.value",
                        "Enter valid JSON; use placeholders inside JSON strings.",
                    )
                })?;
                json_strings(&mut value, &variables, &mut secrets)?;
                *text = serde_json::to_string(&value)
                    .map_err(|_| invalid("body.value", "JSON substitution failed."))?;
            }
            RequestBody::Form(pairs) => {
                for pair in pairs {
                    pair.name = substitute(&pair.name, "body.name", &variables, &mut secrets)?;
                    pair.value = substitute(&pair.value, "body.value", &variables, &mut secrets)?;
                }
            }
            RequestBody::Multipart(parts) => {
                for part in parts {
                    match part {
                        MultipartPart::Text { name, value } => {
                            *name = substitute(name, "body.name", &variables, &mut secrets)?;
                            *value = substitute(value, "body.value", &variables, &mut secrets)?;
                        }
                        MultipartPart::File {
                            name,
                            path,
                            filename,
                            content_type,
                        } => {
                            *name = substitute(name, "body.name", &variables, &mut secrets)?;
                            *filename =
                                substitute(filename, "body.filename", &variables, &mut secrets)?;
                            *content_type = substitute(
                                content_type,
                                "body.content_type",
                                &variables,
                                &mut secrets,
                            )?;
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
        Ok(config)
    }
}
