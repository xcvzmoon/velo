//! Bounded prepared requests and record allocation, independent of JavaScript and transport I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{NameValue, RequestAuth, RequestBody, RunError};

/// Value reuse boundary, independent of record allocation.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::DataLifetime::Run)?, "run");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataLifetime {
    /// One record shared by the entire run.
    Run,
    /// One record per worker, retained until that worker retires.
    User,
    /// A fresh allocation before each logical request.
    Iteration,
}

/// Global record selection order; concurrent completion order is unspecified.
///
/// # Examples
/// ```
/// let _allocation = velo_engine::DataAllocation::Unique;
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataAllocation {
    /// Traverse input order; repeat only when explicitly configured.
    Sequential,
    /// Seeded shuffle, visiting each index once per cycle.
    Random,
    /// Traverse input order without ever recycling an index.
    Unique,
}

/// Action when all record indexes have been allocated.
///
/// # Examples
/// ```
/// let _policy = velo_engine::DataExhaustion::Fail;
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataExhaustion {
    /// Close admission, drain requests, and finish successfully.
    Stop,
    /// Begin another cycle; incompatible with unique allocation.
    Repeat,
    /// Close admission, drain requests, and finish with DATASET_EXHAUSTED.
    Fail,
}

/// Portable preparation settings. Rows and local filenames are never embedded here.
///
/// # Examples
/// ```
/// # fn example(data: velo_engine::DataDefinition) {
/// assert!(data.seed <= u32::MAX);
/// # }
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DataDefinition {
    /// Reuse boundary for selected records.
    pub lifetime: DataLifetime,
    /// Global selection order.
    pub allocation: DataAllocation,
    /// Explicit end-of-data behavior.
    pub exhaustion: DataExhaustion,
    /// Seed for generation and the native allocation shuffle.
    pub seed: u32,
    /// Generator or import recipe. Imported files must be selected on this machine.
    pub source: DataSource,
}

impl std::fmt::Debug for DataDefinition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DataDefinition([redacted])")
    }
}

/// Supported preparation sources; arbitrary JavaScript is never evaluated.
///
/// # Examples
/// ```
/// let source = velo_engine::DataSource::Json { mapping: Default::default() };
/// assert!(matches!(source, velo_engine::DataSource::Json { .. }));
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataSource {
    /// Pinned Faker generation with ordered, structured field definitions.
    Faker {
        /// Exact supported package version, currently 10.6.0.
        version: String,
        /// Supported locale: en, de, or fr, with English fallback.
        locale: String,
        /// Number of prepared records, 1 through 10,000.
        count: u32,
        /// Ordered fields; related fields may refer to earlier fields only.
        #[ts(type = "DataField[]")]
        fields: Vec<DataField>,
    },
    /// CSV with a header row; mapping keys are variable names and values are source columns.
    Csv {
        /// Required source columns, mapped to request variables.
        #[ts(type = "Record<string, string>")]
        mapping: BTreeMap<String, String>,
    },
    /// JSON array of scalar-valued records, with explicit field mapping.
    Json {
        /// Required source fields, mapped to request variables.
        #[ts(type = "Record<string, string>")]
        mapping: BTreeMap<String, String>,
    },
}

impl std::fmt::Debug for DataSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DataSource([redacted])")
    }
}

/// One named generated field, evaluated in definition order.
///
/// # Examples
/// ```
/// let field = velo_engine::DataField { name: "id".into(), generator: velo_engine::DataGenerator::Uuid };
/// assert_eq!(field.name, "id");
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DataField {
    /// Variable name, using letters, digits, underscores, dots, or hyphens.
    pub name: String,
    /// Allowlisted generator and its supported arguments.
    pub generator: DataGenerator,
}

impl std::fmt::Debug for DataField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DataField([redacted])")
    }
}

