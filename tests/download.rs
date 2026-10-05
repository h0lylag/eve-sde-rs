#![cfg(feature = "download")]
mod common;

use common::{build_line, build_only_zip, stored_zip_bytes};
use eve_sde::Error;
use eve_sde::download::{Client, Latest, MAX_DOWNLOAD_BYTES, Update, local_build};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

struct Request {
    path: String,
    headers: HashMap<String, String>,
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn ok(body: impl Into<Vec<u8>>) -> Response {
        Response {
            status: 200,
            headers: Vec::new(),
            body: body.into(),
        }
    }
    fn status(status: u16) -> Response {
        Response {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
    fn header(mut self, name: &str, value: &str) -> Response {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

/// Start an HTTP server that answers with `handler`, one connection at a
/// time. Returns its base URL.
fn serve(handler: impl Fn(&Request) -> Response + Send + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let mut headers = HashMap::new();
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                let Some((k, v)) = line.trim_end().split_once(':') else {
                    break;
                };
                headers.insert(k.to_lowercase(), v.trim().to_owned());
            }
            let response = handler(&Request { path, headers });
            let mut head = format!("HTTP/1.1 {} X\r\nConnection: close\r\n", response.status);
            let mut has_length = false;
            for (k, v) in &response.headers {
                has_length |= k.eq_ignore_ascii_case("content-length");
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            if !has_length {
                head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&response.body);
            let _ = stream.flush();
            let mut sink = Vec::new();
            let _ = reader.read_to_end(&mut sink);
        }
    });
    base
}

fn client(base: &str) -> Client {
    Client::new("eve-sde-test/0").with_base_url(base)
}

fn zip_path(build: u32) -> String {
    format!("/eve-online-static-data-{build}-jsonl.zip")
}

/// A server that lists `latest` as the latest build, with the ETag `"tag"`,
/// and serves each ZIP in `zips` at its URL.
fn release_server(latest: u32, zips: Vec<(u32, Vec<u8>)>) -> String {
    serve(move |req| {
        if req.path == "/latest.jsonl" {
            if req.headers.get("if-none-match").map(String::as_str) == Some("\"tag\"") {
                return Response::status(304);
            }
            return Response::ok(build_line(latest)).header("ETag", "\"tag\"");
        }
        for (build, bytes) in &zips {
            if req.path == zip_path(*build) {
                return Response::ok(bytes.clone());
            }
        }
        Response::status(404)
    })
}

/// The sorted names of the files in `dir`.
fn only_file(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn latest_reports_build_and_honours_etag() {
    let client = client(&release_server(77, vec![]));
    let Latest::Available { build, etag } = client.latest(None).unwrap() else {
        panic!("expected a build");
    };
    assert_eq!(build.build_number, 77);
    assert_eq!(build.release_date.as_deref(), Some("2026-10-02T11:08:57Z"));
    assert_eq!(etag.as_deref(), Some("\"tag\""));
    assert_eq!(client.latest(etag.as_deref()).unwrap(), Latest::Unchanged);
}

#[test]
fn latest_rejects_errors_and_garbage() {
    let base = serve(|_| Response::status(503));
    let err = client(&base).latest(None).unwrap_err();
    assert!(matches!(
        err,
        Error::HttpStatus { url, status: 503 } if url == format!("{base}/latest.jsonl")
    ));
    let base = serve(|_| Response::ok("hello"));
    assert!(client(&base).latest(None).is_err());
}

#[test]
fn latest_reads_first_nonblank_record() {
    let base = serve(|_| Response::ok(format!("\n  \n{}not json\n", build_line(77))));
    let Latest::Available { build, .. } = client(&base).latest(None).unwrap() else {
        panic!("expected a build");
    };
    assert_eq!(build.build_number, 77);
}

#[test]
fn unsolicited_not_modified_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    let original = build_only_zip(1);
    std::fs::write(&dest, &original).unwrap();
    let base = serve(|_| Response::status(304));
    let client = client(&base);

    for err in [
        client.latest(None).unwrap_err(),
        client.update(&dest).unwrap_err(),
    ] {
        assert!(matches!(
            err,
            Error::HttpStatus { url, status: 304 } if url == format!("{base}/latest.jsonl")
        ));
    }
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    assert_eq!(only_file(dir.path()), ["sde.zip"]);
}

#[test]
fn download_replaces_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    std::fs::write(&dest, build_only_zip(1)).unwrap();
    let client = client(&release_server(2, vec![(2, build_only_zip(2))]));

