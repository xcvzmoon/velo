//! Headless host adapter over shared Velo preparation, execution, and reporting.

mod evidence;
mod input;

use std::{io::Write, process::ExitCode, time::Duration};

use clap::Parser;
use velo_contracts::reports::{self, RegressionVerdict};
use velo_engine::{RunError, RunManager, RunSnapshot, RunState};

use input::Arguments;

const SUCCESS: u8 = 0;
const CRITERIA: u8 = 1;
const INVALID: u8 = 2;
const EXECUTION: u8 = 3;
const INTERRUPTED: u8 = 130;

#[derive(Debug)]
struct Failure {
    status: u8,
    code: String,
}

fn failure(status: u8, code: &str) -> Failure {
    Failure {
        status,
        code: code.to_owned(),
    }
}

impl Failure {
    fn input(error: RunError) -> Self {
        failure(INVALID, &error.code)
    }
}

struct Signals {
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(windows)]
    interrupt: tokio::signal::windows::CtrlC,
    #[cfg(windows)]
    terminate: tokio::signal::windows::CtrlBreak,
}

impl Signals {
    fn install() -> Result<Self, Failure> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            Ok(Self {
                interrupt: signal(SignalKind::interrupt())
                    .map_err(|_| failure(EXECUTION, "SIGNAL_SETUP_FAILED"))?,
                terminate: signal(SignalKind::terminate())
                    .map_err(|_| failure(EXECUTION, "SIGNAL_SETUP_FAILED"))?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                interrupt: tokio::signal::windows::ctrl_c()
                    .map_err(|_| failure(EXECUTION, "SIGNAL_SETUP_FAILED"))?,
                terminate: tokio::signal::windows::ctrl_break()
                    .map_err(|_| failure(EXECUTION, "SIGNAL_SETUP_FAILED"))?,
            })
        }
    }

    async fn received(&mut self) {
        tokio::select! {
            _ = self.interrupt.recv() => {},
            _ = self.terminate.recv() => {},
        }
    }
}

fn terminal(state: RunState) -> bool {
    matches!(
        state,
        RunState::Completed | RunState::Cancelled | RunState::Failed | RunState::Aborted
    )
}

async fn wait(manager: &RunManager, signals: &mut Signals) -> Result<(RunSnapshot, bool), Failure> {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = signals.received() => {
                manager.shutdown().await;
                return manager.current().await.map(|snapshot| (snapshot, true))
                    .ok_or_else(|| failure(EXECUTION, "RUN_UNAVAILABLE"));
            },
            _ = interval.tick() => {
                let snapshot = manager.current().await.ok_or_else(|| failure(EXECUTION, "RUN_UNAVAILABLE"))?;
                if terminal(snapshot.state) { return Ok((snapshot, false)); }
            }
        }
    }
}

async fn run(arguments: Arguments) -> Result<u8, Failure> {
    let mut signals = Signals::install()?;
    if arguments.secrets_stdin {
        writeln!(std::io::stderr().lock(), "Reading secret input from stdin.")
            .map_err(|_| failure(EXECUTION, "TERMINAL_WRITE_FAILED"))?;
    }
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let preparation_cancelled = cancelled.clone();
    let preparation = tokio::task::spawn_blocking(move || {
        input::validate_destinations(&arguments)?;
        let prepared = input::prepare(&arguments, &preparation_cancelled)?;
        let comparison = evidence::comparison(&arguments)?;
        Ok::<_, Failure>((arguments, prepared, comparison))
    });
    let (arguments, prepared, (baseline, settings)) = tokio::select! {
        biased;
        _ = signals.received() => {
            cancelled.store(true, std::sync::atomic::Ordering::Release);
            return Ok(INTERRUPTED);
        },
        result = preparation => match result {
            Ok(result) => result?,
            Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
            Err(_) => return Err(failure(EXECUTION, "PREPARATION_FAILED")),
        },
    };
    if arguments.prepare {
        writeln!(
            std::io::stdout().lock(),
            "Prepared successfully; no traffic sent."
        )
        .map_err(|_| failure(EXECUTION, "TERMINAL_WRITE_FAILED"))?;
        return Ok(SUCCESS);
    }
    let confirmation = &prepared.confirmation;
    if arguments.authorize.as_deref() != Some(confirmation.origin.as_str()) {
        return Err(failure(INVALID, "TARGET_AUTHORIZATION_REQUIRED"));
    }
    let method = format!("{:?}", prepared.config.method);
    let configuration = velo_engine::report_configuration(&prepared.config);
    let manager = RunManager::default();
    tokio::select! {
        biased;
        _ = signals.received() => {
            manager.shutdown().await;
            return Ok(INTERRUPTED);
        },
        started = manager.start(prepared.config, true) => {
            started.map_err(|error| failure(EXECUTION, &error.code))?;
        }
    }
    let result = wait(&manager, &mut signals).await;
    manager.shutdown().await;
    let (snapshot, interrupted) = result?;
    let candidate = evidence::entry(method, configuration, &snapshot)?;
    let verdict = reports::run_verdict(&candidate);
    let report = reports::compare(baseline, candidate, &settings);
    let status = if interrupted || snapshot.state == RunState::Cancelled {
        INTERRUPTED
    } else if snapshot.state == RunState::Failed
        || (snapshot.evaluated.counters.responses == 0
            && (snapshot.evaluated.counters.transport_failures > 0
                || snapshot.evaluated.counters.timeouts > 0))
    {
        EXECUTION
    } else if verdict != RegressionVerdict::Pass
        || (arguments.baseline.is_some() && report.verdict != RegressionVerdict::Pass)
    {
        CRITERIA
    } else {
        SUCCESS
    };
    let report = evidence::redact(report, &prepared.sensitive)?;
    evidence::export(&arguments, &report)?;
    writeln!(
        std::io::stdout().lock(),
        "Run finished; evaluated responses {}; criteria {verdict:?}; exit status {status}.",
        snapshot.evaluated.counters.responses
    )
    .map_err(|_| failure(EXECUTION, "TERMINAL_WRITE_FAILED"))?;
    Ok(status)
}

fn main() -> ExitCode {
    let arguments = match Arguments::try_parse() {
        Ok(arguments) => arguments,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                return if error.print().is_ok() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(EXECUTION)
                };
            }
            let _ = writeln!(
                std::io::stderr().lock(),
                "INVALID_ARGUMENTS: use velo-cli --help."
            );
            return ExitCode::from(INVALID);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "RUNTIME_INIT_FAILED: Could not start the execution runtime."
            );
            return ExitCode::from(EXECUTION);
        }
    };
    let result = runtime.block_on(run(arguments));
    runtime.shutdown_timeout(Duration::from_secs(1));
    match result {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "{}: operation failed; check the inputs and destination permissions.",
                error.code
            );
            ExitCode::from(error.status)
        }
    }
}
