//! Bounded host sampling outside the HTTP executor and metrics lock.

use std::{sync::mpsc, time::Duration};

use serde::{Serialize, Serializer};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::watch;
use ts_rs::TS;

/// One second permits CPU deltas without excessive native refresh overhead.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// Availability of the native sampler, independent of HTTP execution outcomes.
///
/// Individual fields may still be unavailable in an available sample.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::TelemetryStatus::Unsupported)?, "unsupported");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryStatus {
    /// No complete CPU delta window has been observed yet.
    #[default]
    WarmingUp,
    /// At least one native measurement is available; inspect nullable fields.
    Available,
    /// Native initialization, refresh, or sampler execution failed.
    Unavailable,
    /// The operating system has no supported native backend.
    Unsupported,
}

/// Latest native resource sample, with its own monotonic window on the workload timeline.
///
/// CPU is normalized to total logical CPU capacity (0–100%). Memory uses bytes.
/// Process scope includes the native generator, excluding WebView subprocesses.
/// Missing measurements are null, never inferred to be zero or healthy.
///
/// # Examples
/// ```
/// let sample = velo_engine::HostTelemetry::default();
/// assert!(sample.process_cpu_percent.is_none());
/// assert!(sample.sampled_at_us.is_none());
/// ```
#[derive(Clone, Debug, Default, Serialize, TS)]
pub struct HostTelemetry {
    /// Sampler availability; a short run may finish while still warming up.
    pub status: TelemetryStatus,
    /// Workload-relative end of the native sample, as decimal microseconds.
    #[serde(serialize_with = "optional_decimal")]
    #[ts(type = "string | null")]
    pub sampled_at_us: Option<u64>,
    /// Actual native refresh window, as decimal microseconds, excluding initialization.
    #[serde(serialize_with = "optional_decimal")]
    #[ts(type = "string | null")]
    pub interval_us: Option<u64>,
    /// Logical CPUs used to normalize process CPU; null when unavailable.
    pub logical_cpus: Option<u32>,
    /// Native process CPU divided by logical CPU count, in percent.
    pub process_cpu_percent: Option<f64>,
    /// Native process resident memory in decimal bytes.
    #[serde(serialize_with = "optional_decimal")]
    #[ts(type = "string | null")]
    pub process_memory_bytes: Option<u64>,
    /// Host-wide CPU usage across logical CPUs, in percent.
    pub system_cpu_percent: Option<f64>,
    /// Host RAM available for allocation, in decimal bytes; OS accounting varies.
    #[serde(serialize_with = "optional_decimal")]
    #[ts(type = "string | null")]
    pub system_available_memory_bytes: Option<u64>,
    /// Total host physical RAM, in decimal bytes.
    #[serde(serialize_with = "optional_decimal")]
    #[ts(type = "string | null")]
    pub system_total_memory_bytes: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct Sampler {
    stop: Option<mpsc::SyncSender<()>>,
    task: Option<std::thread::JoinHandle<()>>,
    latest: watch::Sender<HostTelemetry>,
}

impl Sampler {
    pub(crate) fn start(latest: watch::Sender<HostTelemetry>, started: std::time::Instant) -> Self {
        let (stop, stopped) = mpsc::sync_channel(0);
        let output = latest.clone();
        let task = std::thread::Builder::new()
            .name("velo-telemetry".into())
            .spawn(move || sample_loop(output, started, stopped));
        let task = match task {
            Ok(task) => Some(task),
            Err(_) => {
                latest.send_replace(unavailable(TelemetryStatus::Unavailable));
                tracing::warn!(
                    code = "TELEMETRY_UNAVAILABLE",
                    "Native sampler could not start"
                );
                None
            }
        };
        Self {
            stop: Some(stop),
            task,
            latest,
        }
    }

    pub(crate) async fn finish(mut self) {
        self.stop.take();
        if let Some(task) = self.task.take() {
            match tokio::task::spawn_blocking(move || task.join()).await {
                Ok(Ok(())) => {}
                Ok(Err(panic)) => std::panic::resume_unwind(panic),
                Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
                Err(_) => {
                    self.latest
                        .send_replace(unavailable(TelemetryStatus::Unavailable));
                    tracing::warn!(
                        code = "TELEMETRY_UNAVAILABLE",
                        "Native sampler did not finish normally"
                    );
                }
            }
        }
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.take();
    }
}

fn unavailable(status: TelemetryStatus) -> HostTelemetry {
    HostTelemetry {
        status,
        ..HostTelemetry::default()
    }
}

fn sample_loop(
    latest: watch::Sender<HostTelemetry>,
    started: std::time::Instant,
    stopped: mpsc::Receiver<()>,
) {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        latest.send_replace(unavailable(TelemetryStatus::Unsupported));
        return;
    }
    let Ok(pid) = sysinfo::get_current_pid() else {
        latest.send_replace(unavailable(TelemetryStatus::Unavailable));
        return;
    };
    let mut system = System::new();
    let refresh = ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .without_tasks();
    system.refresh_cpu_usage();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, refresh);
    sample_with(latest, started, stopped, || collect(&mut system, pid));
}

fn sample_with(
    latest: watch::Sender<HostTelemetry>,
    started: std::time::Instant,
    stopped: mpsc::Receiver<()>,
    mut collect: impl FnMut() -> HostTelemetry,
) {
    let mut previous = std::time::Instant::now();
    while matches!(
        stopped.recv_timeout(SAMPLE_INTERVAL),
        Err(mpsc::RecvTimeoutError::Timeout)
    ) {
        let mut sample = collect();
        if !matches!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            break;
        }
        let now = std::time::Instant::now();
        sample.sampled_at_us = Some(now.saturating_duration_since(started).as_micros() as u64);
        sample.interval_us = Some(now.duration_since(previous).as_micros() as u64);
        latest.send_replace(sample);
        previous = now;
    }
}

