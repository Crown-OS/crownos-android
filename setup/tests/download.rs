mod support;

use std::fs;
use std::time::Duration;

use android_setup::Paths;
use android_setup::images::download::{self, Download, DownloadError};
use android_setup::images::network::Backoff;
use android_setup::images::{self, IMAGE_NAMES};
use bundle::Flavor;
use setup_state::{SetupStatus, Stage};
use support::http::Server;
use support::machine::{ota_entry, pin};
use support::{Scratch, payload, sha256_hex};

const FAST: Backoff = Backoff {
    first: Duration::from_millis(10),
    max: Duration::from_millis(40),
};

struct Staged {
    _dir: Scratch,
    partial: std::path::PathBuf,
    complete: std::path::PathBuf,
}

fn staged(label: &str) -> std::io::Result<Staged> {
    let dir = Scratch::new(label)?;
    Ok(Staged {
        partial: dir.join("image.zip.part"),
        complete: dir.join("image.zip"),
        _dir: dir,
    })
}

fn fetch(
    server: &Server,
    staged: &Staged,
    bytes: &[u8],
    sha256: &str,
) -> Result<(), DownloadError> {
    let url = server.url("/image.zip");
    let download = Download {
        url: &url,
        size: bytes.len() as u64,
        sha256,
        partial: &staged.partial,
        complete: &staged.complete,
    };
    download::fetch(&download::agent(), &download, &mut |_| {})
}

#[test]
fn a_complete_download_is_verified_and_moved_into_place() -> anyhow::Result<()> {
    let bytes = payload(1, 300_000);
    let server = Server::start(vec![("/image.zip", bytes.clone())])?;
    let staged = staged("download-complete")?;
    fetch(&server, &staged, &bytes, &sha256_hex(&bytes))?;
    assert_eq!(fs::read(&staged.complete)?, bytes);
    assert!(!staged.partial.exists());
    assert_eq!(server.requests(), [("/image.zip".to_owned(), None)]);
    Ok(())
}

#[test]
fn an_interrupted_transfer_resumes_with_a_range_request() -> anyhow::Result<()> {
    let bytes = payload(2, 400_000);
    let server = Server::start(vec![("/image.zip", bytes.clone())])?;
    let staged = staged("download-resume")?;
    server.cut_next_response_after(150_000);
    let first = fetch(&server, &staged, &bytes, &sha256_hex(&bytes));
    assert!(
        matches!(first, Err(DownloadError::Interrupted(_))),
        "{first:?}"
    );
    assert_eq!(fs::metadata(&staged.partial)?.len(), 150_000);

    fetch(&server, &staged, &bytes, &sha256_hex(&bytes))?;
    assert_eq!(fs::read(&staged.complete)?, bytes);
    assert_eq!(
        server.requests().last(),
        Some(&("/image.zip".to_owned(), Some(150_000)))
    );
    Ok(())
}

#[test]
fn a_server_that_ignores_ranges_restarts_the_file() -> anyhow::Result<()> {
    let bytes = payload(3, 200_000);
    let server = Server::start(vec![("/image.zip", bytes.clone())])?;
    server.ignore_ranges();
    let staged = staged("download-no-range")?;
    fs::write(&staged.partial, &bytes[..50_000])?;
    fetch(&server, &staged, &bytes, &sha256_hex(&bytes))?;
    assert_eq!(fs::read(&staged.complete)?, bytes);
    Ok(())
}

#[test]
fn a_checksum_mismatch_deletes_the_download() -> anyhow::Result<()> {
    let bytes = payload(4, 100_000);
    let server = Server::start(vec![("/image.zip", bytes.clone())])?;
    let staged = staged("download-checksum")?;
    let result = fetch(&server, &staged, &bytes, &sha256_hex(b"something else"));
    assert!(
        matches!(result, Err(DownloadError::Checksum { .. })),
        "{result:?}"
    );
    assert!(!staged.partial.exists() && !staged.complete.exists());
    Ok(())
}

#[test]
fn a_missing_file_is_refused_without_retrying() -> anyhow::Result<()> {
    let server = Server::start(Vec::new())?;
    let staged = staged("download-404")?;
    let result = fetch(&server, &staged, b"x", &sha256_hex(b"x"));
    assert!(
        matches!(result, Err(DownloadError::Refused(_))),
        "{result:?}"
    );
    Ok(())
}

struct Images {
    machine_root: Scratch,
    paths: Paths,
    system_zip: Vec<u8>,
    vendor_zip: Vec<u8>,
}