/// Allowlisted generators, including explicit invalid and boundary values through literals.
///
/// # Examples
/// ```
/// let generator = velo_engine::DataGenerator::Integer { min: 0, max: 10 };
/// assert!(matches!(generator, velo_engine::DataGenerator::Integer { .. }));
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataGenerator {
    /// Faker person.firstName without arguments.
    FirstName,
    /// Faker person.lastName without arguments.
    LastName,
    /// Faker string.uuid without arguments; uniqueness of values is not guaranteed.
    Uuid,
    /// Faker internet.email using earlier first/last-name fields and an explicit provider.
    Email {
        /// Earlier field containing the first name.
        first: String,
        /// Earlier field containing the last name.
        last: String,
        /// Email provider domain.
        provider: String,
    },
    /// Inclusive integer range, bounded to signed 32-bit integers.
    Integer {
        /// Inclusive minimum.
        min: i32,
        /// Inclusive maximum, at least min.
        max: i32,
    },
    /// Deliberate string value, including empty or invalid application data.
    Literal {
        /// Literal string, at most 16 KiB.
        value: String,
    },
    /// Copy an earlier field without another random draw.
    Copy {
        /// Earlier field name.
        field: String,
    },
}

impl std::fmt::Debug for DataGenerator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DataGenerator([redacted])")
    }
}

/// Resolved request values for one record, used only in transient preparation.
///
/// # Examples
/// ```
/// # fn example(record: velo_engine::DataRequest) {
/// assert!(!format!("{record:?}").contains(&record.url));
/// # }
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DataRequest {
    /// Resolved HTTP URL.
    pub url: String,
    /// Resolved ordered headers.
    #[ts(type = "NameValue[]")]
    pub headers: Vec<NameValue>,
    /// Resolved ordered query pairs.
    #[ts(type = "NameValue[]")]
    pub query: Vec<NameValue>,
    /// Resolved body with JSON escaping preserved.
    pub body: RequestBody,
    /// Resolved authentication.
    pub auth: RequestAuth,
}

impl std::fmt::Debug for DataRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DataRequest([redacted])")
    }
}

/// Transient, bounded resolved requests. Never save this object in test definitions or history.
///
/// # Examples
/// ```
/// # fn example(data: velo_engine::PreparedData) {
/// assert!(!format!("{data:?}").contains("requests:"));
/// # }
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PreparedData {
    /// Recipe that produced these requests.
    pub definition: DataDefinition,
    /// Resolved request records, limited to 10,000 and the configured byte budget.
    #[ts(type = "DataRequest[]")]
    pub requests: Vec<DataRequest>,
}

impl std::fmt::Debug for PreparedData {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PreparedData([redacted])")
    }
}

/// Safe preparation and allocation metadata retained in history.
///
/// No field names, literal values, mapping names, or dataset rows are included.
///
/// # Examples
/// ```
/// # fn example(summary: velo_engine::DataSummary) {
/// assert!(summary.records <= 10_000);
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct DataSummary {
    /// Source kind: faker, csv, or json.
    pub source: String,
    /// Faker package version, absent for imports.
    pub version: Option<String>,
    /// Faker locale, absent for imports.
    pub locale: Option<String>,
    /// Generation and shuffle seed.
    pub seed: u32,
    /// Prepared record count.
    pub records: u32,
    /// Reuse boundary.
    pub lifetime: DataLifetime,
    /// Selection order.
    pub allocation: DataAllocation,
    /// End-of-data behavior.
    pub exhaustion: DataExhaustion,
    /// Allocations made, excluding reuse within a lifetime.
    pub allocated: String,
    /// Repeat cycles entered after the initial pass.
    pub cycles: String,
    /// True once a stop or fail policy closed admission.
    pub exhausted: bool,
}

pub(crate) fn invalid(message: &str) -> RunError {
    RunError::new("INVALID_DATASET", message, Some("dataset"))
}

