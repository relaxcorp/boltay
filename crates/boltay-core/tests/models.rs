use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use boltay_core::{Error, Model, ModelFile, Progress, Source, Status, Store};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct Request {
    path: String,
    headers: HashMap<String, String>,
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// Close the connection after this many body bytes, with the full length announced.
    cut: Option<usize>,
}

impl Response {
    fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            cut: None,
        }
    }

    /// Serves `body`, honoring `Range: bytes=N-` unless `ranges` is off.
    fn file(request: &Request, body: &[u8], ranges: bool) -> Self {
        let start = request
            .headers
            .get("range")
            .filter(|_| ranges)
            .and_then(|r| r.strip_prefix("bytes="))
            .and_then(|r| r.strip_suffix('-'))
            .and_then(|n| n.parse::<usize>().ok());
        match start {
            Some(start) => Self {
                status: 206,
                headers: vec![(
                    "Content-Range".into(),
                    format!("bytes {start}-{}/{}", body.len() - 1, body.len()),
                )],
                body: body[start..].to_vec(),
                cut: None,
            },
            None => Self {
                status: 200,
                headers: Vec::new(),
                body: body.to_vec(),
                cut: None,
            },
        }
    }
}

type Handler = dyn Fn(&Request) -> Response + Send + Sync;

/// A minimal HTTP/1.1 server, one connection at a time, that records every request.
struct Server {
    base: String,
    log: Arc<Mutex<Vec<Request>>>,
}

impl Server {
    fn start(handler: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<Handler> = Arc::new(handler);
        let requests = log.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if let Some(request) = read_request(&stream) {
                    requests.lock().unwrap().push(request.clone());
                    write_response(stream, handler(&request));
                }
            }
        });
        Self { base, log }
    }

    fn requests(&self) -> Vec<Request> {
        self.log.lock().unwrap().clone()
    }
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let path = line.split_whitespace().nth(1)?.to_string();
    let mut headers = HashMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).ok()?;
        let Some((name, value)) = line.trim_end().split_once(':') else {
            break;
        };
        headers.insert(name.trim().to_lowercase(), value.trim().to_string());
    }
    Some(Request { path, headers })
}

