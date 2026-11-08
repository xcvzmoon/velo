//! Bounded noninteractive input; error output never includes input or OS messages.

use std::{collections::BTreeMap, io::Read, path::PathBuf};

use clap::Parser;
use velo_engine::{RunConfig, RunConfirmation, RunManager, TestDefinition};

use crate::{Failure, INVALID, failure};

/// Maximum encoded input size, matching portable definitions and prepared datasets.
const INPUT_BYTES: u64 = 8_388_608;

#[derive(Parser)]
#[command(
    name = "velo-cli",
    version,
    about = "Run portable Velo tests without a desktop session"
)]
pub(crate) struct Arguments {
    /// Version 1 desktop-exported JSON definition.
    pub definition: PathBuf,
    /// Resolve and validate all inputs without sending traffic.
    #[arg(long)]
    pub prepare: bool,
    /// Exact origin (scheme, host, and optional port) authorized to receive this workload.
    #[arg(long)]
    pub authorize: Option<String>,
    /// Select a named definition environment.
    #[arg(long)]
    pub environment: Option<String>,
    /// Bind a secret reference using NAME=ENVIRONMENT_VARIABLE (never a literal value).
    #[arg(long = "secret", value_name = "NAME=ENVIRONMENT_VARIABLE")]
    pub secrets: Vec<String>,
    /// Override a variable using NAME=ENVIRONMENT_VARIABLE (never a literal value).
    #[arg(long = "variable", value_name = "NAME=ENVIRONMENT_VARIABLE")]
    pub variables: Vec<String>,
    /// Read a JSON object of secret names and string values from stdin.
    #[arg(long)]
    pub secrets_stdin: bool,
    /// Normalized string records from the shared dataset preparation helper.
    #[arg(long)]
    pub data_rows: Option<PathBuf>,
    /// Write shared JSON evidence, including the candidate usable as a baseline.
    #[arg(long)]
    pub json: Option<PathBuf>,
    /// Write shared JUnit XML.
    #[arg(long)]
    pub junit: Option<PathBuf>,
    /// Write a standalone readable HTML report.
    #[arg(long)]
    pub html: Option<PathBuf>,
    /// Shared version 1 JSON report whose candidate becomes the baseline.
    #[arg(long, requires = "comparison")]
    pub baseline: Option<PathBuf>,
    /// Shared ComparisonSettings JSON, required with --baseline.
    #[arg(long, requires = "baseline")]
    pub comparison: Option<PathBuf>,
}

impl std::fmt::Debug for Arguments {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Arguments([redacted])")
    }
}

pub(crate) fn read_input(reader: impl Read) -> Result<String, Failure> {
    let mut source = String::new();
    reader
        .take(INPUT_BYTES + 1)
        .read_to_string(&mut source)
        .map_err(|_| failure(INVALID, "INPUT_READ_FAILED"))?;
    if source.len() as u64 > INPUT_BYTES {
        return Err(failure(INVALID, "INPUT_TOO_LARGE"));
    }
    Ok(source)
}

pub(crate) fn read_file(path: &std::path::Path) -> Result<String, Failure> {
    let file = std::fs::File::open(path).map_err(|_| failure(INVALID, "INPUT_READ_FAILED"))?;
    read_input(file)
}

fn environment_bindings(bindings: &[String]) -> Result<BTreeMap<String, String>, Failure> {
    let mut values = BTreeMap::new();
    for binding in bindings {
        let (name, variable) = binding
            .split_once('=')
            .filter(|(name, variable)| {
                !name.is_empty() && !variable.is_empty() && !variable.contains('=')
            })
            .ok_or_else(|| failure(INVALID, "INVALID_ENVIRONMENT_BINDING"))?;
        let value = std::env::var(variable)
            .map_err(|_| failure(INVALID, "ENVIRONMENT_INPUT_UNAVAILABLE"))?;
        if value.is_empty()
            || value.len() > 16_384
            || values.insert(name.to_owned(), value).is_some()
        {
            return Err(failure(INVALID, "INVALID_ENVIRONMENT_BINDING"));
        }
    }
    Ok(values)
}

