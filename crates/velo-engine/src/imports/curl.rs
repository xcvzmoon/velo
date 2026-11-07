//! cURL token conversion without shell evaluation.

use super::{ImportedRequest, invalid, method};
use crate::{NameValue, RequestAuth, RequestBody, RunConfig, RunError};

pub(super) fn convert(source: &str) -> Result<Vec<ImportedRequest>, RunError> {
    let tokens = shlex::split(source).ok_or_else(|| invalid("Close all command quotes."))?;
    if tokens.first().map(String::as_str) != Some("curl") {
        return Err(invalid("Start the command with curl."));
    }
    let mut config = RunConfig::default();
    let mut warnings = Vec::new();
    let mut explicit_method = false;
    let mut follow_redirects = false;
    let mut redirect_limit = 10;
    let mut index = 1;
    while index < tokens.len() {
        let token = &tokens[index];
        if matches!(
            token.as_str(),
            "-X" | "--request"
                | "-H"
                | "--header"
                | "-d"
                | "--data"
                | "--data-raw"
                | "--data-binary"
                | "--url"
                | "-u"
                | "--user"
                | "--max-time"
                | "--max-redirs"
        ) {
            index += 1;
            let value = tokens
                .get(index)
                .ok_or_else(|| invalid("A cURL option is missing its value."))?;
            match token.as_str() {
                "-X" | "--request" => {
                    config.method = method(value)?;
                    explicit_method = true;
                }
                "-H" | "--header" => {
                    let (name, value) = value
                        .split_once(':')
                        .ok_or_else(|| invalid("Use Name: value for headers."))?;
                    config.headers.push(NameValue {
                        name: name.trim().into(),
                        value: value.trim_start().into(),
                    });
                }
                "-d" | "--data" | "--data-raw" | "--data-binary" => {
                    if value.starts_with('@') && token != "--data-raw" {
                        return Err(invalid(
                            "File-backed cURL bodies require selecting a file in the editor.",
                        ));
                    }
                    if !matches!(config.body, RequestBody::None) {
                        return Err(invalid(
                            "Combine repeated data options explicitly before importing.",
                        ));
                    }
                    config.body = RequestBody::Text(value.clone());
                    if !explicit_method {
                        config.method = crate::RequestMethod::POST;
                    }
                }
                "-u" | "--user" => {
                    let (username, password) = value.split_once(':').ok_or_else(|| invalid("Basic authentication needs username:password; interactive prompts are unsupported."))?;
                    config.auth = RequestAuth::Basic {
                        username: username.into(),
                        password: password.into(),
                    };
                }
                "--url" => {
                    if !config.url.is_empty() {
                        return Err(invalid("Import one URL at a time."));
                    }
                    config.url = value.clone();
                }
                "--max-time" => {
                    let seconds: f64 = value
                        .parse()
                        .map_err(|_| invalid("Use a numeric --max-time."))?;
                    if !seconds.is_finite() || seconds <= 0.0 || seconds > 3600.0 {
                        return Err(invalid(
                            "--max-time must be greater than zero and at most 3600 seconds.",
                        ));
                    }
                    config.timeout_ms = (seconds * 1000.0).ceil() as u32;
                }
                "--max-redirs" => {
                    redirect_limit = value
                        .parse()
                        .map_err(|_| invalid("Use a nonnegative integer --max-redirs."))?
                }
                _ => unreachable!("all value-taking cURL options are handled"),
            }
        } else {
            match token.as_str() {
                "-L" | "--location" => {
                    follow_redirects = true;
                    warnings.push("Redirects are restricted to the target origin; cURL may follow other origins.".into());
                }
                "-I" | "--head" => {
                    config.method = crate::RequestMethod::HEAD;
                    explicit_method = true;
                }
                "-s" | "--silent" | "-S" | "--show-error" => {}
                "--compressed" => warnings.push("Automatic response decompression is not enabled; review Accept-Encoding and response handling.".into()),
                value
                    if value.starts_with("http://")
                        || value.starts_with("https://")
                        || value.starts_with("{{") =>
                {
                    if !config.url.is_empty() {
                        return Err(invalid("Import one URL at a time."));
                    }
                    config.url = value.into();
                }
                _ => {
                    return Err(invalid(
                        "Unsupported cURL option or shell syntax. Use separate -X, -H, -d, -u, --url, timeout and redirect options; no shell operators or scripts.",
                    ));
                }
            }
        }
        index += 1;
    }
    if config.url.is_empty() {
        return Err(invalid("Provide a target URL."));
    }
    config.max_redirects = if follow_redirects { redirect_limit } else { 0 };
    if matches!(config.body, RequestBody::Text(_))
        && !config
            .headers
            .iter()
            .any(|pair| pair.name.eq_ignore_ascii_case("content-type"))
    {
        config.headers.push(NameValue {
            name: "content-type".into(),
            value: "application/x-www-form-urlencoded".into(),
        });
    }
    Ok(vec![ImportedRequest {
        name: "cURL request".into(),
        config,
        warnings,
    }])
}
