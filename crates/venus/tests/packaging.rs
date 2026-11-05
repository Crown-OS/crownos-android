#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use venus::{RENDER_SERVICE, SOCKET_GUEST};

/// Android's `/data` is the user's `~/.local/share/waydroid/data`.
const HOST_WAYDROID_DIR: &str = "%h/.local/share/waydroid";

fn packaging_file(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "../../packaging/venus", name]
        .iter()
        .collect();
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn directive<'a>(unit: &'a str, key: &str) -> Vec<&'a str> {
    unit.lines()
        .filter_map(|line| line.strip_prefix(key)?.strip_prefix('='))
        .collect()
}

#[test]
fn the_render_server_listens_where_android_connects() {
    let unit = packaging_file(RENDER_SERVICE);
    let socket = format!("{HOST_WAYDROID_DIR}{SOCKET_GUEST}");
    let exec_start = directive(&unit, "ExecStart");
    assert_eq!(exec_start.len(), 1);
    assert!(
        exec_start[0].ends_with(&format!("--socket-path {socket}")),
        "{}",
        exec_start[0]
    );
}

#[test]
fn the_render_server_creates_its_socket_dir_first() {
    let unit = packaging_file(RENDER_SERVICE);
    let socket = format!("{HOST_WAYDROID_DIR}{SOCKET_GUEST}");
    let socket_dir = Path::new(&socket).parent().unwrap().to_str().unwrap();
    assert!(
        directive(&unit, "ExecStartPre")
            .iter()
            .any(|command| command.ends_with(socket_dir))
    );
}

#[test]
fn the_daemon_starts_the_render_server_so_it_has_no_install_section() {
    let unit = packaging_file(RENDER_SERVICE);
    assert!(!unit.lines().any(|line| line.trim() == "[Install]"));
    assert_eq!(directive(&unit, "Restart"), ["on-failure"]);
}
