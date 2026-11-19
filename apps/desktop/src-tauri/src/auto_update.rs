//! The automatic update check (E55.7, ADR 0012): at start and then once a
//! day for as long as the app runs, the tray included.
//!
//! The decisions are made in `fm-api`: whether the app checks by itself
//! (`fm_api::update::checks_automatically`: the setting is on and this copy
//! updates itself) and when the next check is due
//! (`fm_api::update::automatic_check_wait`). This file is the timer: one
//! thread, parked on a condition variable between checks, that runs only
//! while automatic checks are on. A settings change wakes it ([`AutoCheck::start`]);
//! turned off, it ends, so no timer runs and no request is made (rule 7).
//!
//! A check that fails (offline, the server down) is silent: it counts as a
//! check, so the next one is a day later or at the next start, never sooner.
//! A found update is announced, never shown in a window that opens, rises or
//! takes the focus by itself.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use fm_api::update::{AUTOMATIC_CHECK_INTERVAL_SECS, AvailableUpdate, automatic_check_wait};
use fm_api::{ApiResult, Event};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::{AppState, EVENT_CHANNEL, tray, updater};

/// Asks the release server once; `Some` when it offers a newer version.
pub trait ReleaseServer: Send + Sync {
    /// One request to the release server.
    fn check(&self) -> ApiResult<Option<AvailableUpdate>>;
}

/// Whether automatic checks are on now.
pub type Enabled = Arc<dyn Fn() -> bool + Send + Sync>;
/// What happens with an update a check found.
pub type Announce = Arc<dyn Fn(AvailableUpdate) + Send + Sync>;

/// The timer of the automatic check; managed state of the app.
#[derive(Debug, Clone)]
pub struct AutoCheck {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    /// The clock the check times are counted on, in seconds from here.
    epoch: Instant,
    state: Mutex<TimerState>,
    wake: Condvar,
}

#[derive(Debug, Default)]
struct TimerState {
    /// When the last automatic check of this run started.
    last: Option<u64>,
    /// The timer thread runs.
    running: bool,
    /// A settings change asked the thread to look again.
    woken: bool,
    /// Requests sent to the release server by this timer.
    requests: u64,
}

impl Default for AutoCheck {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                epoch: Instant::now(),
                state: Mutex::default(),
                wake: Condvar::new(),
            }),
        }
    }
}

impl AutoCheck {
    fn lock(&self) -> MutexGuard<'_, TimerState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Seconds on this timer's clock.
    fn now(&self) -> u64 {
        self.inner.epoch.elapsed().as_secs()
    }

    /// Whether the timer thread runs.
    pub fn is_running(&self) -> bool {
        self.lock().running
    }

    /// How many requests this timer sent to the release server.
    pub fn requests(&self) -> u64 {
        self.lock().requests
    }

    /// One turn of the timer at `now` (seconds on its clock): asks `server`
    /// when a check is due and gives a found update to `announce`. Returns the
    /// seconds until the next check is due. The time is recorded before the
    /// request, so a failed check waits a full day too.
    pub fn round(
        &self,
        server: &dyn ReleaseServer,
        now: u64,
        announce: &dyn Fn(AvailableUpdate),
    ) -> u64 {
        {
            let mut st = self.lock();
            let wait = automatic_check_wait(st.last, now);
            if wait > 0 {
                return wait;
            }
            st.last = Some(now);
            st.requests += 1;
        }
        match server.check() {
            Ok(Some(update)) => announce(update),
            Ok(None) => {}
            // Silent: tried again at the next start or a day later.
            Err(e) => tracing::info!(error = %e, "the automatic update check failed"),
        }
        AUTOMATIC_CHECK_INTERVAL_SECS
    }

