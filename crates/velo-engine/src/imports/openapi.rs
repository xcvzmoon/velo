//! OpenAPI operations converted into editable request templates.

use super::{ImportedRequest, invalid, method, text, unsupported};
use crate::{NameValue, RunConfig, RunError};
use serde_json::Value;

pub(super) fn convert(value: &Value) -> Result<Vec<ImportedRequest>, RunError> {
    if !text(value, "openapi").starts_with("3.") {
        return Err(invalid("Use an OpenAPI 3 JSON document."));
    }
    let paths = value["paths"]
        .as_object()
        .ok_or_else(|| invalid("OpenAPI needs a paths object."))?;
    let mut base_warnings = Vec::new();
    unsupported(
        value,
        &[
            "openapi",
            "info",
            "servers",
            "paths",
            "tags",
            "externalDocs",
            "components",
        ],
        "OpenAPI document",
        &mut base_warnings,
    );
    let mut output = Vec::new();
    for (path, item) in paths {
        for verb in ["get", "post", "put", "patch", "delete", "head", "options"] {
            let Some(operation) = item.get(verb) else {
                continue;
            };
            let mut warnings = base_warnings.clone();
            unsupported(
                item,
                &[
                    "get",
                    "post",
                    "put",
                    "patch",
                    "delete",
                    "head",
                    "options",
                    "parameters",
                    "servers",
                    "summary",
                    "description",
                ],
                "path",
                &mut warnings,
            );
            unsupported(
                operation,
                &[
                    "operationId",
                    "summary",
                    "description",
                    "tags",
                    "responses",
                    "deprecated",
                    "parameters",
                    "servers",
                    "requestBody",
                    "externalDocs",
                ],
                "operation (security requires manual configuration)",
                &mut warnings,
            );
            let servers = operation
                .get("servers")
                .or_else(|| item.get("servers"))
                .or_else(|| value.get("servers"));
            let base = servers
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .map(|server| text(server, "url"))
                .unwrap_or_default();
            if base.is_empty() || !base.starts_with("http") || base.contains('{') {
                warnings.push("Set an absolute server URL and resolve server variables.".into());
            }
            if servers
                .and_then(Value::as_array)
                .is_some_and(|items| items.len() > 1)
            {
                warnings.push("Only the first server was selected. Review the target URL.".into());
            }
            let mut url = format!("{}{path}", base.trim_end_matches('/'));
            let mut offset = 0;
            while let Some(start) = url[offset..].find('{') {
                let start = offset + start;
                let Some(end) = url[start..].find('}') else {
                    break;
                };
                let end = start + end;
                let placeholder = format!("{{{{{}}}}}", &url[start + 1..end]);
                url.replace_range(start..=end, &placeholder);
                offset = start + placeholder.len();
            }
            let mut config = RunConfig {
                url,
                method: method(&verb.to_uppercase())?,
                ..Default::default()
            };
            for parameters in [item.get("parameters"), operation.get("parameters")] {
                if let Some(Value::Array(parameters)) = parameters {
                    for parameter in parameters {
                        unsupported(
                            parameter,
                            &[
                                "name",
                                "in",
                                "description",
                                "required",
                                "schema",
                                "example",
                                "deprecated",
                            ],
                            "parameter (references and serialization need review)",
                            &mut warnings,
                        );
                        if parameter.get("$ref").is_some() {
                            continue;
                        }
                        let name = text(parameter, "name");
                        let pair = NameValue {
                            value: format!("{{{{{name}}}}}"),
                            name,
                        };
                        match parameter["in"].as_str() {
                            Some("query") => config.query.push(pair),
                            Some("header") => config.headers.push(pair),
                            Some("path") => {}
                            _ => warnings.push(
                                "Unsupported cookie or parameter location; configure it manually."
                                    .into(),
                            ),
                        }
                    }
                }
            }
            if operation.get("requestBody").is_some() {
                warnings.push("Request body schemas, examples, and references are not synthesized. Configure the body in the editor.".into());
            }
            if value.get("components").is_some() {
                warnings.push("Component references and security schemes are not resolved. Configure referenced behavior manually.".into());
            }
            output.push(ImportedRequest {
                name: format!("{} {}", verb.to_uppercase(), path),
                config,
                warnings,
            });
            if output.len() > 1000 {
                return Err(invalid("Import at most 1000 operations."));
            }
        }
    }
    Ok(output)
}
