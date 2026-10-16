//! The telemetry and crash-reporting switch.
//!
//! Both are off unless the user turns them on (rule 7). The free app ships no
//! transport for them (ADR 0005): a report goes to a [`TelemetrySink`] that the
//! host hands in, and only when its flag is on. When a flag is off the sink is
//! never called, so nothing can be sent.

use std::panic::{self, PanicHookInfo};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// What the user allows. Stored in the config file; a missing key means off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TelemetrySettings {
    /// Anonymous usage statistics.
    pub usage_statistics: bool,
    /// A report when the app crashes.
    pub crash_reports: bool,
}

/// One report handed to a sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// A named usage event ("scan.finished"). No paths, no font names.
    Usage(String),
    /// A crash.
    Crash(CrashReport),
}

/// What a crash report holds: the panic message and where it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    /// The panic message.
    pub message: String,
    /// `file:line:column` of the panic, when known.
    pub location: Option<String>,
}

impl CrashReport {
    /// Builds a report from a panic.
    pub fn from_panic(info: &PanicHookInfo<'_>) -> Self {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic without a message".to_owned());
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        Self { message, location }
    }
}

/// Where reports go when the user allows them.
pub trait TelemetrySink: Send + Sync {
    /// Delivers one report.
    fn send(&self, report: &Report);
}

/// The gate between the app and a sink.
#[derive(Clone)]
pub struct Telemetry {
    settings: TelemetrySettings,
    sink: Option<Arc<dyn TelemetrySink>>,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Telemetry")
            .field("settings", &self.settings)
            .field("sink", &self.sink.is_some())
            .finish()
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::off()
    }
}

impl Telemetry {
    /// Everything off, no sink.
    pub fn off() -> Self {
        Self {
            settings: TelemetrySettings::default(),
            sink: None,
        }
    }

    /// The gate for `settings`, sending to `sink` what they allow.
    pub fn new(settings: TelemetrySettings, sink: Arc<dyn TelemetrySink>) -> Self {
        Self {
            settings,
            sink: Some(sink),
        }
    }

    /// The current settings.
    pub fn settings(&self) -> TelemetrySettings {
        self.settings
    }

    /// Sends a usage event if usage statistics are on. Returns whether it was sent.
    pub fn usage(&self, event: impl Into<String>) -> bool {
        self.send(self.settings.usage_statistics, || {
            Report::Usage(event.into())
        })
    }

    /// Sends a crash report if crash reports are on. Returns whether it was sent.
    pub fn crash(&self, report: CrashReport) -> bool {
        self.send(self.settings.crash_reports, || Report::Crash(report))
    }

    fn send(&self, allowed: bool, report: impl FnOnce() -> Report) -> bool {
        match (&self.sink, allowed) {
            (Some(sink), true) => {
                sink.send(&report());
                true
            }
            _ => false,
        }
    }

    /// Installs a panic hook that reports crashes, then runs the previous
    /// hook. With crash reports off it installs nothing and returns `false`.
    pub fn install_crash_hook(&self) -> bool {
        if !self.settings.crash_reports || self.sink.is_none() {
            return false;
        }
        let telemetry = self.clone();
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            telemetry.crash(CrashReport::from_panic(info));
            previous(info);
        }));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Report>>);

    impl TelemetrySink for Recorder {
        fn send(&self, report: &Report) {
            self.0.lock().unwrap().push(report.clone());
        }
    }

    impl Recorder {
        fn reports(&self) -> Vec<Report> {
            self.0.lock().unwrap().clone()
        }
    }

    fn crash() -> CrashReport {
        CrashReport {
            message: "boom".into(),
            location: None,
        }
    }

    #[test]
    fn defaults_are_off() {
        let s = TelemetrySettings::default();
        assert!(!s.usage_statistics);
        assert!(!s.crash_reports);
        assert_eq!(Telemetry::default().settings(), s);
    }

    #[test]
    fn missing_keys_in_the_config_mean_off() {
        let s: TelemetrySettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, TelemetrySettings::default());
        let s: TelemetrySettings = serde_json::from_str(r#"{"crashReports":true}"#).unwrap();
        assert!(s.crash_reports);
        assert!(!s.usage_statistics);
    }

    #[test]
    fn nothing_is_sent_when_off() {
        let sink = Arc::new(Recorder::default());
        let t = Telemetry::new(TelemetrySettings::default(), sink.clone());
        assert!(!t.usage("scan.finished"));
        assert!(!t.crash(crash()));
        assert!(!t.install_crash_hook());
        assert!(sink.reports().is_empty());
    }

    #[test]
    fn off_without_a_sink_sends_nothing_even_if_flags_are_on() {
        let mut t = Telemetry::off();
        t.settings = TelemetrySettings {
            usage_statistics: true,
            crash_reports: true,
        };
        assert!(!t.usage("scan.finished"));
        assert!(!t.crash(crash()));
        assert!(!t.install_crash_hook());
    }

    #[test]
    fn each_flag_gates_only_its_own_reports() {
        let sink = Arc::new(Recorder::default());
        let usage_only = TelemetrySettings {
            usage_statistics: true,
            crash_reports: false,
        };
        let t = Telemetry::new(usage_only, sink.clone());
        assert!(t.usage("scan.finished"));
        assert!(!t.crash(crash()));
        assert_eq!(sink.reports(), vec![Report::Usage("scan.finished".into())]);

        let sink = Arc::new(Recorder::default());
        let crash_only = TelemetrySettings {
            usage_statistics: false,
            crash_reports: true,
        };
        let t = Telemetry::new(crash_only, sink.clone());
        assert!(!t.usage("scan.finished"));
        assert!(t.crash(crash()));
        assert_eq!(sink.reports(), vec![Report::Crash(crash())]);
    }

    #[test]
    fn the_crash_hook_reports_a_panic_when_on() {
        let sink = Arc::new(Recorder::default());
        let on = TelemetrySettings {
            usage_statistics: false,
            crash_reports: true,
        };
        let t = Telemetry::new(on, sink.clone());
        let original = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        assert!(t.install_crash_hook());
        let result = panic::catch_unwind(|| panic!("index broke"));
        let _ = panic::take_hook();
        panic::set_hook(original);
        assert!(result.is_err());
        let reports = sink.reports();
        let crashes: Vec<_> = reports
            .iter()
            .filter_map(|r| match r {
                Report::Crash(c) if c.message == "index broke" => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(crashes.len(), 1);
        assert!(
            crashes[0]
                .location
                .as_deref()
                .is_some_and(|l| l.contains("telemetry.rs"))
        );
    }
}