    /// Starts the timer when `enabled` says automatic checks are on and it
    /// does not run yet; when it runs, wakes it so it looks at the settings
    /// again (turned off, it ends). Returns whether automatic checks are on.
    /// Nothing starts and nothing is asked while they are off.
    pub fn start(
        &self,
        enabled: Enabled,
        server: Arc<dyn ReleaseServer>,
        announce: Announce,
    ) -> bool {
        let on = enabled();
        {
            let mut st = self.lock();
            if st.running {
                st.woken = true;
                self.inner.wake.notify_all();
                return on;
            }
            if !on {
                return false;
            }
            st.running = true;
        }
        let timer = self.clone();
        let spawned = std::thread::Builder::new()
            .name("fm-update-check".into())
            .spawn(move || timer.run(enabled.as_ref(), server.as_ref(), announce.as_ref()));
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "the automatic update check could not start");
            self.lock().running = false;
            return false;
        }
        true
    }

    fn run(
        &self,
        enabled: &dyn Fn() -> bool,
        server: &dyn ReleaseServer,
        announce: &dyn Fn(AvailableUpdate),
    ) {
        loop {
            {
                // Under the lock, so a `start` that sees the thread running is
                // never left without one.
                let mut st = self.lock();
                if !enabled() {
                    st.running = false;
                    return;
                }
            }
            let wait = self.round(server, self.now(), announce);
            let deadline = Instant::now() + Duration::from_secs(wait);
            let mut st = self.lock();
            while !st.woken {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                st = self
                    .inner
                    .wake
                    .wait_timeout(st, left)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
            st.woken = false;
        }
    }
}

/// The release server the app was built with, through Tauri's updater.
struct TauriServer<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> ReleaseServer for TauriServer<R> {
    fn check(&self) -> ApiResult<Option<AvailableUpdate>> {
        tauri::async_runtime::block_on(updater::check_and_keep(&self.app)).map(|c| c.update)
    }
}

/// Whether the app checks by itself now: the setting is on and this copy
/// updates itself.
fn enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<AppState>().is_some_and(|state| {
        fm_api::update::checks_automatically(&state.current_settings(), &updater::support())
    })
}

/// Announces `update`: the typed event to the UI, "Update to <version>…" in
/// the tray. No window is shown, raised or focused.
pub fn announce<R: Runtime>(app: &AppHandle<R>, update: AvailableUpdate) {
    tray::offer_update(app, &update.version);
    if let Err(e) = app.emit(EVENT_CHANNEL, Event::UpdateAvailable { update }) {
        tracing::warn!(error = %e, "could not announce the update to the UI");
    }
}

/// Starts, wakes or ends the app's automatic check after the settings may
/// have changed, and at start.
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let Some(timer) = app.try_state::<AutoCheck>() else {
        return;
    };
    let on = app.clone();
    let server = TauriServer { app: app.clone() };
    let found = app.clone();
    timer.start(
        Arc::new(move || enabled(&on)),
        Arc::new(server),
        Arc::new(move |update| announce(&found, update)),
    );
}

#[cfg(test)]
mod tests {
    // Test code: its helpers fail the test by panicking.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use fm_api::{ApiError, ErrorCode};

    use super::*;

    const DAY: u64 = AUTOMATIC_CHECK_INTERVAL_SECS;

    /// A release server that answers `answer` and counts the requests.
    struct Server {
        calls: AtomicU32,
        answer: fn() -> ApiResult<Option<AvailableUpdate>>,
    }