fn images(label: &str) -> anyhow::Result<Images> {
    let machine_root = Scratch::new(label)?;
    let paths = Paths::under(&machine_root);
    Ok(Images {
        system_zip: support::zip(&[("system.img", &payload(5, 600_000))])?,
        vendor_zip: support::zip(&[("vendor.img", &payload(6, 200_000))])?,
        machine_root,
        paths,
    })
}

#[test]
fn images_download_with_progress_after_waiting_for_the_network() -> anyhow::Result<()> {
    let images_case = images("images-network")?;
    let reserved = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = reserved.local_addr()?;
    drop(reserved);
    let base = format!("http://{address}");
    let pinned = pin(
        Flavor::Vanilla,
        ota_entry(
            1775215493,
            "system.zip",
            "VANILLA",
            format!("{base}/system.zip"),
            &images_case.system_zip,
        ),
        ota_entry(
            1777409843,
            "vendor.zip",
            "MAINLINE",
            format!("{base}/vendor.zip"),
            &images_case.vendor_zip,
        ),
    );
    let files = vec![
        ("/system.zip", images_case.system_zip.clone()),
        ("/vendor.zip", images_case.vendor_zip.clone()),
    ];
    let starter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        std::net::TcpListener::bind(address).and_then(|listener| Server::start_on(listener, files))
    });

    let mut statuses: Vec<SetupStatus> = Vec::new();
    let dir = images::ensure_unpacked(&images_case.paths, &pinned, FAST, &mut statuses)?;
    let _server = starter
        .join()
        .map_err(|_| anyhow::anyhow!("the server thread panicked"))??;

    assert_eq!(
        dir,
        images_case.paths.images().join("1775215493-1777409843")
    );
    for image in IMAGE_NAMES {
        assert!(dir.join(image).is_file(), "{image}");
    }
    let mut staged: Vec<_> = fs::read_dir(images_case.paths.staging())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    staged.sort();
    assert_eq!(
        staged,
        ["system.zip", "vendor.zip"],
        "verified zips wait for waydroid init"
    );
    let stages: Vec<Stage> = statuses.iter().map(|status| status.stage).collect();
    assert_eq!(stages.first(), Some(&Stage::WaitingForNetwork));
    assert_eq!(stages.last(), Some(&Stage::Verifying));
    let percents: Vec<u8> = statuses
        .iter()
        .filter_map(|status| status.percent)
        .collect();
    assert!(
        percents.windows(2).all(|pair| pair[0] <= pair[1]),
        "{percents:?}"
    );
    assert_eq!(percents.last(), Some(&100));
    drop(images_case.machine_root);
    Ok(())
}

#[test]
fn a_partial_image_left_by_a_reboot_is_resumed_and_retries_are_automatic() -> anyhow::Result<()> {
    let images_case = images("images-reboot")?;
    let server = Server::start(vec![
        ("/system.zip", images_case.system_zip.clone()),
        ("/vendor.zip", images_case.vendor_zip.clone()),
    ])?;
    let pinned = pin(
        Flavor::Gapps,
        ota_entry(
            10,
            "system.zip",
            "GAPPS",
            server.url("/system.zip"),
            &images_case.system_zip,
        ),
        ota_entry(
            20,
            "vendor.zip",
            "MAINLINE",
            server.url("/vendor.zip"),
            &images_case.vendor_zip,
        ),
    );
    let staging = images_case.paths.staging();
    fs::create_dir_all(&staging)?;
    fs::write(
        staging.join("system.zip.part"),
        &images_case.system_zip[..250_000],
    )?;
    fs::write(staging.join("stale-other-flavor.zip"), "old")?;
    server.cut_next_response_after(100_000);

    let mut statuses: Vec<SetupStatus> = Vec::new();
    let dir = images::ensure_unpacked(&images_case.paths, &pinned, FAST, &mut statuses)?;

    assert!(dir.join("system.img").is_file() && dir.join("vendor.img").is_file());
    let system_requests: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|(path, _)| path == "/system.zip")
        .collect();
    assert_eq!(
        system_requests.first().map(|request| request.1),
        Some(Some(250_000))
    );
    assert_eq!(
        system_requests.get(1).map(|request| request.1),
        Some(Some(350_000))
    );
    let first_percent = statuses.iter().find_map(|status| status.percent);
    assert!(
        first_percent.is_some_and(|percent| percent > 0),
        "{first_percent:?}"
    );
    assert!(!staging.join("stale-other-flavor.zip").exists());

    let again = images::ensure_unpacked(&images_case.paths, &pinned, FAST, &mut statuses)?;
    assert_eq!(again, dir);
    Ok(())
}
