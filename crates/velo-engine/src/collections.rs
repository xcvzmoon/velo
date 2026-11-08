//! Ordered local API collections with explicit inheritance and portable secret references.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    Extraction, NameValue, RequestAuth, RequestBody, RunError, Scenario, ScenarioPlan,
    ScenarioRequest, ScenarioStep, StepFailure, TestDefinition,
};

/// Matches test-definition import bounds so authored artifacts cannot grow without limit.
const DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
/// Bounds inherited-default expansion independently of the smaller authored document.
const FLATTENED_BYTES: usize = 8 * 1024 * 1024;
/// Bounds the retained tree and the maximum flattened selection before execution admission.
const ENTRY_LIMIT: usize = 1000;
/// Bounds recursive traversal and keeps nested collection navigation usable.
const DEPTH_LIMIT: usize = 8;

/// Header, authentication, and variable defaults inherited by descendants.
///
/// Inner scopes replace matching header names and variable keys. Duplicate headers
/// authored within one scope retain their order. Authentication is a single value.
///
/// # Examples
/// ```
/// let defaults = velo_engine::CollectionDefaults::default();
/// assert!(defaults.headers.is_empty());
/// ```
#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectionDefaults {
    /// Ordered headers overridden case-insensitively by an inner scope.
    #[serde(default)]
    #[ts(type = "NameValue[]")]
    pub headers: Vec<NameValue>,
    /// Explicit authentication override; absence inherits the parent.
    #[serde(default)]
    pub auth: Option<RequestAuth>,
    /// Shared values or secret references, overridden by inner scopes and environments.
    #[serde(default)]
    #[ts(type = "Record<string, string>")]
    pub variables: BTreeMap<String, String>,
}

/// A folder or request in the collection's displayed execution order.
///
/// # Examples
/// ```
/// # fn example(entry: velo_engine::CollectionEntry) {
/// assert!(!entry.id().is_empty());
/// # }
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollectionEntry {
    /// A nested scope whose descendants follow the stored array order.
    Folder {
        /// Identity unique across the entire collection.
        id: String,
        /// Display name, at most 160 bytes.
        name: String,
        /// Purpose and context, at most 16 KiB.
        #[serde(default)]
        description: String,
        /// Usage notes, at most 16 KiB.
        #[serde(default)]
        notes: String,
        /// Defaults applied after the parent scope.
        #[serde(default)]
        defaults: CollectionDefaults,
        /// Ordered folders and saved requests.
        #[ts(type = "CollectionEntry[]")]
        entries: Vec<CollectionEntry>,
    },
    /// A shared definition with functional extraction rules.
    Request {
        /// Identity unique across the entire collection.
        id: String,
        /// Display name, at most 160 bytes.
        name: String,
        /// Purpose and context, at most 16 KiB.
        #[serde(default)]
        description: String,
        /// Usage notes, at most 16 KiB.
        #[serde(default)]
        notes: String,
        /// Canonical shared request, variables, environment overrides and dataset recipe.
        definition: Box<TestDefinition>,
        /// Inherit defaults when request authentication is None; false explicitly disables auth.
        #[serde(default = "inherit_auth_default")]
        inherit_auth: bool,
        /// Response values made available to later requests in the session namespace.
        #[serde(default)]
        #[ts(type = "Extraction[]")]
        extractions: Vec<Extraction>,
    },
}

fn inherit_auth_default() -> bool {
    true
}

impl CollectionEntry {
    /// Returns the stable identity without inspecting request content.
    ///
    /// # Returns
    /// The folder or request identifier. No resources are acquired.
    ///
    /// # Examples
    /// ```
    /// # fn example(entry: &velo_engine::CollectionEntry) {
    /// let id = entry.id();
    /// assert!(!id.is_empty());
    /// # }
    /// ```
    pub fn id(&self) -> &str {
        match self {
            Self::Folder { id, .. } | Self::Request { id, .. } => id,
        }
    }
}