fn write_response(mut stream: TcpStream, response: Response) {
    let mut head = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        response.body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let body = &response.body[..response.cut.unwrap_or(response.body.len())];
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn content(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn sha256(data: &[u8]) -> &'static str {
    let digest = Sha256::digest(data);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Box::leak(hex.into_boxed_str())
}

/// A two-file model with the given contents.
fn model(a: &[u8], b: &[u8]) -> &'static Model {
    let files = vec![
        ModelFile {
            name: "encoder.onnx",
            size: a.len() as u64,
            sha256: sha256(a),
        },
        ModelFile {
            name: "vocab.txt",
            size: b.len() as u64,
            sha256: sha256(b),
        },
    ];
    Box::leak(Box::new(Model {
        id: "test-model",
        dir: "test-model",
        hugging_face: "someone/test-model-onnx",
        hugging_face_onnx: "onnx",
        files: Box::leak(files.into_boxed_slice()),
    }))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("boltay-models-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn release(server: &Server) -> Source {
    Source::Release {
        base: server.base.clone(),
    }
}

fn fetch(store: &Store, model: &Model, sources: &[Source]) -> Result<Vec<Progress>, Error> {
    let mut seen = Vec::new();
    store.fetch(model, sources, |p| seen.push(p), &AtomicBool::new(false))?;
    Ok(seen)
}

#[test]
fn downloads_and_verifies() {
    let (a, b) = (content(300_000, 1), content(1_000, 2));
    let model = model(&a, &b);
    let (sa, sb) = (a.clone(), b.clone());
    let server = Server::start(move |r| match r.path.as_str() {
        "/test-model__encoder.onnx" => Response::file(r, &sa, true),
        "/test-model__vocab.txt" => Response::file(r, &sb, true),
        _ => Response::status(404),
    });
    let store = Store::new(temp_dir("ok"));
    assert_eq!(store.status(model), Status::Missing);

    let progress = fetch(&store, model, &[release(&server)]).unwrap();
    assert_eq!(store.status(model), Status::Ready);
    let dir = store.dir(model);
    assert_eq!(fs::read(dir.join("encoder.onnx")).unwrap(), a);
    assert_eq!(fs::read(dir.join("vocab.txt")).unwrap(), b);
    assert!(!dir.join("encoder.onnx.part").exists());
    assert!(progress.windows(2).all(|w| w[0].done <= w[1].done));
    let last = progress.last().unwrap();
    assert_eq!((last.done, last.total), (301_000, 301_000));

    // Everything is in place, nothing is requested again.
    let before = server.requests().len();
    fetch(&store, model, &[release(&server)]).unwrap();
    assert_eq!(server.requests().len(), before);
}

#[test]
fn resumes_after_dropped_connection() {
    let (a, b) = (content(200_000, 3), content(10, 4));
    let model = model(&a, &b);
    let first = AtomicBool::new(true);
    let (sa, sb) = (a.clone(), b.clone());
    let server = Server::start(move |r| {
        let body = if r.path.ends_with("encoder.onnx") {
            &sa
        } else {
            &sb
        };
        let mut response = Response::file(r, body, true);
        if r.path.ends_with("encoder.onnx") && first.swap(false, Ordering::SeqCst) {
            response.cut = Some(70_000);
        }
        response
    });
    let store = Store::new(temp_dir("resume"));

    let err = fetch(&store, model, &[release(&server)]).unwrap_err();
    assert!(matches!(err, Error::Download { .. }), "{err}");
    assert_eq!(store.status(model), Status::Partial(70_000));

    fetch(&store, model, &[release(&server)]).unwrap();
    assert_eq!(fs::read(store.dir(model).join("encoder.onnx")).unwrap(), a);
    let ranges: Vec<_> = server
        .requests()
        .into_iter()
        .filter_map(|r| r.headers.get("range").cloned())
        .collect();
    assert_eq!(ranges, ["bytes=70000-"]);
}

#[test]
fn server_without_ranges_restarts_the_file() {
    let (a, b) = (content(50_000, 5), content(10, 6));
    let model = model(&a, &b);
    let store = Store::new(temp_dir("norange"));
    let dir = store.dir(model);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("encoder.onnx.part"), &a[..20_000]).unwrap();

    let (sa, sb) = (a.clone(), b.clone());
    let server = Server::start(move |r| {
        let body = if r.path.ends_with("encoder.onnx") {
            &sa
        } else {
            &sb
        };
        Response::file(r, body, false)
    });
    fetch(&store, model, &[release(&server)]).unwrap();
    assert_eq!(fs::read(dir.join("encoder.onnx")).unwrap(), a);
}

#[test]
fn corrupt_file_is_rejected_and_removed() {
    let (a, b) = (content(10_000, 7), content(10, 8));
    let model = model(&a, &b);
    let mut corrupt = a.clone();
    corrupt[5_000] ^= 1;
    let server = Server::start(move |r| Response::file(r, &corrupt, true));
    let store = Store::new(temp_dir("corrupt"));

    let err = fetch(&store, model, &[release(&server)]).unwrap_err();
    assert!(
        matches!(err, Error::Checksum(ref f) if f == "encoder.onnx"),
        "{err}"
    );
    let dir = store.dir(model);
    assert!(!dir.join("encoder.onnx").exists());
    assert!(!dir.join("encoder.onnx.part").exists());
    assert_eq!(store.status(model), Status::Missing);
}

#[test]
fn falls_back_to_hugging_face() {
    let (a, b) = (content(10_000, 9), content(10, 10));
    let model = model(&a, &b);
    let release_server = Server::start(|_| Response::status(404));
    let (sa, sb) = (a.clone(), b.clone());
    let hf = Server::start(move |r| match r.path.as_str() {
        "/someone/test-model-onnx/resolve/main/onnx/encoder.onnx" => Response::file(r, &sa, true),
        "/someone/test-model-onnx/resolve/main/vocab.txt" => Response::file(r, &sb, true),
        _ => Response::status(404),
    });
    let store = Store::new(temp_dir("fallback"));
    let sources = [
        release(&release_server),
        Source::HuggingFace {
            base: hf.base.clone(),
        },
    ];
    fetch(&store, model, &sources).unwrap();
    assert_eq!(store.status(model), Status::Ready);
    assert_eq!(release_server.requests().len(), 2);
}