    let mut calls = 0;
    let info = client
        .download(2, &dest, |_, _| {
            calls += 1;
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(info.build_number, 2);
    assert!(calls > 0);
    assert_eq!(local_build(&dest).unwrap().unwrap().build_number, 2);
    assert_eq!(only_file(dir.path()), [".sde.zip.lock", "sde.zip"]);
}

#[test]
fn wrong_build_leaves_old_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    std::fs::write(&dest, build_only_zip(1)).unwrap();
    let client = client(&release_server(2, vec![(2, build_only_zip(9))]));

    let err = client
        .download(2, &dest, |_, _| ControlFlow::Continue(()))
        .unwrap_err();
    assert!(matches!(
        err,
        Error::BuildMismatch {
            expected: 2,
            got: 9
        }
    ));
    assert_eq!(local_build(&dest).unwrap().unwrap().build_number, 1);
    assert_eq!(only_file(dir.path()), ["sde.zip"]);
}

#[test]
fn corrupt_download_leaves_old_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    std::fs::write(&dest, build_only_zip(1)).unwrap();
    let mut broken = stored_zip_bytes(&[
        ("_sde.jsonl", &build_line(2)),
        ("types.jsonl", "x".repeat(5000).as_str()),
    ]);
    // Damage the stored data of the second file, keeping the directory intact.
    let at = broken.windows(5).rposition(|w| w == b"xxxxx").unwrap();
    broken[at] = b'y';
    let client = client(&release_server(2, vec![(2, broken)]));

    let err = client
        .download(2, &dest, |_, _| ControlFlow::Continue(()))
        .unwrap_err();
    assert!(
        matches!(&err, Error::Read { file, .. } if file == "types.jsonl"),
        "{err:?}"
    );
    assert_eq!(local_build(&dest).unwrap().unwrap().build_number, 1);
    assert_eq!(only_file(dir.path()), ["sde.zip"]);
}

#[test]
fn cancelling_leaves_old_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    std::fs::write(&dest, build_only_zip(1)).unwrap();
    let client = client(&release_server(2, vec![(2, build_only_zip(2))]));

    let err = client
        .download(2, &dest, |_, _| ControlFlow::Break(()))
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled));
    assert_eq!(local_build(&dest).unwrap().unwrap().build_number, 1);
    assert_eq!(only_file(dir.path()), ["sde.zip"]);
}

#[test]
fn overlapping_downloads_do_not_share_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    std::fs::write(&dest, build_only_zip(1)).unwrap();
    // Two servers, because each one answers one connection at a time.
    let first_client = client(&release_server(2, vec![(2, build_only_zip(2))]));
    let second_client = first_client
        .clone()
        .with_base_url(release_server(2, vec![(2, build_only_zip(2))]));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (cancel_tx, cancel_rx) = mpsc::channel();

    thread::scope(|scope| {
        let client = &first_client;
        let dest = &dest;
        let first = scope.spawn(move || {
            client.download(2, dest, |_, _| {
                ready_tx.send(()).unwrap();
                cancel_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                ControlFlow::Break(())
            })
        });
        ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let second = second_client.download(2, dest, |_, _| ControlFlow::Continue(()));
        cancel_tx.send(()).unwrap();
        assert!(matches!(first.join().unwrap(), Err(Error::Cancelled)));
        assert_eq!(second.unwrap().build_number, 2);
    });
    assert_eq!(local_build(&dest).unwrap().unwrap().build_number, 2);
    assert_eq!(only_file(dir.path()), [".sde.zip.lock", "sde.zip"]);
}

#[test]
fn oversized_download_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    let base = serve(|_| Response::ok("tiny").header("Content-Length", "900000000"));
    let err = client(&base)
        .download(2, &dest, |_, _| ControlFlow::Continue(()))
        .unwrap_err();
    assert!(
        matches!(&err, Error::LimitExceeded { resource, limit }
            if resource == "download" && *limit == MAX_DOWNLOAD_BYTES),
        "{err}"
    );
    assert!(only_file(dir.path()).is_empty());
}

#[test]
fn missing_folder_error_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("missing").join("sde.zip");
    let client = client(&release_server(2, vec![(2, build_only_zip(2))]));
    let err = client
        .download(2, &dest, |_, _| ControlFlow::Continue(()))
        .unwrap_err();
    let Error::Io(io) = &err else {
        panic!("{err:?}");
    };
    assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
    let message = err.to_string();
    assert!(message.starts_with("cannot create `"), "{message}");
    assert!(message.contains("missing"), "{message}");
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn destination_without_file_name_fails_before_any_request() {
    let requests = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&requests);
    let client = client(&serve(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        Response::status(404)
    }));
    let download = client.download(2, "..", |_, _| ControlFlow::Continue(()));
    for err in [download.unwrap_err(), client.update("..").unwrap_err()] {
        assert!(
            matches!(&err, Error::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput),
            "{err:?}"
        );
    }
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}