/// A versioned collection saved locally or exchanged as a portable JSON document.
///
/// Literal credentials in known sensitive fields are rejected. OS-store references
/// remain references; portable copies remove local upload bindings.
///
/// # Examples
/// ```
/// let collection = velo_engine::ApiCollection::default();
/// assert_eq!(collection.version, 1);
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ApiCollection {
    /// Portable format version, currently 1.
    pub version: u32,
    /// Stable local identity, 1 to 80 ASCII letters, digits, underscores or hyphens.
    pub id: String,
    /// Display name, at most 160 bytes.
    pub name: String,
    /// Purpose and context, at most 16 KiB.
    #[serde(default)]
    pub description: String,
    /// Usage notes, at most 16 KiB.
    #[serde(default)]
    pub notes: String,
    /// Root inherited scope.
    #[serde(default)]
    pub defaults: CollectionDefaults,
    /// Named overrides applied after inherited and request variables.
    #[serde(default)]
    #[ts(type = "Record<string, Record<string, string>>")]
    pub environments: BTreeMap<String, BTreeMap<String, String>>,
    /// JSON pointers masked after assertions and extraction, before display, history or export.
    #[serde(default)]
    #[ts(type = "string[]")]
    pub sensitive_response_fields: Vec<String>,
    /// Ordered folders and saved requests.
    #[ts(type = "CollectionEntry[]")]
    pub entries: Vec<CollectionEntry>,
}

impl Default for ApiCollection {
    fn default() -> Self {
        Self {
            version: 1,
            id: "collection".into(),
            name: "New collection".into(),
            description: String::new(),
            notes: String::new(),
            defaults: CollectionDefaults::default(),
            environments: BTreeMap::new(),
            sensitive_response_fields: Vec::new(),
            entries: Vec::new(),
        }
    }
}

/// A saved request after applying its collection and folder defaults.
///
/// Variables remain unresolved, and no credentials or upload files are opened.
///
/// # Examples
/// ```
/// # fn example(request: velo_engine::CollectionRequest) {
/// assert!(request.definition.config.scenarios.is_none());
/// # }
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectionRequest {
    /// Original saved request identity.
    pub id: String,
    /// Original saved request name.
    pub name: String,
    /// Shared request with inherited settings applied.
    pub definition: TestDefinition,
    /// Ordered response extraction rules.
    #[ts(type = "Extraction[]")]
    pub extractions: Vec<Extraction>,
}

impl std::fmt::Debug for CollectionDefaults {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CollectionDefaults([redacted])")
    }
}

impl std::fmt::Debug for CollectionEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CollectionEntry([redacted])")
    }
}

impl std::fmt::Debug for ApiCollection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ApiCollection([redacted])")
    }
}

impl std::fmt::Debug for CollectionRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CollectionRequest([redacted])")
    }
}

fn invalid(message: &str) -> RunError {
    RunError::new("INVALID_API_COLLECTION", message, Some("collection"))
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn metadata(id: &str, name: &str, description: &str, notes: &str) -> Result<(), RunError> {
    if !valid_id(id)
        || name.trim().is_empty()
        || name.len() > 160
        || name.chars().any(char::is_control)
        || description.len() > 16_384
        || notes.len() > 16_384
    {
        return Err(invalid(
            "Use valid unique identifiers, names up to 160 bytes, and descriptions and notes up to 16 KiB.",
        ));
    }
    Ok(())
}

fn sensitive(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '_'], "");
    matches!(
        normalized.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "password"
            | "passwd"
            | "secret"
            | "clientsecret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "apikey"
            | "xapikey"
    )
}

/// Unknown field-name bindings may become credential names at execution, so their values stay protected.
fn sensitive_name(name: &str, variables: &BTreeMap<String, String>) -> bool {
    if !name.contains("{{") {
        return sensitive(name);
    }
    let mut resolved = String::new();
    let mut remaining = name;
    while let Some((prefix, tail)) = remaining.split_once("{{") {
        let Some((variable, tail)) = tail.split_once("}}") else {
            return true;
        };
        let Some(value) = variables.get(variable) else {
            return true;
        };
        if value.contains("{{")
            || resolved
                .len()
                .saturating_add(prefix.len())
                .saturating_add(value.len())
                > DOCUMENT_BYTES
        {
            return true;
        }
        resolved.push_str(prefix);
        resolved.push_str(value);
        remaining = tail;
    }
    if resolved.len().saturating_add(remaining.len()) > DOCUMENT_BYTES {
        return true;
    }
    resolved.push_str(remaining);
    sensitive(&resolved)
}