impl DataDefinition {
    pub(crate) fn validate(&self) -> Result<(), RunError> {
        if self.allocation == DataAllocation::Unique && self.exhaustion == DataExhaustion::Repeat {
            return Err(invalid("Unique allocation cannot repeat records."));
        }
        match &self.source {
            DataSource::Faker {
                version,
                locale,
                count,
                fields,
            } => {
                if version != "10.6.0" || !["en", "de", "fr"].contains(&locale.as_str()) {
                    return Err(invalid("Use Faker 10.6.0 and locale en, de, or fr."));
                }
                if !(1..=10_000).contains(count) || fields.is_empty() || fields.len() > 64 {
                    return Err(invalid("Use 1 to 10,000 records and 1 to 64 fields."));
                }
                let mut names = std::collections::BTreeSet::new();
                for field in fields {
                    if !valid_name(&field.name) || names.contains(&field.name) {
                        return Err(invalid("Field names must be valid and unique."));
                    }
                    match &field.generator {
                        DataGenerator::Email {
                            first,
                            last,
                            provider,
                        } => {
                            if !names.contains(first)
                                || !names.contains(last)
                                || provider.is_empty()
                                || provider.len() > 253
                                || !provider.bytes().all(|byte| {
                                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
                                })
                            {
                                return Err(invalid(
                                    "Email requires earlier first/last fields and a provider domain.",
                                ));
                            }
                        }
                        DataGenerator::Copy { field } if !names.contains(field) => {
                            return Err(invalid("Copy must reference an earlier field."));
                        }
                        DataGenerator::Integer { min, max } if min > max => {
                            return Err(invalid("Integer minimum must not exceed maximum."));
                        }
                        DataGenerator::Literal { value } if value.len() > 16_384 => {
                            return Err(invalid("Literal values must not exceed 16 KiB."));
                        }
                        _ => {}
                    }
                    names.insert(&field.name);
                }
            }
            DataSource::Csv { mapping } | DataSource::Json { mapping } => {
                if mapping.is_empty()
                    || mapping.len() > 64
                    || mapping.iter().any(|(name, column)| {
                        !valid_name(name) || column.is_empty() || column.len() > 256
                    })
                {
                    return Err(invalid(
                        "Map 1 to 64 source columns to valid variable names.",
                    ));
                }
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| invalid("Dataset settings could not be encoded."))?
            .len()
            > 65_536
        {
            return Err(invalid("Dataset settings exceed 64 KiB."));
        }
        Ok(())
    }

    pub(crate) fn summary(&self, records: usize) -> DataSummary {
        let (source, version, locale) = match &self.source {
            DataSource::Faker {
                version, locale, ..
            } => ("faker", Some(version.clone()), Some(locale.clone())),
            DataSource::Csv { .. } => ("csv", None, None),
            DataSource::Json { .. } => ("json", None, None),
        };
        DataSummary {
            source: source.into(),
            version,
            locale,
            seed: self.seed,
            records: records as u32,
            lifetime: self.lifetime,
            allocation: self.allocation,
            exhaustion: self.exhaustion,
            allocated: "0".into(),
            cycles: "0".into(),
            exhausted: false,
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

/// Owns one global cursor. Call only while holding the run admission lock.
/// Random order is Fisher-Yates with SplitMix64 constants, fixed for format version 1.
/// Repeat cycles replay the same permutation; network completion never changes the cursor.
pub(crate) struct Allocator {
    order: Box<[usize]>,
    next: u64,
    policy: DataExhaustion,
}

impl Allocator {
    pub(crate) fn new(
        allocation: DataAllocation,
        policy: DataExhaustion,
        seed: u32,
        count: usize,
    ) -> Self {
        let mut order: Vec<_> = (0..count).collect();
        if allocation == DataAllocation::Random {
            let mut state = u64::from(seed);
            for index in (1..count).rev() {
                state = state.wrapping_add(0x9e3779b97f4a7c15);
                let mut random = state;
                random = (random ^ (random >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                random = (random ^ (random >> 27)).wrapping_mul(0x94d049bb133111eb);
                random ^= random >> 31;
                order.swap(index, (random % (index as u64 + 1)) as usize);
            }
        }
        Self {
            order: order.into_boxed_slice(),
            next: 0,
            policy,
        }
    }

    pub(crate) fn take(&mut self, summary: &mut DataSummary) -> Option<usize> {
        let count = self.order.len() as u64;
        if self.next >= count && self.policy != DataExhaustion::Repeat {
            summary.exhausted = true;
            return None;
        }
        let index = self.order[(self.next % count) as usize];
        self.next += 1;
        summary.allocated = self.next.to_string();
        summary.cycles = ((self.next - 1) / count).to_string();
        Some(index)
    }
}