#[test]
fn reports_every_failed_source() {
    let (a, b) = (content(100, 13), content(10, 14));
    let model = model(&a, &b);
    let release_server = Server::start(|_| Response::status(404));
    let hf = Server::start(|_| Response::status(503));
    let store = Store::new(temp_dir("allfail"));
    let sources = [
        release(&release_server),
        Source::HuggingFace {
            base: hf.base.clone(),
        },
    ];
    let err = fetch(&store, model, &sources).unwrap_err();
    let Error::Sources { file, failures } = &err else {
        panic!("{err}");
    };
    assert_eq!(file, "encoder.onnx");
    assert_eq!(failures.len(), 2);
    let text = err.to_string();
    assert!(
        text.contains(&format!(
            "{}/test-model__encoder.onnx: HTTP 404",
            release_server.base
        )),
        "{text}"
    );
    assert!(text.contains("HTTP 503"), "{text}");
}

#[test]
fn github_api_resolves_assets_with_token() {
    let (a, b) = (content(10_000, 11), content(10, 12));
    let model = model(&a, &b);
    let base = Arc::new(Mutex::new(String::new()));
    let (sa, sb, url) = (a.clone(), b.clone(), base.clone());
    let server = Server::start(move |r| {
        if r.headers.get("authorization").map(String::as_str) != Some("Bearer secret") {
            return Response::status(404);
        }
        let base = url.lock().unwrap().clone();
        match r.path.as_str() {
            "/release" => Response {
                status: 200,
                headers: Vec::new(),
                body: format!(
                    r#"{{"assets": [
                        {{"name": "test-model__encoder.onnx", "url": "{base}/assets/1", "id": 1}},
                        {{"name": "test-model__vocab.txt", "url": "{base}/assets/2", "id": 2}}
                    ]}}"#
                )
                .into_bytes(),
                cut: None,
            },
            "/assets/1" | "/assets/2"
                if r.headers.get("accept").map(String::as_str)
                    == Some("application/octet-stream") =>
            {
                Response::file(r, if r.path.ends_with('1') { &sa } else { &sb }, true)
            }
            _ => Response::status(404),
        }
    });
    *base.lock().unwrap() = server.base.clone();

    let store = Store::new(temp_dir("api"));
    let sources = [Source::GithubApi {
        release: format!("{}/release", server.base),
        token: Some("secret".into()),
    }];
    fetch(&store, model, &sources).unwrap();
    assert_eq!(store.status(model), Status::Ready);
    // The release listing is fetched once for both files.
    let paths: Vec<_> = server.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(paths, ["/release", "/assets/1", "/assets/2"]);
}

#[test]
fn cancel_keeps_the_partial_file() {
    let (a, b) = (content(3_000_000, 13), content(10, 14));
    let model = model(&a, &b);
    let sa = a.clone();
    let server = Server::start(move |r| Response::file(r, &sa, true));
    let store = Store::new(temp_dir("cancel"));

    let cancel = AtomicBool::new(false);
    let err = store
        .fetch(
            model,
            &[release(&server)],
            |_| cancel.store(true, Ordering::Relaxed),
            &cancel,
        )
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled), "{err}");
    let Status::Partial(done) = store.status(model) else {
        panic!("{:?}", store.status(model));
    };
    assert!(done > 0 && done < 3_000_000, "{done}");
    assert!(!store.dir(model).join("encoder.onnx").exists());
}

#[test]
fn registry_lists_known_models() {
    let ids: Vec<_> = boltay_core::MODELS.iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        [
            "gigaam-v3-e2e-rnnt",
            "parakeet-v3",
            "silero-vad",
            "opus-mt-ru-en",
            "opus-mt-tc-big-zle-en",
            "opus-mt-en-ru",
            "opus-mt-tc-big-en-zle"
        ]
    );
    assert!(Model::by_id("parakeet-v3").is_some());
    assert!(Model::by_id("whisper").is_none());
    for model in boltay_core::MODELS {
        for file in model.files {
            assert_eq!(file.sha256.len(), 64, "{}", file.name);
        }
    }
}