fn reference(value: &str) -> bool {
    value.is_empty()
        || (value.starts_with("{{")
            && value.ends_with("}}")
            && !value[2..value.len() - 2].contains(['{', '}']))
}

fn private_value(value: &str) -> Result<(), RunError> {
    if !reference(value) {
        return Err(RunError::new(
            "COLLECTION_SECRET_REQUIRED",
            "Use a variable or OS secret reference for sensitive fields before saving or exporting.",
            Some("collection"),
        ));
    }
    Ok(())
}

fn inspect_json(value: &serde_json::Value) -> Result<(), RunError> {
    match value {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if sensitive(key) {
                    match value {
                        serde_json::Value::String(text) => private_value(text)?,
                        serde_json::Value::Null => {}
                        _ => {
                            return Err(invalid(
                                "Sensitive JSON fields must use variable or secret references.",
                            ));
                        }
                    }
                } else {
                    inspect_json(value)?;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                inspect_json(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_auth(auth: &RequestAuth) -> Result<(), RunError> {
    let value =
        serde_json::to_value(auth).map_err(|_| invalid("Authentication encoding failed."))?;
    inspect_json(&value)?;
    if let Some(key) = value.get("value").and_then(serde_json::Value::as_str) {
        private_value(key)?;
    }
    Ok(())
}

fn check_pairs(pairs: &[NameValue]) -> Result<(), RunError> {
    for pair in pairs {
        if sensitive(&pair.name) {
            private_value(&pair.value)?;
        }
    }
    Ok(())
}

fn check_variables(variables: &BTreeMap<String, String>) -> Result<(), RunError> {
    for (name, value) in variables {
        if sensitive(name) {
            private_value(value)?;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
            inspect_json(&json)?;
        }
        if let Ok(url) = reqwest::Url::parse(value)
            && (!url.username().is_empty() || url.password().is_some())
        {
            return Err(invalid(
                "Use authentication controls instead of credentials in URL variables.",
            ));
        }
    }
    Ok(())
}

fn check_defaults(defaults: &CollectionDefaults) -> Result<(), RunError> {
    check_pairs(&defaults.headers)?;
    check_variables(&defaults.variables)?;
    if let Some(auth) = &defaults.auth {
        check_auth(auth)?;
    }
    Ok(())
}

fn secret_alias(value: &str, variables: &BTreeMap<String, String>) -> Result<(), RunError> {
    let Some(name) = value
        .strip_prefix("{{")
        .and_then(|value| value.strip_suffix("}}"))
    else {
        return private_value(value);
    };
    if !name.starts_with("secret:")
        && !name.starts_with("session:")
        && let Some(value) = variables.get(name)
        && (!value.starts_with("{{secret:") || !reference(value))
    {
        return Err(RunError::new(
            "COLLECTION_SECRET_REQUIRED",
            "Sensitive variable aliases must point to an OS secret reference, not a literal value.",
            Some("collection"),
        ));
    }
    Ok(())
}

fn check_alias_json(
    value: &serde_json::Value,
    variables: &BTreeMap<String, String>,
    auth: bool,
) -> Result<(), RunError> {
    match value {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if sensitive_name(key, variables) || (auth && key == "value") {
                    match value {
                        serde_json::Value::String(value) => secret_alias(value, variables)?,
                        serde_json::Value::Null => {}
                        _ => {
                            return Err(invalid(
                                "Sensitive JSON fields must use variable or secret references.",
                            ));
                        }
                    }
                } else {
                    check_alias_json(value, variables, auth)?;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                check_alias_json(value, variables, auth)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn authored_query(source: &str) -> Option<reqwest::Url> {
    let (_, query) = source.split_once('?')?;
    let query = query.split('#').next().unwrap_or_default();
    reqwest::Url::parse(&format!("https://collection.invalid/?{query}")).ok()
}

fn check_request_aliases(definition: &TestDefinition) -> Result<(), RunError> {
    let auth = serde_json::to_value(&definition.config.auth)
        .map_err(|_| invalid("Authentication encoding failed."))?;
    let body = match &definition.config.body {
        RequestBody::Json(body) => serde_json::from_str::<serde_json::Value>(body).ok(),
        _ => None,
    };
    for environment in std::iter::once(None).chain(definition.environments.values().map(Some)) {
        let mut variables = definition.variables.clone();
        if let Some(environment) = environment {
            variables.extend(environment.clone());
        }
        if let Some(dataset) = &definition.dataset {
            match &dataset.source {
                crate::DataSource::Faker { fields, .. } => {
                    let mut literals: BTreeMap<String, String> = BTreeMap::new();
                    for field in fields {
                        let literal = match &field.generator {
                            crate::DataGenerator::Literal { value } => Some(value.clone()),
                            crate::DataGenerator::Copy { field } => literals.get(field).cloned(),
                            _ => None,
                        };
                        if let Some(value) = literal {
                            literals.insert(field.name.clone(), value.clone());
                            variables.insert(field.name.clone(), value);
                        } else {
                            variables.remove(&field.name);
                        }
                    }
                }
                crate::DataSource::Csv { mapping } | crate::DataSource::Json { mapping } => {
                    for name in mapping.keys() {
                        variables.remove(name);
                    }
                }
            }
        }
        check_variables(&variables)?;
        check_alias_json(&auth, &variables, true)?;
        if let Some(body) = &body {
            check_alias_json(body, &variables, false)?;
        }
        for pair in definition
            .config
            .headers
            .iter()
            .chain(&definition.config.query)
        {
            if sensitive_name(&pair.name, &variables) {
                secret_alias(&pair.value, &variables)?;
            }
        }
        match &definition.config.body {
            RequestBody::Form(pairs) => {
                for pair in pairs {
                    if sensitive_name(&pair.name, &variables) {
                        secret_alias(&pair.value, &variables)?;
                    }
                }
            }
            RequestBody::Multipart(parts) => {
                for part in parts {
                    if let crate::MultipartPart::Text { name, value } = part
                        && sensitive_name(name, &variables)
                    {
                        secret_alias(value, &variables)?;
                    }
                }
            }
            _ => {}
        }
        if let Some(url) = authored_query(&definition.config.url) {
            for (name, value) in url.query_pairs() {
                if sensitive_name(&name, &variables) {
                    secret_alias(&value, &variables)?;
                }
            }
        }
    }
    Ok(())
}

fn check_definition(definition: &TestDefinition) -> Result<(), RunError> {
    if definition.version != 1
        || definition.config.scenarios.is_some()
        || definition.config.data.is_some()
    {
        return Err(invalid(
            "Save one request per collection entry, without scenario plans or prepared data.",
        ));
    }
    if let Some(dataset) = &definition.dataset {
        dataset.validate()?;
    }
    check_request_credentials(definition)
}

fn check_request_credentials(definition: &TestDefinition) -> Result<(), RunError> {
    check_auth(&definition.config.auth)?;
    if !matches!(definition.config.auth, RequestAuth::None)
        && definition.config.headers.iter().any(|pair| {
            pair.name.eq_ignore_ascii_case("authorization")
                || pair.name.eq_ignore_ascii_case("proxy-authorization")
        })
    {
        return Err(invalid(
            "Choose authentication controls or explicit authorization headers for a request, not both.",
        ));
    }
    check_pairs(&definition.config.headers)?;
    check_pairs(&definition.config.query)?;
    check_variables(&definition.variables)?;
    for values in definition.environments.values() {
        check_variables(values)?;
    }
    if definition
        .config
        .url
        .split_once("://")
        .is_some_and(|(_, authority)| {
            authority
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
    {
        return Err(invalid(
            "Use authentication controls instead of credentials in URLs.",
        ));
    }
    if let Some(url) = authored_query(&definition.config.url) {
        for (name, value) in url.query_pairs() {
            if sensitive(&name) {
                private_value(&value)?;
            }
        }
    }
    match &definition.config.body {
        RequestBody::Form(pairs) => check_pairs(pairs)?,
        RequestBody::Json(body) => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
                inspect_json(&value)?;
            }
        }
        RequestBody::Multipart(parts) => {
            for part in parts {
                if let crate::MultipartPart::Text { name, value } = part
                    && sensitive(name)
                {
                    private_value(value)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Rejects literal credentials before saving or exporting a shared test definition.
///
/// # Arguments
/// * `definition` - Editable request or scenario document with shared variables and environments.
///
/// # Returns
/// No value. Secret and session references remain unresolved; no OS store, files, or network are accessed.
/// Root request fields and every setup, journey, and cleanup step use the same credential policy.
/// This check does not validate execution settings or reject scenario formats.
///
/// # Errors
/// Returns `DEFINITION_SECRET_REQUIRED` for recognized sensitive literal values or aliases,
/// or `INVALID_DEFINITION_CREDENTIALS` for URL credentials or unsupported sensitive field structures.
/// Error messages omit credential values. Arbitrary custom content cannot be classified as a secret.
///
/// # Examples
/// ```
/// let mut definition = velo_engine::TestDefinition::default();
/// definition.config.auth = velo_engine::RequestAuth::Bearer {
///     token: "{{secret:api-token}}".into(),
/// };
/// velo_engine::validate_definition_credentials(&definition)?;
/// # Ok::<(), velo_engine::RunError>(())
/// ```
pub fn validate_definition_credentials(definition: &TestDefinition) -> Result<(), RunError> {
    check_definition_requests(definition).map_err(|mut error| {
        error.code = if error.code == "COLLECTION_SECRET_REQUIRED" {
            "DEFINITION_SECRET_REQUIRED"
        } else {
            "INVALID_DEFINITION_CREDENTIALS"
        }
        .into();
        error.field = Some("credentials".into());
        error
    })
}

fn check_definition_requests(definition: &TestDefinition) -> Result<(), RunError> {
    check_request_credentials(definition)?;
    check_request_aliases(definition)?;
    if let Some(plan) = &definition.config.scenarios {
        for step in plan
            .setup
            .iter()
            .chain(&plan.cleanup)
            .chain(plan.scenarios.iter().flat_map(|scenario| &scenario.steps))
        {
            let request = TestDefinition {
                config: crate::scenario_runtime::request_config(
                    &definition.config,
                    step.request.clone(),
                ),
                variables: definition.variables.clone(),
                environments: definition.environments.clone(),
                dataset: definition.dataset.clone(),
                version: definition.version,
            };
            check_request_credentials(&request)?;
            check_request_aliases(&request)?;
        }
    }
    Ok(())
}

fn check_entries(
    entries: &[CollectionEntry],
    depth: usize,
    identities: &mut BTreeSet<String>,
) -> Result<(), RunError> {
    if depth > DEPTH_LIMIT {
        return Err(invalid("Collections support at most eight folder levels."));
    }
    for entry in entries {
        if identities.len() >= ENTRY_LIMIT || !identities.insert(entry.id().to_owned()) {
            return Err(invalid(
                "Use unique identifiers and at most 1,000 entries per collection.",
            ));
        }
        match entry {
            CollectionEntry::Folder {
                id,
                name,
                description,
                notes,
                defaults,
                entries,
            } => {
                metadata(id, name, description, notes)?;
                check_defaults(defaults)?;
                check_entries(entries, depth + 1, identities)?;
            }
            CollectionEntry::Request {
                id,
                name,
                description,
                notes,
                definition,
                extractions,
                ..
            } => {
                metadata(id, name, description, notes)?;
                check_definition(definition)?;
                if extractions.len() > 32 {
                    return Err(invalid("Use at most 32 extractions per request."));
                }
            }
        }
    }
    Ok(())
}

fn merge_headers(parent: &[NameValue], child: &[NameValue]) -> Vec<NameValue> {
    let mut merged = Vec::with_capacity(parent.len() + child.len());
    for pair in parent {
        if !child
            .iter()
            .any(|override_| override_.name.eq_ignore_ascii_case(&pair.name))
        {
            merged.push(pair.clone());
        }
    }
    merged.extend_from_slice(child);
    merged
}

fn merge_defaults(parent: &CollectionDefaults, child: &CollectionDefaults) -> CollectionDefaults {
    let mut variables = parent.variables.clone();
    variables.extend(child.variables.clone());
    CollectionDefaults {
        headers: merge_headers(&parent.headers, &child.headers),
        auth: child.auth.clone().or_else(|| parent.auth.clone()),
        variables,
    }
}

fn collect(
    entries: &[CollectionEntry],
    defaults: &CollectionDefaults,
    environments: &BTreeMap<String, BTreeMap<String, String>>,
    selected: &BTreeSet<&str>,
    include: bool,
    output: &mut Vec<CollectionRequest>,
    bytes: &mut usize,
) -> Result<(), RunError> {
    check_request_aliases(&TestDefinition {
        config: crate::RunConfig {
            headers: defaults.headers.clone(),
            auth: defaults.auth.clone().unwrap_or_default(),
            ..Default::default()
        },
        variables: defaults.variables.clone(),
        environments: environments.clone(),
        ..Default::default()
    })?;
    for entry in entries {
        let include = include || selected.contains(entry.id());
        match entry {
            CollectionEntry::Folder {
                defaults: child,
                entries,
                ..
            } => collect(
                entries,
                &merge_defaults(defaults, child),
                environments,
                selected,
                include,
                output,
                bytes,
            )?,
            CollectionEntry::Request {
                id,
                name,
                definition,
                inherit_auth,
                extractions,
                ..
            } if include => {
                let mut definition = definition.as_ref().clone();
                let explicit_authorization = definition.config.headers.iter().any(|pair| {
                    pair.name.eq_ignore_ascii_case("authorization")
                        || pair.name.eq_ignore_ascii_case("proxy-authorization")
                });
                definition.config.headers =
                    merge_headers(&defaults.headers, &definition.config.headers);
                if *inherit_auth
                    && matches!(definition.config.auth, RequestAuth::None)
                    && !explicit_authorization
                {
                    definition.config.auth = defaults.auth.clone().unwrap_or_default();
                }
                if !matches!(definition.config.auth, RequestAuth::None) {
                    definition.config.headers.retain(|pair| {
                        !pair.name.eq_ignore_ascii_case("authorization")
                            && !pair.name.eq_ignore_ascii_case("proxy-authorization")
                    });
                }
                let mut variables = defaults.variables.clone();
                variables.extend(definition.variables);
                definition.variables = variables;
                let mut inherited = environments.clone();
                for (name, values) in definition.environments {
                    inherited.entry(name).or_default().extend(values);
                }
                definition.environments = inherited;
                let request = CollectionRequest {
                    id: id.clone(),
                    name: name.clone(),
                    definition,
                    extractions: extractions.clone(),
                };
                *bytes = bytes.saturating_add(
                    serde_json::to_vec(&request)
                        .map_err(|_| invalid("Collection encoding failed."))?
                        .len(),
                );
                if *bytes > FLATTENED_BYTES {
                    return Err(RunError::new(
                        "COLLECTION_BUDGET_EXCEEDED",
                        "Inherited collection requests exceed the 8 MiB preparation budget. Reduce shared defaults or the number of requests.",
                        Some("collection"),
                    ));
                }
                output.push(request);
            }
            CollectionEntry::Request { .. } => {}
        }
    }
    Ok(())
}

fn unbind(entries: &mut [CollectionEntry]) {
    for entry in entries {
        match entry {
            CollectionEntry::Folder { entries, .. } => unbind(entries),
            CollectionEntry::Request { definition, .. } => {
                if let RequestBody::Multipart(parts) = &mut definition.config.body {
                    for part in parts {
                        if let crate::MultipartPart::File { path, .. } = part {
                            path.clear();
                        }
                    }
                }
            }
        }
    }
}

impl ApiCollection {
    /// Parses a bounded portable document without resolving values or opening upload files.
    ///
    /// # Arguments
    /// * `source` - UTF-8 JSON, at most 8 MiB.
    /// # Returns
    /// A validated collection with upload paths removed. No traffic is sent.
    /// # Errors
    /// Returns `INVALID_API_COLLECTION`, `UNSUPPORTED_COLLECTION_VERSION`, or
    /// `COLLECTION_SECRET_REQUIRED` when input is malformed, unsupported, or contains literal credentials.
    /// # Examples
    /// ```
    /// let source = serde_json::to_string(&velo_engine::ApiCollection::default())?;
    /// let collection = velo_engine::ApiCollection::parse(&source)?;
    /// assert_eq!(collection.version, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(source: &str) -> Result<Self, RunError> {
        if source.len() > DOCUMENT_BYTES {
            return Err(invalid("Collection exceeds 8 MiB."));
        }
        let collection: Self = serde_json::from_str(source)
            .map_err(|_| invalid("Choose a valid Velo collection JSON document."))?;
        collection.portable()
    }

    /// Validates structure, bounds and references before storage or execution.
    ///
    /// # Returns
    /// Unit for a valid collection. Upload paths remain bound for local execution.
    /// # Errors
    /// Returns `INVALID_API_COLLECTION`, `UNSUPPORTED_COLLECTION_VERSION`, or
    /// `COLLECTION_SECRET_REQUIRED` for invalid structure or known sensitive literal fields.
    /// Dataset validation errors are propagated unchanged. `COLLECTION_BUDGET_EXCEEDED`
    /// bounds expanded inherited settings to 8 MiB before preparation.
    /// # Examples
    /// ```
    /// velo_engine::ApiCollection::default().validate()?;
    /// # Ok::<(), velo_engine::RunError>(())
    /// ```
    pub fn validate(&self) -> Result<(), RunError> {
        if self.version != 1 {
            return Err(RunError::new(
                "UNSUPPORTED_COLLECTION_VERSION",
                "Use Velo collection version 1.",
                Some("version"),
            ));
        }
        metadata(&self.id, &self.name, &self.description, &self.notes)?;
        if self.sensitive_response_fields.len() > 32
            || self.sensitive_response_fields.iter().any(|pointer| {
                pointer.len() > 1024
                    || (!pointer.is_empty() && !pointer.starts_with('/'))
                    || pointer.contains("{{")
                    || pointer.as_bytes().iter().enumerate().any(|(index, byte)| {
                        *byte == b'~'
                            && !matches!(pointer.as_bytes().get(index + 1), Some(b'0' | b'1'))
                    })
            })
        {
            return Err(invalid(
                "Configure at most 32 literal JSON pointers of at most 1,024 bytes for sensitive response fields.",
            ));
        }
        check_entries(&self.entries, 0, &mut BTreeSet::new())?;
        let encoded =
            serde_json::to_string(self).map_err(|_| invalid("Collection encoding failed."))?;
        if encoded.len() > DOCUMENT_BYTES {
            return Err(invalid("Collection exceeds 8 MiB."));
        }
        if encoded.contains("{{secret:saved.") {
            return Err(invalid(
                "Replace machine-specific saved credentials with named OS secret references.",
            ));
        }
        check_defaults(&self.defaults)?;
        for values in self.environments.values() {
            check_variables(values)?;
        }
        let mut requests = Vec::new();
        collect(
            &self.entries,
            &self.defaults,
            &self.environments,
            &BTreeSet::new(),
            true,
            &mut requests,
            &mut 0,
        )?;
        for request in requests {
            check_request_aliases(&request.definition)?;
        }
        Ok(())
    }

    /// Creates an export-safe copy without machine-specific upload bindings.
    ///
    /// # Returns
    /// A validated copy with file paths cleared. Secret references remain unresolved.
    /// # Errors
    /// Propagates validation errors from [`Self::validate`].
    /// # Examples
    /// ```
    /// let portable = velo_engine::ApiCollection::default().portable()?;
    /// assert_eq!(portable.version, 1);
    /// # Ok::<(), velo_engine::RunError>(())
    /// ```
    pub fn portable(&self) -> Result<Self, RunError> {
        self.validate()?;
        let mut portable = self.clone();
        unbind(&mut portable.entries);
        Ok(portable)
    }

    /// Resolves inherited scopes for one saved request without sending it.
    ///
    /// # Arguments
    /// * `id` - Saved request identifier, not a folder identifier.
    /// # Returns
    /// The canonical definition and extraction rules with all parent defaults applied.
    /// Environment values and secrets remain unresolved.
    /// # Errors
    /// Propagates collection validation errors or `COLLECTION_REQUEST_NOT_FOUND`.
    /// # Examples
    /// ```
    /// let collection = velo_engine::ApiCollection::default();
    /// assert!(collection.request("missing").is_err());
    /// ```
    pub fn request(&self, id: &str) -> Result<CollectionRequest, RunError> {
        self.requests(&[id.to_owned()])?
            .into_iter()
            .find(|request| request.id == id)
            .ok_or_else(|| {
                RunError::new(
                    "COLLECTION_REQUEST_NOT_FOUND",
                    "Choose an existing saved request.",
                    Some("request"),
                )
            })
    }

    /// Flattens selected requests and folders in their displayed deterministic order.
    ///
    /// # Arguments
    /// * `selection` - Request or folder identifiers; an empty slice selects the entire collection.
    /// # Returns
    /// Each selected descendant once, with collection and folder defaults applied.
    /// Inner header names replace outer ones case-insensitively; request variables
    /// override folder variables, and environment overrides apply during shared resolution.
    /// # Errors
    /// Propagates collection validation errors or `COLLECTION_SELECTION_INVALID` for missing selections.
    /// # Examples
    /// ```
    /// let requests = velo_engine::ApiCollection::default().requests(&[])?;
    /// assert!(requests.is_empty());
    /// # Ok::<(), velo_engine::RunError>(())
    /// ```
    pub fn requests(&self, selection: &[String]) -> Result<Vec<CollectionRequest>, RunError> {
        self.validate()?;
        let mut identities = BTreeSet::new();
        check_entries(&self.entries, 0, &mut identities)?;
        if selection.iter().any(|id| !identities.contains(id)) {
            return Err(RunError::new(
                "COLLECTION_SELECTION_INVALID",
                "The selection contains an entry that no longer exists.",
                Some("selection"),
            ));
        }
        let selected: BTreeSet<&str> = selection.iter().map(String::as_str).collect();
        let mut requests = Vec::new();
        collect(
            &self.entries,
            &self.defaults,
            &self.environments,
            &selected,
            selection.is_empty(),
            &mut requests,
            &mut 0,
        )?;
        Ok(requests)
    }

    /// Converts a compatible ordered flow into a scenario for explicit load review.
    ///
    /// # Arguments
    /// * `selection` - Request or folder identifiers; empty selects the whole collection.
    /// # Returns
    /// A shared test definition with one ordered scenario. Conversion never sends traffic.
    /// # Errors
    /// Propagates collection validation errors or `COLLECTION_SCENARIO_UNSUPPORTED` when
    /// requests use different transport, variable, dataset or inspection settings, or the
    /// flow has zero or more than 32 steps. Unsupported settings are never discarded.
    /// # Examples
    /// ```
    /// assert!(velo_engine::ApiCollection::default().scenario(&[]).is_err());
    /// ```
    pub fn scenario(&self, selection: &[String]) -> Result<TestDefinition, RunError> {
        let requests = self.requests(selection)?;
        let unsupported = || {
            RunError::new(
                "COLLECTION_SCENARIO_UNSUPPORTED",
                "Review conversion: use 1 to 32 requests with identical variables, environments, dataset recipes, timeouts, redirects, protocol, inspection and resource budgets. Per-request transport or variable scopes cannot be represented by a shared scenario.",
                Some("selection"),
            )
        };
        let Some(first) = requests.first() else {
            return Err(unsupported());
        };
        if requests.len() > 32 || requests.iter().any(|request| request.id.len() > 64) {
            return Err(unsupported());
        }
        fn settings(definition: &TestDefinition) -> serde_json::Value {
            serde_json::json!({"variables":definition.variables,"environments":definition.environments,"dataset":definition.dataset,"timeout":definition.config.timeout_ms,"redirects":definition.config.max_redirects,"protocol":definition.config.protocol,"inspection":definition.config.inspection,"budget":definition.config.budget})
        }
        let expected = settings(&first.definition);
        if requests
            .iter()
            .any(|request| settings(&request.definition) != expected)
        {
            return Err(unsupported());
        }
        let mut definition = first.definition.clone();
        let steps = requests
            .into_iter()
            .map(|request| {
                let config = request.definition.config;
                ScenarioStep {
                    id: request.id,
                    request: ScenarioRequest {
                        url: config.url,
                        method: config.method,
                        headers: config.headers,
                        query: config.query,
                        body: config.body,
                        auth: config.auth,
                        assertions: config.assertions,
                    },
                    extractions: request.extractions,
                    think_ms: 0,
                    on_failure: StepFailure::Stop,
                }
            })
            .collect();
        definition.config.scenarios = Some(ScenarioPlan {
            seed: 0,
            setup: Vec::new(),
            cleanup: Vec::new(),
            scenarios: vec![Scenario {
                id: "collection".into(),
                weight: 1,
                steps,
            }],
        });
        Ok(definition)
    }
}
