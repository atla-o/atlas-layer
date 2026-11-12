//! Contract for the background service that will own Filter and Shared Compute.
//!
//! The desktop app does not scan content and does not lend CPU. It sends
//! [`ServiceRequest`] and reads [`ServiceReport`]. Today [`MockService`]
//! answers in-process and starts no threads. A later daemon can speak the
//! same JSON over a local socket; the settings window stays on this side.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Filter,
    SharedCompute,
}

impl ServiceKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "filter" => Some(Self::Filter),
            "sharedCompute" | "shared-compute" | "compute" => Some(Self::SharedCompute),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::SharedCompute => "sharedCompute",
        }
    }
}

/// Messages the app would send to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ServiceRequest {
    SetEnabled { feature: String, enabled: bool },
    Status { feature: String },
    Shutdown,
}

/// What the daemon would answer. `running`, `contributed`, and `earned` stay
/// filled in even when the feature is off, so the readout never has a blank.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceReport {
    pub feature: String,
    pub enabled: bool,
    pub phase: String,
    pub detail: String,
    pub running: String,
    pub contributed: String,
    pub earned: String,
}

/// The seam the window uses. A socket client can implement this later.
pub trait BackgroundService {
    fn handle(&mut self, request: ServiceRequest) -> Result<Vec<ServiceReport>, String>;
}

#[derive(Debug, Clone)]
pub struct MockService {
    filter: bool,
    compute: bool,
}

impl MockService {
    pub fn from_flags(filter: bool, compute: bool) -> Self {
        Self { filter, compute }
    }

    pub fn pair(&self) -> (ServiceReport, ServiceReport) {
        (
            self.report(ServiceKind::Filter),
            self.report(ServiceKind::SharedCompute),
        )
    }

    fn report(&self, kind: ServiceKind) -> ServiceReport {
        match kind {
            ServiceKind::Filter if self.filter => ServiceReport {
                feature: "filter".into(),
                enabled: true,
                phase: "standing-by".into(),
                detail: "Armed. The filter engine is not installed yet, so nothing is scanned."
                    .into(),
                running: "Nothing".into(),
                contributed: "0".into(),
                earned: "0".into(),
            },
            ServiceKind::Filter => ServiceReport {
                feature: "filter".into(),
                enabled: false,
                phase: "off".into(),
                detail: "Off. Harmful and NSFW filtering is not running.".into(),
                running: "Nothing".into(),
                contributed: "0".into(),
                earned: "0".into(),
            },
            ServiceKind::SharedCompute if self.compute => ServiceReport {
                feature: "sharedCompute".into(),
                enabled: true,
                phase: "idle-wait".into(),
                detail: "On, and waiting. Shared Compute only runs when this machine is idle or plugged in."
                    .into(),
                running: "Nothing".into(),
                contributed: "0".into(),
                earned: "0".into(),
            },
            ServiceKind::SharedCompute => ServiceReport {
                feature: "sharedCompute".into(),
                enabled: false,
                phase: "off".into(),
                detail: "Off. This machine is not lending compute.".into(),
                running: "Nothing".into(),
                contributed: "0".into(),
                earned: "0".into(),
            },
        }
    }
}

impl BackgroundService for MockService {
    fn handle(&mut self, request: ServiceRequest) -> Result<Vec<ServiceReport>, String> {
        match request {
            ServiceRequest::Shutdown => {
                self.filter = false;
                self.compute = false;
                Ok(vec![
                    self.report(ServiceKind::Filter),
                    self.report(ServiceKind::SharedCompute),
                ])
            }
            ServiceRequest::Status { feature } => {
                let kind = ServiceKind::parse(&feature)
                    .ok_or_else(|| format!("Unknown service {feature}."))?;
                Ok(vec![self.report(kind)])
            }
            ServiceRequest::SetEnabled { feature, enabled } => {
                let kind = ServiceKind::parse(&feature)
                    .ok_or_else(|| format!("Unknown service {feature}."))?;
                match kind {
                    ServiceKind::Filter => self.filter = enabled,
                    ServiceKind::SharedCompute => self.compute = enabled,
                }
                Ok(vec![self.report(kind)])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_filter_arms_without_scanning() {
        let mut service = MockService::from_flags(false, false);
        let report = service
            .handle(ServiceRequest::SetEnabled {
                feature: "filter".into(),
                enabled: true,
            })
            .unwrap();
        assert_eq!(report[0].phase, "standing-by");
        assert_eq!(report[0].running, "Nothing");
        assert!(report[0].enabled);
        let off = service
            .handle(ServiceRequest::SetEnabled {
                feature: "filter".into(),
                enabled: false,
            })
            .unwrap();
        assert_eq!(off[0].phase, "off");
        assert!(!off[0].enabled);
    }

    #[test]
    fn shared_compute_turns_off_in_one_call() {
        let mut service = MockService::from_flags(false, true);
        let waiting = service
            .handle(ServiceRequest::Status {
                feature: "sharedCompute".into(),
            })
            .unwrap();
        assert_eq!(waiting[0].phase, "idle-wait");
        assert_eq!(waiting[0].contributed, "0");
        assert_eq!(waiting[0].earned, "0");
        assert!(waiting[0].detail.contains("idle or plugged in"));
        let off = service
            .handle(ServiceRequest::SetEnabled {
                feature: "sharedCompute".into(),
                enabled: false,
            })
            .unwrap();
        assert_eq!(off[0].phase, "off");
        assert_eq!(off[0].running, "Nothing");
        assert!(!off[0].enabled);
    }

    #[test]
    fn request_json_is_the_daemon_contract() {
        let request = ServiceRequest::SetEnabled {
            feature: "filter".into(),
            enabled: true,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            json,
            r#"{"op":"setEnabled","feature":"filter","enabled":true}"#
        );
        let parsed: ServiceRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, request);
        let shutdown = serde_json::to_string(&ServiceRequest::Shutdown).unwrap();
        assert_eq!(shutdown, r#"{"op":"shutdown"}"#);
    }
}