    impl Server {
        fn new(answer: fn() -> ApiResult<Option<AvailableUpdate>>) -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicU32::new(0),
                answer,
            })
        }

        fn calls(&self) -> u32 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl ReleaseServer for Server {
        fn check(&self) -> ApiResult<Option<AvailableUpdate>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            (self.answer)()
        }
    }

    fn offline() -> ApiResult<Option<AvailableUpdate>> {
        Err(ApiError::new(ErrorCode::Io, "offline"))
    }

    fn latest() -> ApiResult<Option<AvailableUpdate>> {
        Ok(None)
    }

    fn newer() -> ApiResult<Option<AvailableUpdate>> {
        AvailableUpdate::new("1.4.0", "1.5.0", None, None)
    }

    /// Fails the test on any request.
    struct NoRequest;

    impl ReleaseServer for NoRequest {
        fn check(&self) -> ApiResult<Option<AvailableUpdate>> {
            panic!("no request may reach the release server");
        }
    }

    fn ignore() -> Announce {
        Arc::new(|_| {})
    }

    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting: {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn the_first_round_checks_at_once_and_the_next_a_day_later() {
        let timer = AutoCheck::default();
        let server = Server::new(latest);
        assert_eq!(timer.round(server.as_ref(), 0, &|_| {}), DAY);
        assert_eq!(server.calls(), 1);
        assert_eq!(timer.round(server.as_ref(), 3_600, &|_| {}), DAY - 3_600);
        assert_eq!(server.calls(), 1, "no request before the day is over");
        assert_eq!(timer.round(server.as_ref(), DAY, &|_| {}), DAY);
        assert_eq!(server.calls(), 2);
    }

    #[test]
    fn a_failed_check_is_silent_and_tried_again_only_a_day_later() {
        let timer = AutoCheck::default();
        let server = Server::new(offline);
        let announced = AtomicU32::new(0);
        let count = |_: AvailableUpdate| {
            announced.fetch_add(1, Ordering::SeqCst);
        };
        assert_eq!(timer.round(server.as_ref(), 10, &count), DAY);
        for later in [11, 60, 3_600, DAY + 9] {
            assert!(timer.round(server.as_ref(), later, &count) > 0);
        }
        assert_eq!(server.calls(), 1, "a failure is not retried sooner");
        assert_eq!(timer.round(server.as_ref(), DAY + 10, &count), DAY);
        assert_eq!(server.calls(), 2);
        assert_eq!(announced.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_found_update_is_announced() {
        let timer = AutoCheck::default();
        let server = Server::new(newer);
        let found = Mutex::new(None);
        timer.round(server.as_ref(), 0, &|u| *found.lock().unwrap() = Some(u));
        let found = found.into_inner().unwrap().expect("announced");
        assert_eq!(found.version, "1.5.0");
    }

    #[test]
    fn with_checks_off_there_is_no_timer_and_no_request() {
        let timer = AutoCheck::default();
        let asked = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&asked);
        let enabled: Enabled = Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            false
        });
        assert!(!timer.start(Arc::clone(&enabled), Arc::new(NoRequest), ignore()));
        assert!(!timer.is_running());
        // Asked again, as after a settings change: still nothing starts.
        assert!(!timer.start(enabled, Arc::new(NoRequest), ignore()));
        std::thread::sleep(Duration::from_millis(50));
        assert!(!timer.is_running());
        assert_eq!(timer.requests(), 0);
        assert_eq!(asked.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn where_the_copy_does_not_update_itself_the_setting_starts_nothing() {
        let settings = fm_api::settings::Settings {
            check_updates_automatically: true,
            ..fm_api::settings::Settings::default()
        };
        let managed = fm_core::update::support(&fm_platform::Distribution::Flatpak);
        let timer = AutoCheck::default();
        let enabled: Enabled =
            Arc::new(move || fm_api::update::checks_automatically(&settings, &managed));
        assert!(!timer.start(enabled, Arc::new(NoRequest), ignore()));
        assert!(!timer.is_running());
        assert_eq!(timer.requests(), 0);
    }

    #[test]
    fn the_timer_checks_at_start_and_ends_when_turned_off() {
        let timer = AutoCheck::default();
        let on = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&on);
        let enabled: Enabled = Arc::new(move || flag.load(Ordering::SeqCst));
        let server = Server::new(latest);
        let as_server: Arc<dyn ReleaseServer> = server.clone();
        assert!(timer.start(Arc::clone(&enabled), Arc::clone(&as_server), ignore()));
        wait_until("the check at start", || server.calls() == 1);
        assert!(timer.is_running(), "it waits for the next day");

        // Turned off: the settings change wakes the timer and it ends.
        on.store(false, Ordering::SeqCst);
        assert!(!timer.start(Arc::clone(&enabled), Arc::clone(&as_server), ignore()));
        wait_until("the timer to end", || !timer.is_running());

        // Turned on again within the day: the timer runs, the check waits.
        on.store(true, Ordering::SeqCst);
        assert!(timer.start(Arc::clone(&enabled), as_server, ignore()));
        std::thread::sleep(Duration::from_millis(100));
        assert!(timer.is_running());
        assert_eq!(server.calls(), 1, "never sooner than a day after the last");
        on.store(false, Ordering::SeqCst);
        timer.start(enabled, Arc::new(NoRequest), ignore());
        wait_until("the timer to end", || !timer.is_running());
    }
}