fn collect(system: &mut System, pid: sysinfo::Pid) -> HostTelemetry {
    system.refresh_cpu_usage();
    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let refresh = ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .without_tasks();
    let refreshed =
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, refresh);
    let cpus = u32::try_from(system.cpus().len())
        .ok()
        .filter(|count| *count > 0);
    let process = (refreshed > 0).then(|| system.process(pid)).flatten();
    let process_cpu_percent = process.and_then(|process| {
        cpus.and_then(|count| percentage(f64::from(process.cpu_usage()) / f64::from(count)))
    });
    let process_memory_bytes = process
        .map(|process| process.memory())
        .filter(|bytes| *bytes > 0);
    let system_cpu_percent = cpus.and_then(|_| percentage(f64::from(system.global_cpu_usage())));
    let total = system.total_memory();
    let available = system.available_memory();
    let memory_available = total > 0 && available <= total;
    let status = if process_cpu_percent.is_some()
        || process_memory_bytes.is_some()
        || system_cpu_percent.is_some()
        || memory_available
    {
        TelemetryStatus::Available
    } else {
        TelemetryStatus::Unavailable
    };
    HostTelemetry {
        status,
        logical_cpus: cpus,
        process_cpu_percent,
        process_memory_bytes,
        system_cpu_percent,
        system_available_memory_bytes: memory_available.then_some(available),
        system_total_memory_bytes: memory_available.then_some(total),
        ..HostTelemetry::default()
    }
}

fn optional_decimal<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&value.to_string()),
        None => serializer.serialize_none(),
    }
}

fn percentage(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then(|| value.min(100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sampler_publishes_native_windows_and_stops_without_another_refresh() {
        let (sender, mut receiver) = watch::channel(HostTelemetry::default());
        let sampler = Sampler::start(sender, std::time::Instant::now());
        tokio::time::timeout(Duration::from_secs(5), receiver.changed())
            .await
            .unwrap()
            .unwrap();
        let sample = receiver.borrow().clone();
        if sysinfo::IS_SUPPORTED_SYSTEM {
            assert!(sample.sampled_at_us.is_some());
            assert!(sample.interval_us.unwrap() >= 1_000_000);
            assert!(sample.process_cpu_percent.is_some());
            assert!(sample.process_memory_bytes.unwrap() > 0);
        } else {
            assert!(matches!(sample.status, TelemetryStatus::Unsupported));
        }
        sampler.finish().await;
        let final_sample = serde_json::to_value(receiver.borrow().clone()).unwrap();
        assert!(receiver.changed().await.is_err());
        assert_eq!(
            serde_json::to_value(receiver.borrow().clone()).unwrap(),
            final_sample
        );
    }

    #[tokio::test]
    async fn dropping_sampler_disconnects_its_stop_channel() {
        let (sender, mut receiver) = watch::channel(HostTelemetry::default());
        let sampler = Sampler::start(sender, std::time::Instant::now());
        drop(sampler);
        tokio::time::timeout(Duration::from_secs(2), async {
            while receiver.changed().await.is_ok() {}
        })
        .await
        .unwrap();
        assert!(receiver.borrow().process_cpu_percent.is_none());
    }
    #[tokio::test]
    async fn unavailable_reader_publishes_nulls_and_remains_cancellable() {
        let (sender, mut receiver) = watch::channel(HostTelemetry::default());
        let (stop, stopped) = mpsc::sync_channel(0);
        let task = std::thread::spawn(move || {
            sample_with(sender, std::time::Instant::now(), stopped, || {
                unavailable(TelemetryStatus::Unavailable)
            });
        });
        tokio::time::timeout(Duration::from_secs(5), receiver.changed())
            .await
            .unwrap()
            .unwrap();
        let sample = receiver.borrow().clone();
        assert!(matches!(sample.status, TelemetryStatus::Unavailable));
        assert!(sample.sampled_at_us.is_some());
        assert!(sample.process_cpu_percent.is_none());
        assert!(sample.system_cpu_percent.is_none());
        assert!(sample.process_memory_bytes.is_none());
        drop(stop);
        tokio::task::spawn_blocking(move || task.join().unwrap())
            .await
            .unwrap();
        assert!(receiver.changed().await.is_err());
    }

    #[tokio::test]
    #[should_panic(expected = "sampler programming bug")]
    async fn sampler_programming_panics_follow_the_engine_panic_policy() {
        let (latest, _receiver) = watch::channel(HostTelemetry::default());
        let sampler = Sampler {
            stop: None,
            task: Some(std::thread::spawn(|| panic!("sampler programming bug"))),
            latest,
        };
        sampler.finish().await;
    }

    #[test]
    fn missing_process_does_not_become_zero_resource_usage() {
        let mut system = System::new();
        let sample = collect(&mut system, sysinfo::Pid::from_u32(u32::MAX));
        assert!(sample.process_cpu_percent.is_none());
        assert!(sample.process_memory_bytes.is_none());
    }

    #[test]
    fn wire_counters_preserve_precision_and_null_availability() {
        let sample = HostTelemetry {
            process_memory_bytes: Some(9_007_199_254_740_993),
            ..HostTelemetry::default()
        };
        let value = serde_json::to_value(sample).unwrap();
        assert_eq!(value["process_memory_bytes"], "9007199254740993");
        assert!(value["system_cpu_percent"].is_null());
        assert!(value["sampled_at_us"].is_null());
    }
}