#[test]
fn missing_build_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let base = release_server(2, vec![]);
    let client = client(&base);
    let err = client
        .download(2, dir.path().join("sde.zip"), |_, _| {
            ControlFlow::Continue(())
        })
        .unwrap_err();
    assert!(matches!(
        err,
        Error::HttpStatus { url, status: 404 } if url == format!("{base}{}", zip_path(2))
    ));
    assert!(only_file(dir.path()).is_empty());
}

#[test]
fn update_downloads_only_when_behind() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    let client = client(&release_server(5, vec![(5, build_only_zip(5))]));

    assert_eq!(
        client.update(&dest).unwrap(),
        Update::Updated { from: None, to: 5 }
    );
    assert_eq!(client.update(&dest).unwrap(), Update::UpToDate(5));

    std::fs::write(&dest, build_only_zip(4)).unwrap();
    assert_eq!(
        client.update(&dest).unwrap(),
        Update::Updated {
            from: Some(4),
            to: 5
        }
    );

    std::fs::write(&dest, build_only_zip(6)).unwrap();
    assert_eq!(client.update(&dest).unwrap(), Update::UpToDate(6));
}

#[test]
fn local_build_reads_only_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sde.zip");
    assert_eq!(local_build(&path).unwrap(), None);
    std::fs::write(&path, build_only_zip(12)).unwrap();
    let build = local_build(&path).unwrap().unwrap();
    assert_eq!(build.build_number, 12);
    assert_eq!(build.release_date.as_deref(), Some("2026-10-02T11:08:57Z"));
    std::fs::write(&path, "garbage").unwrap();
    assert!(local_build(&path).is_err());
}

#[test]
fn overlapping_updates_recheck_the_build_before_replacement() {
    for (slow_build, fast_build) in [(2, 3), (2, 2), (3, 2)] {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("sde.zip");
        std::fs::write(&dest, build_only_zip(1)).unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let resume_rx = std::sync::Mutex::new(resume_rx);
        let slow_base = serve(move |req| {
            if req.path == "/latest.jsonl" {
                Response::ok(build_line(slow_build))
            } else {
                ready_tx.send(()).unwrap();
                resume_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                Response::ok(build_only_zip(slow_build))
            }
        });
        let slow = client(&slow_base);
        let fast = client(&release_server(
            fast_build,
            vec![(fast_build, build_only_zip(fast_build))],
        ));

        thread::scope(|scope| {
            let dest = &dest;
            let delayed = scope.spawn(move || slow.update(dest));
            ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            assert_eq!(
                fast.update(dest).unwrap(),
                Update::Updated {
                    from: Some(1),
                    to: fast_build
                }
            );
            resume_tx.send(()).unwrap();
            let expected = if fast_build >= slow_build {
                Update::UpToDate(fast_build)
            } else {
                Update::Updated {
                    from: Some(fast_build),
                    to: slow_build,
                }
            };
            assert_eq!(delayed.join().unwrap().unwrap(), expected);
        });
        assert_eq!(
            local_build(&dest).unwrap().unwrap().build_number,
            slow_build.max(fast_build)
        );
        assert_eq!(only_file(dir.path()), [".sde.zip.lock", "sde.zip"]);
    }
}

#[test]
fn explicit_download_can_replace_a_newer_build() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    std::fs::write(&dest, build_only_zip(3)).unwrap();
    let client = client(&release_server(2, vec![(2, build_only_zip(2))]));
    client
        .download(2, &dest, |_, _| ControlFlow::Continue(()))
        .unwrap();
    assert_eq!(local_build(&dest).unwrap().unwrap().build_number, 2);
}

#[test]
fn oversized_metadata_preserves_the_existing_archive() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("sde.zip");
    let original = build_only_zip(1);
    std::fs::write(&dest, &original).unwrap();
    let text = format!(
        "{}{}",
        build_line(2),
        " ".repeat(eve_sde::MAX_METADATA_BYTES as usize)
    );
    let mut bytes = common::zip_bytes(&[("_sde.jsonl", &text)]);
    common::understate_sizes(&mut bytes);
    let client = client(&release_server(2, vec![(2, bytes)]));
    let err = client
        .download(2, &dest, |_, _| ControlFlow::Continue(()))
        .unwrap_err();
    assert!(matches!(err, Error::LimitExceeded { resource, .. } if resource == "build metadata"));
    assert_eq!(std::fs::read(&dest).unwrap(), original);
    assert_eq!(only_file(dir.path()), ["sde.zip"]);
}

#[test]
fn latest_rejects_oversized_metadata() {
    let base = serve(|_| Response::ok(" ".repeat(eve_sde::MAX_METADATA_BYTES as usize + 1)));
    assert!(matches!(
        client(&base).latest(None),
        Err(Error::LimitExceeded { .. })
    ));
}
