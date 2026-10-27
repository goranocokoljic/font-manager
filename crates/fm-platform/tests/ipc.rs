//! The IPC listener (E29.1 criteria 1 and 2): a round trip, only the user
//! can connect, a stale socket is replaced and the socket goes on drop.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use fm_platform::ipc::connect;
use fm_platform::{IpcListener, IpcName};

/// A fresh name for this test; on Unix the folder is removed at the end.
struct Scratch {
    name: IpcName,
    #[cfg(unix)]
    dir: std::path::PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let tag = format!(
            "fm-ipc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        #[cfg(windows)]
        {
            Self {
                name: IpcName::Pipe(format!(r"\\.\pipe\{tag}")),
            }
        }
        #[cfg(unix)]
        {
            // Short: a socket path must fit in `sun_path` (104 bytes on macOS).
            let dir = std::env::temp_dir().join(tag);
            Self {
                name: IpcName::Socket(dir.join("fm.sock")),
                dir,
            }
        }
    }
}

#[cfg(unix)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Accepts one client on a thread and echoes one message back.
fn echo_once(listener: IpcListener) -> std::thread::JoinHandle<IpcListener> {
    std::thread::spawn(move || {
        let mut stream = listener.accept().expect("accept");
        let mut buf = [0u8; 5];
        stream.read_exact(&mut buf).expect("read");
        stream.write_all(&buf).expect("write");
        stream.flush().expect("flush");
        listener
    })
}

fn round_trip(name: &IpcName) {
    let mut client = connect(name).expect("connect");
    client.write_all(b"hello").expect("send");
    client.flush().expect("flush");
    let mut back = [0u8; 5];
    client.read_exact(&mut back).expect("receive");
    assert_eq!(&back, b"hello");
}

#[test]
fn a_client_reaches_the_listener_and_only_the_user_may() {
    let scratch = Scratch::new();
    let listener = IpcListener::bind(&scratch.name).expect("bind");
    assert_eq!(listener.name(), &scratch.name);
    assert!(listener.is_private().expect("permissions"), "not private");
    let server = echo_once(listener);
    round_trip(&scratch.name);
    let listener = server.join().expect("server");
    assert!(listener.is_private().expect("permissions"));
}

#[test]
fn a_second_listener_on_a_live_name_is_refused() {
    let scratch = Scratch::new();
    let _first = IpcListener::bind(&scratch.name).expect("bind");
    let second = IpcListener::bind(&scratch.name).expect_err("in use");
    #[cfg(unix)]
    assert_eq!(second.kind(), std::io::ErrorKind::AddrInUse, "{second}");
    #[cfg(windows)]
    assert_eq!(
        second.kind(),
        std::io::ErrorKind::PermissionDenied,
        "FILE_FLAG_FIRST_PIPE_INSTANCE: {second}"
    );
}

#[test]
fn the_name_is_free_again_after_the_listener_drops() {
    let scratch = Scratch::new();
    drop(IpcListener::bind(&scratch.name).expect("bind"));
    assert!(connect(&scratch.name).is_err(), "nothing listens");
    let again = IpcListener::bind(&scratch.name).expect("bind again");
    let server = echo_once(again);
    round_trip(&scratch.name);
    server.join().expect("server");
}

/// E29.4 criterion 3: a client of an app that quit (or never ran) gets a
/// plain "not running" error at once, not a wait.
#[test]
fn connecting_where_nothing_listens_says_font_manager_is_not_running() {
    let never = Scratch::new();
    let quit = Scratch::new();
    drop(IpcListener::bind(&quit.name).expect("bind"));
    for name in [&never.name, &quit.name] {
        let start = std::time::Instant::now();
        let e = connect(name).expect_err("nothing listens");
        assert!(
            start.elapsed() < std::time::Duration::from_secs(3),
            "{name}: waited {:?}",
            start.elapsed()
        );
        assert!(
            e.to_string().starts_with(fm_platform::ipc::NOT_RUNNING),
            "{name}: {e}"
        );
    }
}

