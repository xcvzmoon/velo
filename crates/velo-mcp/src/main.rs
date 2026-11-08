//! Local stdio MCP adapter with explicit file grants and review-only proposals.

mod authoring;
mod evidence;
mod reference;
mod server;

use std::{path::PathBuf, process::ExitCode, time::Duration};

use clap::Parser;
use rmcp::{ServiceExt, transport::async_rw::AsyncRwTransport};
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(
    name = "velo-mcp",
    version,
    about = "Author Velo drafts and inspect explicitly selected, redacted evidence over MCP stdio"
)]
struct Arguments {
    /// Share a frozen redacted snapshot of a version 1 JSON comparison report (repeatable, at most 16).
    #[arg(long, value_name = "REPORT.json")]
    evidence: Vec<PathBuf>,
    /// Enable creation of new review-only proposals in this existing directory (at most 8 per session).
    #[arg(long, value_name = "DIRECTORY")]
    draft_directory: Option<PathBuf>,
}

async fn interrupted() -> Result<(), &'static str> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).map_err(|_| "MCP_SIGNAL_FAILED")?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(|_| "MCP_SIGNAL_FAILED"),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|_| "MCP_SIGNAL_FAILED")
    }
}

async fn serve(server: server::VeloServer) -> Result<(), &'static str> {
    let cancelled = CancellationToken::new();
    let (reader, writer) = rmcp::transport::io::stdio_pipes();
    let transport = AsyncRwTransport::new(reader, writer).with_max_line_length(1_048_576);
    let signals = interrupted();
    tokio::pin!(signals);
    let initialize = tokio::time::timeout(
        Duration::from_secs(30),
        server.serve_with_ct(transport, cancelled.clone()),
    );
    let service = tokio::select! {
        result = initialize => result.map_err(|_| "MCP_INITIALIZATION_TIMEOUT")?
            .map_err(|_| "MCP_INITIALIZATION_FAILED")?,
        result = &mut signals => {
            cancelled.cancel();
            return result;
        },
    };
    let waiting = service.waiting();
    tokio::pin!(waiting);
    tokio::select! {
        result = &mut waiting => result.map(|_| ()).map_err(|_| "MCP_SERVICE_FAILED"),
        result = &mut signals => {
            cancelled.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut waiting).await;
            result
        },
    }
}

fn run(arguments: Arguments) -> Result<(), &'static str> {
    let reports = evidence::load(&arguments.evidence)?;
    let drafts = authoring::Drafts::new(arguments.draft_directory)?;
    let server = server::VeloServer::new(reports, drafts);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "MCP_RUNTIME_FAILED")?;
    let result = runtime.block_on(serve(server));
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}

fn main() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("MCP_PROGRAMMING_PANIC: The server stopped after an internal error.");
        std::process::abort();
    }));
    let arguments = match Arguments::try_parse() {
        Ok(arguments) => arguments,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            return if error.print().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Err(_) => {
            eprintln!("MCP_INVALID_ARGUMENTS: Use velo-mcp --help for supported startup options.");
            return ExitCode::from(2);
        }
    };
    match run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("{code}: The MCP session could not complete.");
            ExitCode::FAILURE
        }
    }
}
