//! Spare-compute engine for Creation's Roleplay sim.
//!
//! This is a local worker with a small public interface. It does not join a
//! paid network and it does not pay out real money. Credits are sim ticks.
//! The switch starts it off. It pauses on battery and while the person is active.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::services::{ServiceKind, ServiceReport};

pub const IDLE_AFTER: Duration = Duration::from_secs(45);
const STEPS_PER_TICK: u64 = 2_000;

pub trait SpareEngine {
    fn set_enabled(&mut self, enabled: bool);
    fn note_activity(&mut self);
    fn tick(&mut self, on_battery: bool);
    fn report(&self) -> ServiceReport;
}

#[derive(Debug)]
pub struct LocalCompute {
    enabled: bool,
    last_activity: Instant,
    ticks: u64,
    credits: u64,
    phase: &'static str,
}

impl LocalCompute {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_activity: Instant::now(),
            ticks: 0,
            credits: 0,
            phase: if enabled { "paused-active" } else { "off" },
        }
    }

    pub fn load(path: &Path, enabled: bool) -> Self {
        let mut engine = Self::new(enabled);
        if let Ok(text) = fs::read_to_string(path) {
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("ticks=") {
                    engine.ticks = value.trim().parse().unwrap_or(0);
                } else if let Some(value) = line.strip_prefix("credits=") {
                    engine.credits = value.trim().parse().unwrap_or(0);
                }
            }
        }
        engine
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let body = format!("ticks={}\ncredits={}\n", self.ticks, self.credits);
        let _ = fs::write(path, body);
    }
}

/// One Roleplay sim step. Deterministic and cheap. Other Creation sims can call this.
pub fn roleplay_step(state: u64) -> u64 {
    state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

pub fn decide(enabled: bool, idle: bool, on_battery: bool) -> &'static str {
    if !enabled {
        "off"
    } else if on_battery {
        "paused-battery"
    } else if !idle {
        "paused-active"
    } else {
        "working"
    }
}

/// True when any power supply reports Discharging. No battery info means plugged in.
pub fn battery_discharging(statuses: &[String]) -> bool {
    statuses
        .iter()
        .any(|status| status.eq_ignore_ascii_case("discharging"))
}

impl SpareEngine for LocalCompute {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.phase = "off";
        }
    }

    fn note_activity(&mut self) {
        self.last_activity = Instant::now();
        if self.enabled && self.phase == "working" {
            self.phase = "paused-active";
        }
    }

    fn tick(&mut self, on_battery: bool) {
        let idle = self.last_activity.elapsed() >= IDLE_AFTER;
        self.phase = decide(self.enabled, idle, on_battery);
        if self.phase != "working" {
            return;
        }
        let mut state = self.ticks;
        for _ in 0..STEPS_PER_TICK {
            state = roleplay_step(state);
        }
        self.ticks = self.ticks.saturating_add(STEPS_PER_TICK);
        self.credits = self.ticks / 1_000;
    }

    fn report(&self) -> ServiceReport {
        let (running, detail) = match self.phase {
            "working" => (
                "Roleplay sim",
                "On. Running a capped Roleplay sim tick while this machine is idle and plugged in.",
            ),
            "paused-battery" => (
                "Paused",
                "On, and paused because the machine is on battery. Plug in to contribute.",
            ),
            "paused-active" => (
                "Paused",
                "On, and paused while you are using the machine. It waits until you are idle.",
            ),
            _ => (
                "Nothing",
                "Off. This machine is not lending compute. Credits stay on this computer; there is no payout network.",
            ),
        };
        ServiceReport {
            feature: ServiceKind::SharedCompute.key().into(),
            enabled: self.enabled,
            phase: self.phase.into(),
            detail: detail.into(),
            running: running.into(),
            contributed: format!("{} ticks", self.ticks),
            earned: format!("{} credits", self.credits),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{battery_discharging, decide, roleplay_step, LocalCompute, SpareEngine};

    #[test]
    fn stays_off_until_enabled_and_pauses_on_battery_or_activity() {
        assert_eq!(decide(false, true, false), "off");
        assert_eq!(decide(true, false, false), "paused-active");
        assert_eq!(decide(true, true, true), "paused-battery");
        assert_eq!(decide(true, true, false), "working");
        assert!(battery_discharging(&["Discharging".into()]));
        assert!(!battery_discharging(&["Full".into(), "Charging".into()]));
        assert!(!battery_discharging(&[]));
    }

    #[test]
    fn a_working_tick_adds_capped_steps_and_credits() {
        let mut engine = LocalCompute::new(true);
        engine.last_activity = std::time::Instant::now() - super::IDLE_AFTER;
        let before = roleplay_step(0);
        assert_ne!(before, 0);
        engine.tick(false);
        assert_eq!(engine.phase, "working");
        assert_eq!(engine.ticks, super::STEPS_PER_TICK);
        assert_eq!(engine.credits, super::STEPS_PER_TICK / 1_000);
        engine.tick(true);
        assert_eq!(engine.phase, "paused-battery");
        assert_eq!(engine.ticks, super::STEPS_PER_TICK);
        engine.set_enabled(false);
        engine.tick(false);
        assert_eq!(engine.report().phase, "off");
        assert_eq!(engine.ticks, super::STEPS_PER_TICK);
    }
}