#[test]
fn a_name_of_the_other_os_family_is_an_error() {
    #[cfg(windows)]
    let other = IpcName::Socket("fm.sock".into());
    #[cfg(unix)]
    let other = IpcName::Pipe(r"\\.\pipe\fm".into());
    let e = IpcListener::bind(&other).expect_err("wrong kind");
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
    assert!(connect(&other).is_err());
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};

    use super::*;

    fn path(scratch: &Scratch) -> &std::path::Path {
        match &scratch.name {
            IpcName::Socket(path) => path,
            IpcName::Pipe(_) => unreachable!(),
        }
    }

    fn mode(path: &std::path::Path) -> u32 {
        std::fs::symlink_metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn the_socket_is_0600_in_a_0700_folder_even_when_the_folder_was_open() {
        let scratch = Scratch::new();
        std::fs::create_dir_all(&scratch.dir).expect("dir");
        std::fs::set_permissions(&scratch.dir, std::fs::Permissions::from_mode(0o755))
            .expect("open the folder");
        let listener = IpcListener::bind(&scratch.name).expect("bind");
        assert_eq!(mode(path(&scratch)), 0o600);
        assert_eq!(mode(&scratch.dir), 0o700);
        assert!(listener.is_private().expect("permissions"));
        // Another program widens the socket: no longer private.
        std::fs::set_permissions(path(&scratch), std::fs::Permissions::from_mode(0o666))
            .expect("widen");
        assert!(!listener.is_private().expect("permissions"));
    }

    #[test]
    fn a_stale_socket_from_a_crashed_run_is_replaced() {
        let scratch = Scratch::new();
        std::fs::create_dir_all(&scratch.dir).expect("dir");
        // std's listener leaves its file behind when it drops, as a crash does.
        drop(std::os::unix::net::UnixListener::bind(path(&scratch)).expect("stale"));
        assert!(
            std::fs::symlink_metadata(path(&scratch))
                .expect("stale file")
                .file_type()
                .is_socket()
        );
        let listener = IpcListener::bind(&scratch.name).expect("replaces the stale socket");
        let server = echo_once(listener);
        round_trip(&scratch.name);
        server.join().expect("server");
    }

    #[test]
    fn the_socket_is_removed_when_the_listener_drops() {
        let scratch = Scratch::new();
        let listener = IpcListener::bind(&scratch.name).expect("bind");
        assert!(path(&scratch).exists());
        drop(listener);
        assert!(
            std::fs::symlink_metadata(path(&scratch)).is_err(),
            "the socket file is gone"
        );
    }

    #[test]
    fn a_file_that_is_not_a_socket_is_left_alone() {
        let scratch = Scratch::new();
        std::fs::create_dir_all(&scratch.dir).expect("dir");
        std::fs::write(path(&scratch), b"user data").expect("file");
        let e = IpcListener::bind(&scratch.name).expect_err("not a socket");
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists, "{e}");
        assert_eq!(
            std::fs::read(path(&scratch)).expect("still there"),
            b"user data"
        );
    }
}

#[test]
fn a_closer_ends_a_read_waiting_on_another_thread() {
    let scratch = Scratch::new();
    let listener = IpcListener::bind(&scratch.name).expect("bind");
    let name = scratch.name.clone();
    let client = std::thread::spawn(move || connect(&name).expect("connect"));
    let mut server_end = listener.accept().expect("accept");
    let mut client_end = client.join().expect("client");
    let closer = server_end.closer().expect("closer");
    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 8];
        // Nothing is ever sent: only the closer ends this read.
        let read = server_end.read(&mut buf);
        assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    closer.close();
    reader.join().expect("the read returned");
    // The client sees the connection closed.
    let mut buf = [0u8; 1];
    assert!(matches!(client_end.read(&mut buf), Ok(0) | Err(_)));
    // Closing again does nothing.
    closer.close();
}
