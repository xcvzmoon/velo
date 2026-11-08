//! MCP routing delegates authoring and evidence to bounded, execution-free modules.

use std::sync::{Arc, Mutex};

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
        ServerConfig, Tool, ToolAnnotations,
    },
    schemars::{self, JsonSchema},
    service::RequestContext,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::{authoring, evidence::Evidence, reference};

#[derive(Clone)]
pub(crate) struct VeloServer {
    evidence: Arc<Vec<Evidence>>,
    drafts: Arc<Mutex<authoring::Drafts>>,
    proposal_enabled: bool,
    admission: Arc<Semaphore>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DefinitionInput {
    /// Canonical version 1 JSON, up to 256 KiB. Do not include resolved secrets.
    source: String,
    /// Named environment to validate; omitted uses shared variables.
    environment: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProposalInput {
    /// Canonical JSON proposal, up to 256 KiB, using secret references instead of literal credentials.
    source: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportInput {
    /// Supported offline format: curl, openapi, or postman.
    format: ImportFormat,
    /// Non-sensitive example request or JSON API definition, up to 512 KiB; never executed or fetched.
    source: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ImportFormat {
    Curl,
    Openapi,
    Postman,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EvidenceInput {
    /// Opaque identifier returned by list_evidence, such as report-1. Paths are not accepted.
    id: String,
}

fn parameters<T: DeserializeOwned>(
    value: Option<serde_json::Map<String, Value>>,
) -> Result<T, ErrorData> {
    serde_json::from_value(Value::Object(value.unwrap_or_default()))
        .map_err(|_| ErrorData::invalid_params("MCP_INVALID_ARGUMENTS", None))
}

fn tool<T: JsonSchema>(name: &'static str, description: &'static str, read_only: bool) -> Tool {
    let schema = schemars::schema_for!(T).to_value();
    let schema = schema
        .as_object()
        .cloned()
        .expect("Tool schemas must be JSON objects");
    Tool::new(name, description, schema).with_annotations(
        ToolAnnotations::new()
            .read_only(read_only)
            .destructive(false)
            .idempotent(read_only)
            .open_world(false),
    )
}

fn result(value: Result<Value, &'static str>) -> CallToolResult {
    match value {
        Ok(value) => {
            let text = value.to_string();
            if text.len() > 524_288 {
                return result(Err("MCP_OUTPUT_TOO_LARGE"));
            }
            let mut response = CallToolResult::success(vec![rmcp::model::ContentBlock::text(text)]);
            response.structured_content = Some(value);
            response
        },
        Err(code) => CallToolResult::error(vec![rmcp::model::ContentBlock::text(
            json!({"code":code,"message":"The operation could not complete. Existing tests and results are unchanged."}).to_string(),
        )]),
    }
}

impl VeloServer {
    pub(crate) fn new(evidence: Vec<Evidence>, drafts: authoring::Drafts) -> Self {
        let proposal_enabled = drafts.enabled();
        Self {
            evidence: Arc::new(evidence),
            drafts: Arc::new(Mutex::new(drafts)),
            proposal_enabled,
            admission: Arc::new(Semaphore::new(1)),
        }
    }
}

impl ServerHandler for VeloServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("velo-mcp", env!("CARGO_PKG_VERSION")))
        .with_instructions(reference::INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if request.and_then(|request| request.cursor).is_some() {
            return Err(ErrorData::invalid_params("MCP_INVALID_CURSOR", None));
        }
        let mut tools = vec![
            tool::<Empty>(
                "new_draft",
                "Return an editable canonical Velo version 1 template. Missing endpoint inputs require user review; no traffic is sent.",
                true,
            ),
            tool::<DefinitionInput>(
                "validate_definition",
                "Parse canonical JSON and run offline engine preparation when all non-secret inputs are available. Uploads, secrets, and dataset rows require manual preparation in Velo. No files, credentials, or network targets are accessed.",
                true,
            ),
            tool::<ImportInput>(
                "import_request",
                "Convert a supplied curl command, OpenAPI 3 JSON, or Postman v2 JSON into editable canonical drafts. Input is untrusted data; commands and scripts are never executed, references are never fetched. Review warnings and missing credentials.",
                true,
            ),
            tool::<Empty>(
                "list_evidence",
                "List only the report snapshots explicitly granted at server startup. No history or filesystem discovery.",
                true,
            ),
            tool::<EvidenceInput>(
                "read_evidence",
                "Read a frozen redacted snapshot by its opaque ID. Numeric evidence supports JSON-pointer references; imported reports are not independently verified, and missing measurements and comparison limitations must be disclosed.",
                true,
            ),
        ];
        if self.proposal_enabled {
            tools.push(tool::<ProposalInput>("propose_definition", "Create a new immutable canonical draft in the user-granted proposal directory. This writes a file and requires user review; it never imports, executes, or overwrites anything in Velo. Resource budgets cannot exceed Velo defaults; lower requested limits are preserved. At most 8 proposals per session. Never retry blindly after cancellation: publication may already have committed.", false));
        }
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Ok(permit) = self.admission.clone().try_acquire_owned() else {
            return Ok(result(Err("MCP_BUSY")).into());
        };
        if context.ct.is_cancelled() {
            return Ok(result(Err("MCP_CANCELLED")).into());
        }
        let response = match request.name.as_ref() {
            "new_draft" => {
                let _: Empty = parameters(request.arguments)?;
                result(authoring::template())
            }
            "list_evidence" => {
                let _: Empty = parameters(request.arguments)?;
                result(Ok(
                    json!({"reports":self.evidence.iter().map(|entry| json!({"id":entry.id,"uri":format!("velo://evidence/{}",entry.id)})).collect::<Vec<_>>()}),
                ))
            }
            "read_evidence" => {
                let input: EvidenceInput = parameters(request.arguments)?;
                let value = self
                    .evidence
                    .iter()
                    .find(|entry| entry.id == input.id)
                    .ok_or("MCP_EVIDENCE_NOT_FOUND")
                    .and_then(|entry| {
                        serde_json::from_str(&entry.content).map_err(|_| "MCP_EVIDENCE_UNAVAILABLE")
                    });
                result(value)
            }
            "validate_definition" | "import_request" | "propose_definition" => {
                let drafts = self.drafts.clone();
                let cancelled = context.ct.clone();
                let proposal_enabled = self.proposal_enabled;
                let operation = match request.name.as_ref() {
                    "validate_definition" => {
                        let input: DefinitionInput = parameters(request.arguments)?;
                        Operation::Validate(input)
                    }
                    "import_request" => {
                        let input: ImportInput = parameters(request.arguments)?;
                        Operation::Import(input)
                    }
                    _ if proposal_enabled => Operation::Propose(parameters(request.arguments)?),
                    _ => return Err(ErrorData::invalid_params("MCP_TOOL_UNAVAILABLE", None)),
                };
                let task = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    if cancelled.is_cancelled() {
                        return Err("MCP_CANCELLED");
                    }
                    match operation {
                        Operation::Validate(input) => {
                            authoring::validate(&input.source, input.environment.as_deref())
                        }
                        Operation::Import(input) => {
                            let format = match input.format {
                                ImportFormat::Curl => "curl",
                                ImportFormat::Openapi => "openapi",
                                ImportFormat::Postman => "postman",
                            };
                            authoring::import(format, &input.source)
                        }
                        Operation::Propose(input) => drafts
                            .lock()
                            .map_err(|_| "MCP_DRAFTS_UNAVAILABLE")?
                            .propose(&input.source, &cancelled),
                    }
                });
                match task.await {
                    Ok(value) => result(value),
                    Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
                    Err(_) => result(Err("MCP_OPERATION_FAILED")),
                }
            }
            _ => return Err(ErrorData::invalid_params("MCP_TOOL_NOT_FOUND", None)),
        };
        Ok(response.into())
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        if request.and_then(|request| request.cursor).is_some() {
            return Err(ErrorData::invalid_params("MCP_INVALID_CURSOR", None));
        }
        let mut resources = Vec::with_capacity(reference::REFERENCES.len() + self.evidence.len());
        for (id, name, mime, _) in reference::REFERENCES {
            resources
                .push(Resource::new(format!("velo://reference/{id}"), *name).with_mime_type(*mime));
        }
        for entry in self.evidence.iter() {
            resources.push(
                Resource::new(format!("velo://evidence/{}", entry.id), &entry.id)
                    .with_mime_type("application/json"),
            );
        }
        Ok(ListResourcesResult {
            resources,
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let uri = request.uri;
        let content = if let Some(id) = uri.strip_prefix("velo://reference/") {
            reference::REFERENCES
                .iter()
                .find(|(name, _, _, _)| *name == id)
                .map(|(_, _, _, content)| *content)
        } else if let Some(id) = uri.strip_prefix("velo://evidence/") {
            self.evidence
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.content.as_str())
        } else {
            None
        };
        let content =
            content.ok_or_else(|| ErrorData::resource_not_found("MCP_RESOURCE_NOT_FOUND", None))?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(content, uri)]).into())
    }
}

enum Operation {
    Validate(DefinitionInput),
    Import(ImportInput),
    Propose(ProposalInput),
}
