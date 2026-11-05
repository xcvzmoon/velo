use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::RunError;

/// One linear transition from the preceding VU count to a target.
///
/// # Examples
/// ```
/// let stage = velo_engine::RampStage { duration_ms: 1_000, target_vus: 10 };
/// assert_eq!(stage.target_vus, 10);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RampStage {
    /// Positive stage duration in milliseconds.
    pub duration_ms: u32,
    /// Desired workers at the stage endpoint; zero is allowed.
    pub target_vus: u32,
}

/// Scheduling model for sequential virtual users sharing one connection pool.
///
/// Ramps interpolate whole VUs toward each target using monotonic time. Workers
/// retire after their current request; actual VUs can temporarily exceed desired
/// VUs. The peak allocation includes retiring workers. The default workload runs
/// one constant VU for ten seconds.
///
/// # Examples
/// ```
/// use velo_engine::{RampStage, Workload};
/// let workload = Workload::Ramping {
///     start_vus: 0,
///     stages: vec![RampStage { duration_ms: 1_000, target_vus: 5 }],
/// };
/// assert!(matches!(workload, Workload::Ramping { start_vus: 0, .. }));
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Workload {
    /// Fixed positive worker count for a positive scheduling duration.
    Constant {
        /// Sequential worker count.
        vus: u32,
        /// Scheduling duration in milliseconds.
        duration_ms: u32,
    },
    /// Ordered linear transitions, including zero-worker stages.
    Ramping {
        /// Initial worker count; zero is allowed.
        start_vus: u32,
        /// Nonempty stage schedule; total duration must fit a positive u32 in milliseconds.
        #[ts(type = "RampStage[]")]
        stages: Vec<RampStage>,
    },
}

impl Default for Workload {
    fn default() -> Self {
        Self::Constant {
            vus: 1,
            duration_ms: 10_000,
        }
    }
}

struct Segment {
    start_ns: u128,
    end_ns: u128,
    start_vus: u32,
    target_vus: u32,
}

pub(crate) struct Schedule {
    segments: Vec<Segment>,
    pub peak_vus: u32,
    pub duration_ms: u32,
}

impl Schedule {
    pub fn new(workload: &Workload, budget: u32, byte_budget: u32) -> Result<Self, RunError> {
        let invalid = |message| RunError::new("INVALID_RUN_CONFIG", message, Some("workload"));
        let (start, stages) = match workload {
            Workload::Constant { vus, duration_ms } => {
                if *vus == 0 {
                    return Err(RunError::new(
                        "VU_BUDGET_EXCEEDED",
                        "Constant VUs must be positive.",
                        Some("workload.vus"),
                    ));
                }
                (
                    *vus,
                    vec![RampStage {
                        duration_ms: *duration_ms,
                        target_vus: *vus,
                    }],
                )
            }
            Workload::Ramping { start_vus, stages } => {
                if stages.is_empty() {
                    return Err(invalid("Add at least one ramp stage."));
                }
                let bytes = stages.len().saturating_mul(
                    std::mem::size_of::<Segment>() + 2 * std::mem::size_of::<RampStage>(),
                );
                if bytes > byte_budget as usize {
                    return Err(RunError::new(
                        "REQUEST_BUDGET_EXCEEDED",
                        "Stage configuration exceeds the byte budget.",
                        Some("workload.stages"),
                    ));
                }
                (*start_vus, stages.clone())
            }
        };
        let mut segments = Vec::with_capacity(stages.len());
        let mut duration_ms = 0_u32;
        let mut peak_vus = start;
        let mut start_vus = start;
        for stage in stages {
            if stage.duration_ms == 0 {
                return Err(invalid("Stage durations must be positive."));
            }
            let start_ns = u128::from(duration_ms) * 1_000_000;
            duration_ms = duration_ms.checked_add(stage.duration_ms).ok_or_else(|| {
                invalid("Total scheduling duration exceeds the supported representation.")
            })?;
            peak_vus = peak_vus.max(stage.target_vus);
            segments.push(Segment {
                start_ns,
                end_ns: u128::from(duration_ms) * 1_000_000,
                start_vus,
                target_vus: stage.target_vus,
            });
            start_vus = stage.target_vus;
        }
        if peak_vus > budget || budget == 0 {
            return Err(RunError::new(
                "VU_BUDGET_EXCEEDED",
                "Peak VUs exceed the configured worker budget.",
                Some("workload"),
            ));
        }
        Ok(Self {
            segments,
            peak_vus,
            duration_ms,
        })
    }

    pub fn retained_bytes(&self) -> usize {
        self.segments
            .len()
            .saturating_mul(std::mem::size_of::<Segment>() + 2 * std::mem::size_of::<RampStage>())
    }

    pub fn position(&self, elapsed: Duration) -> (u32, Duration) {
        let now = elapsed.as_nanos();
        let index = self
            .segments
            .partition_point(|segment| segment.end_ns <= now);
        let Some(segment) = self.segments.get(index) else {
            return (0, Duration::from_millis(self.duration_ms.into()));
        };
        let duration = segment.end_ns - segment.start_ns;
        let delta = segment.start_vus.abs_diff(segment.target_vus);
        let (desired, next) = if delta == 0 {
            (segment.start_vus, segment.end_ns)
        } else {
            let completed = u128::from(delta) * (now - segment.start_ns) / duration;
            let desired = if segment.target_vus >= segment.start_vus {
                segment.start_vus + completed as u32
            } else {
                segment.start_vus - completed as u32
            };
            let next = segment.start_ns + ((completed + 1) * duration).div_ceil(u128::from(delta));
            (desired, next)
        };
        (desired, Duration::from_nanos(next as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_boundaries_zero_stages_and_delayed_wakes() {
        let schedule = Schedule::new(
            &Workload::Ramping {
                start_vus: 0,
                stages: vec![
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 4,
                    },
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 0,
                    },
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 0,
                    },
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 2,
                    },
                ],
            },
            4,
            8_388_608,
        )
        .unwrap();
        for (millis, desired, next) in [
            (0, 0, 250),
            (249, 0, 250),
            (250, 1, 500),
            (999, 3, 1_000),
            (1_000, 4, 1_250),
            (1_250, 3, 1_500),
            (1_999, 1, 2_000),
            (2_000, 0, 3_000),
            (3_000, 0, 3_500),
            (3_750, 1, 4_000),
            (4_000, 0, 4_000),
        ] {
            assert_eq!(
                schedule.position(Duration::from_millis(millis)),
                (desired, Duration::from_millis(next))
            );
        }
    }

    #[test]
    fn nanosecond_transitions_and_large_representation_are_exact() {
        let schedule = Schedule::new(
            &Workload::Ramping {
                start_vus: 0,
                stages: vec![RampStage {
                    duration_ms: u32::MAX,
                    target_vus: u32::MAX,
                }],
            },
            u32::MAX,
            1_024,
        )
        .unwrap();
        assert_eq!(
            schedule.position(Duration::from_millis(1)),
            (1, Duration::from_millis(2))
        );
        let schedule = Schedule::new(
            &Workload::Ramping {
                start_vus: 3,
                stages: vec![RampStage {
                    duration_ms: 1,
                    target_vus: 0,
                }],
            },
            3,
            1_024,
        )
        .unwrap();
        assert_eq!(
            schedule.position(Duration::from_nanos(333_333)),
            (3, Duration::from_nanos(333_334))
        );
        assert_eq!(
            schedule.position(Duration::from_nanos(333_334)),
            (2, Duration::from_nanos(666_667))
        );
    }
}