pub(crate) struct Prepared {
    pub config: RunConfig,
    pub confirmation: RunConfirmation,
    pub sensitive: Vec<String>,
}

pub(crate) fn prepare(
    arguments: &Arguments,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Prepared, Failure> {
    let mut definition =
        TestDefinition::parse(&read_file(&arguments.definition)?).map_err(Failure::input)?;
    let mut secrets = environment_bindings(&arguments.secrets)?;
    if arguments.secrets_stdin {
        let values: BTreeMap<String, String> =
            serde_json::from_str(&read_input(std::io::stdin().lock())?)
                .map_err(|_| failure(INVALID, "INVALID_SECRET_INPUT"))?;
        for (name, value) in values {
            if name.is_empty()
                || value.is_empty()
                || value.len() > 16_384
                || secrets.insert(name, value).is_some()
            {
                return Err(failure(INVALID, "INVALID_SECRET_INPUT"));
            }
        }
    }
    let variables = environment_bindings(&arguments.variables)?;
    let sensitive: Vec<String> = secrets.values().cloned().collect();
    if let Some(environment) = &arguments.environment {
        definition
            .environments
            .get_mut(environment)
            .ok_or_else(|| failure(INVALID, "ENVIRONMENT_UNAVAILABLE"))?
            .extend(variables);
    } else {
        definition.variables.extend(variables);
    }
    let rows: Vec<BTreeMap<String, String>> = match &arguments.data_rows {
        Some(path) => serde_json::from_str(&read_file(path)?)
            .map_err(|_| failure(INVALID, "INVALID_DATASET_ROWS"))?,
        None if definition.dataset.is_some() => {
            return Err(failure(INVALID, "DATASET_INPUT_REQUIRED"));
        }
        None => Vec::new(),
    };
    let config = definition
        .resolve_data(
            arguments.environment.as_deref(),
            &rows,
            || cancelled.load(std::sync::atomic::Ordering::Acquire),
            |name| {
                secrets
                    .get(name)
                    .cloned()
                    .ok_or_else(|| velo_engine::RunError {
                        code: "SECRET_UNAVAILABLE".into(),
                        message: String::new(),
                        field: None,
                    })
            },
        )
        .map_err(Failure::input)?;
    let confirmation = RunManager::prepare(config.clone()).map_err(Failure::input)?;
    if sensitive
        .iter()
        .any(|secret| confirmation.origin.contains(secret))
    {
        return Err(failure(INVALID, "SECRET_TARGET_UNSUPPORTED"));
    }
    Ok(Prepared {
        config,
        confirmation,
        sensitive,
    })
}

pub(crate) fn validate_destinations(arguments: &Arguments) -> Result<(), Failure> {
    let mut inputs = Vec::new();
    for path in [
        Some(&arguments.definition),
        arguments.data_rows.as_ref(),
        arguments.baseline.as_ref(),
        arguments.comparison.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        inputs
            .push(std::fs::canonicalize(path).map_err(|_| failure(INVALID, "INPUT_READ_FAILED"))?);
    }
    let mut destinations = Vec::new();
    for path in [&arguments.json, &arguments.junit, &arguments.html]
        .into_iter()
        .flatten()
    {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let destination = std::fs::canonicalize(parent)
            .map_err(|_| failure(INVALID, "INVALID_REPORT_DESTINATION"))?
            .join(
                path.file_name()
                    .ok_or_else(|| failure(INVALID, "INVALID_REPORT_DESTINATION"))?,
            );
        let resolved = if path.exists() {
            std::fs::canonicalize(path)
                .map_err(|_| failure(INVALID, "INVALID_REPORT_DESTINATION"))?
        } else {
            destination.clone()
        };
        if inputs.contains(&resolved)
            || destinations.contains(&resolved)
            || destinations.contains(&destination)
        {
            return Err(failure(INVALID, "REPORT_PATH_COLLISION"));
        }
        destinations.push(resolved);
    }
    Ok(())
}
