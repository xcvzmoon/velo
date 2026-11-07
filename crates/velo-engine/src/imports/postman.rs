//! Postman request selection with inherited conversion warnings.

use super::{ImportedRequest, invalid, method, pairs, text, unsupported};
use crate::{RequestBody, RunConfig, RunError};
use serde_json::Value;

pub(super) fn convert(
    items: &[Value],
    inherited: &[String],
    output: &mut Vec<ImportedRequest>,
    depth: usize,
) -> Result<(), RunError> {
    if depth > 32 || output.len() > 1000 {
        return Err(invalid(
            "Collection exceeds 32 folder levels or 1000 requests.",
        ));
    }
    for item in items {
        let mut warnings = inherited.to_vec();
        unsupported(
            item,
            &["name", "item", "request", "response", "description"],
            "collection item",
            &mut warnings,
        );
        if let Some(children) = item.get("item").and_then(Value::as_array) {
            convert(children, &warnings, output, depth + 1)?;
            continue;
        }
        let Some(request) = item.get("request") else {
            continue;
        };
        unsupported(
            request,
            &["method", "header", "url", "body", "description"],
            "Postman request (authentication/scripts are not executed)",
            &mut warnings,
        );
        let mut config = RunConfig {
            method: method(&text(request, "method"))?,
            ..Default::default()
        };
        let url = &request["url"];
        config.url = if let Some(url) = url.as_str() {
            url.into()
        } else {
            text(url, "raw")
        };
        if url.is_object() {
            unsupported(
                url,
                &["raw", "protocol", "host", "path", "query", "variable"],
                "URL",
                &mut warnings,
            );
            if url.get("variable").is_some() {
                warnings.push("URL path variables require explicit environment values and {{name}} placeholders.".into());
            }
            if url.get("query").is_some() {
                if let Some((base, _)) = config.url.clone().split_once('?') {
                    config.url = base.into();
                }
                config.query = pairs(url.get("query"), &mut warnings);
            }
            if config.url.is_empty() {
                warnings.push(
                    "The structured URL has no raw URL. Enter its full target in the editor."
                        .into(),
                );
            }
        }
        config.headers = pairs(request.get("header"), &mut warnings);
        if let Some(body) = request.get("body") {
            unsupported(
                body,
                &["mode", "raw", "urlencoded", "options", "disabled"],
                "Postman body",
                &mut warnings,
            );
            if body.get("disabled").and_then(Value::as_bool) != Some(true) {
                config.body = match body["mode"].as_str() {
                    Some("raw") => {
                        if body["options"]["raw"]["language"] == "json" {
                            RequestBody::Json(text(body, "raw"))
                        } else {
                            RequestBody::Text(text(body, "raw"))
                        }
                    }
                    Some("urlencoded") => {
                        RequestBody::Form(pairs(body.get("urlencoded"), &mut warnings))
                    }
                    _ => {
                        warnings.push("Unsupported body mode, including files: configure it in the editor and select local files.".into());
                        RequestBody::None
                    }
                };
            }
        }
        output.push(ImportedRequest {
            name: text(item, "name"),
            config,
            warnings,
        });
    }
    Ok(())
}
