use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use ureq::tls::{RootCerts, TlsConfig};
use ureq::{Agent, Proxy};

use boltay_translate::{Direction, Translator};

use crate::{EngineOptions, Error, Gigaam, Parakeet, Result, Transcriber, Vad};

pub struct ModelFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub struct Model {
    /// Also the asset name prefix in the release: `<id>__<file>`.
    pub id: &'static str,
    /// Subdirectory of the models root. Empty for files kept in the root itself,
    /// released without a prefix.
    pub dir: &'static str,
    /// Hugging Face repository, empty for a model only the project release has.
    pub hugging_face: &'static str,
    /// Subdirectory of the Hugging Face repository with the `.onnx` files, empty for the root.
    pub hugging_face_onnx: &'static str,
    pub files: &'static [ModelFile],
}

pub const GIGAAM: Model = Model {
    id: "gigaam-v3-e2e-rnnt",
    dir: "gigaam-v3-e2e-rnnt",
    hugging_face: "istupakov/gigaam-v3-onnx",
    hugging_face_onnx: "",
    files: &[
        ModelFile {
            name: "v3_e2e_rnnt_encoder.int8.onnx",
            size: 224_570_477,
            sha256: "4e0e076a6076cd110277e529b8ac8f32cd5297f7fbebad5341ae8ddb7d00817b",
        },
        ModelFile {
            name: "v3_e2e_rnnt_decoder.int8.onnx",
            size: 1_159_170,
            sha256: "89014e134865615b91e037157e46e389b1271e6072460efc010ea08e61e23146",
        },
        ModelFile {
            name: "v3_e2e_rnnt_joint.int8.onnx",
            size: 687_791,
            sha256: "ade116563dbf66e503b0994efab6b5861412743e52bf31c39fc3fffa3783d5d1",
        },
        ModelFile {
            name: "v3_e2e_rnnt_vocab.txt",
            size: 13_354,
            sha256: "39abae20e692998290c574e606f11a9edef2902a1995463fcff63d1490cf22b7",
        },
    ],
};

pub const PARAKEET: Model = Model {
    id: "parakeet-v3",
    dir: "parakeet-v3",
    hugging_face: "istupakov/parakeet-tdt-0.6b-v3-onnx",
    hugging_face_onnx: "",
    files: &[
        ModelFile {
            name: "encoder-model.int8.onnx",
            size: 652_183_999,
            sha256: "6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09",
        },
        ModelFile {
            name: "decoder_joint-model.int8.onnx",
            size: 18_202_004,
            sha256: "eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70",
        },
        ModelFile {
            name: "vocab.txt",
            size: 93_939,
            sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d",
        },
    ],
};

pub const SILERO_VAD: Model = Model {
    id: "silero-vad",
    dir: "",
    hugging_face: "istupakov/silero-vad-onnx",
    hugging_face_onnx: "",
    files: &[ModelFile {
        name: "silero_vad.onnx",
        size: 2_327_524,
        sha256: "1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3",
    }],
};

/// Opus-MT ru-en (Helsinki-NLP), exported to ONNX by Xenova.
pub const OPUS_MT: Model = Model {
    id: "opus-mt-ru-en",
    dir: "opus-mt-ru-en",
    hugging_face: "Xenova/opus-mt-ru-en",
    hugging_face_onnx: "onnx",
    files: &[
        ModelFile {
            name: "encoder_model_quantized.onnx",
            size: 51_628_446,
            sha256: "c10df0466540534bb70c4402d539c349b8bfa9968184cad89f51408cf13e7eb7",
        },
        ModelFile {
            name: "decoder_model_merged_quantized.onnx",
            size: 58_931_576,
            sha256: "25e7ff97b201bce64ab9c39749761d657b70b8602bcbc1ce01c67f713e90ffdf",
        },
        ModelFile {
            name: "tokenizer.json",
            size: 7_205_388,
            sha256: "981cd3d9fef6bb4dda8082afda716b65deb6717cb3ef35ac0b57002c09dec2bb",
        },
        ModelFile {
            name: "generation_config.json",
            size: 293,
            sha256: "9bcb507fa6117b07870df6627294b878b0710a55bc3abc9cf0db980e0c48fe2c",
        },
    ],
};

/// opus-mt-tc-big-zle-en (Helsinki-NLP), int8 ONNX export by TigreGotico. About five times
/// the size of [`OPUS_MT`].
pub const OPUS_MT_BIG: Model = Model {
    id: "opus-mt-tc-big-zle-en",
    dir: "opus-mt-tc-big-zle-en",
    hugging_face: "TigreGotico/opus-mt-tc-big-zle-en-onnx",
    hugging_face_onnx: "int8",
    files: &[
        ModelFile {
            name: "encoder_model.onnx",
            size: 139_490_280,
            sha256: "fde62b663c3bf7ffe62e761b540e10cc4870b7b40ee5545b8df01320140a0a2e",
        },
        ModelFile {
            name: "decoder_model.onnx",
            size: 227_740_288,
            sha256: "c45b1075a7a18e471cb03dde762bc1cd83c277f4f0a2f452411966f9ab866d32",
        },
        ModelFile {
            name: "decoder_with_past_model.onnx",
            size: 215_066_580,
            sha256: "fb8cdaf2d875bbe690400d077ac10686610f232b3964dad316e51f7341ca9d8a",
        },
        ModelFile {
            name: "source.spm",
            size: 1_016_851,
            sha256: "a982dbb9362861151e36b0db1595b324cd1ce09acf46ce1f4d6d624e11c5807f",
        },
        ModelFile {
            name: "vocab.json",
            size: 2_492_486,
            sha256: "92148daf001ba378588442dfcdc7c4529210f2f28635ebc092faecf508190680",
        },
        ModelFile {
            name: "generation_config.json",
            size: 296,
            sha256: "e10b4d8b214d71ef17d883e6fc93617dad95e612f4a6d8473bfd0b3ad2fa1792",
        },
    ],
};

/// Opus-MT en-ru (Helsinki-NLP), exported to ONNX by Xenova. The source SentencePiece
/// model instead of `tokenizer.json`, as `MarianTokenizer` encodes.
pub const OPUS_MT_EN_RU: Model = Model {
    id: "opus-mt-en-ru",
    dir: "opus-mt-en-ru",
    hugging_face: "Xenova/opus-mt-en-ru",
    hugging_face_onnx: "onnx",
    files: &[
        ModelFile {
            name: "encoder_model_quantized.onnx",
            size: 51_628_446,
            sha256: "97f28d8295d231b7da721ded06fa8c034787df2122cd8f85b904abdc4d15bd53",
        },
        ModelFile {
            name: "decoder_model_merged_quantized.onnx",
            size: 58_931_576,
            sha256: "7efefcd793a5663bc204224a92a123524f3dfe2ffad59e5f73d6734d1b11ad04",
        },
        ModelFile {
            name: "source.spm",
            size: 802_781,
            sha256: "16bebef1389a0b8ab452772c4e35b9e605e5713f8ac7baa71ca701394eaa086d",
        },
        ModelFile {
            name: "vocab.json",
            size: 2_726_796,
            sha256: "5cf0d95d930d8d3e783c9e2f46a72f08b43a18060dab4ddefbcb66a733efedcb",
        },
        ModelFile {
            name: "generation_config.json",
            size: 293,
            sha256: "885345001ac4f978b5e295a04eb6bd6c346ebc7381e30d5b3dd927b178d8a5ef",
        },
    ],
};

/// opus-mt-tc-big-en-zle (Helsinki-NLP), int8 ONNX export by TigreGotico. Into several East
/// Slavic languages, Russian picked with a target token.
pub const OPUS_MT_EN_RU_BIG: Model = Model {
    id: "opus-mt-tc-big-en-zle",
    dir: "opus-mt-tc-big-en-zle",
    hugging_face: "TigreGotico/opus-mt-tc-big-en-zle-onnx",
    hugging_face_onnx: "int8",
    files: &[
        ModelFile {
            name: "encoder_model.onnx",
            size: 140_063_720,
            sha256: "1361c4babcffa5245986d27a2e67f075132f95c2453b6c56fec1216d6c4d5271",
        },
        ModelFile {
            name: "decoder_model.onnx",
            size: 228_889_401,
            sha256: "9138d3b26a755bf685cb3424b8c9e739ca8c7306e85fc708edf87ca801724cdf",
        },
        ModelFile {
            name: "decoder_with_past_model.onnx",
            size: 216_215_690,
            sha256: "e01dfab4696d2385cde8b66b8a1c15594490df32ea099c77133861f9ef008903",
        },
        ModelFile {
            name: "source.spm",
            size: 802_747,
            sha256: "3612abfe04bf08344ba91115f0e15e228a7a15a621ea856bfd548097dbaeb43c",
        },
        ModelFile {
            name: "vocab.json",
            size: 2_510_527,
            sha256: "41dbdff4a0b5a6ab125715c3342c5ce6516e93ffd49608813240403f036c5efb",
        },
        ModelFile {
            name: "generation_config.json",
            size: 296,
            sha256: "173a87cd61329cb69fd7047dfa5fff49418892fbbf04382b131dfc5fcfba9e02",
        },
    ],
};

pub const MODELS: [&Model; 7] = [
    &GIGAAM,
    &PARAKEET,
    &SILERO_VAD,
    &OPUS_MT,
    &OPUS_MT_BIG,
    &OPUS_MT_EN_RU,
    &OPUS_MT_EN_RU_BIG,
];

impl Model {
    pub fn by_id(id: &str) -> Option<&'static Model> {
        MODELS.into_iter().find(|m| m.id == id)
    }

    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    fn hugging_face_path(&self, file: &ModelFile) -> String {
        if file.name.ends_with(".onnx") && !self.hugging_face_onnx.is_empty() {
            format!("{}/{}", self.hugging_face_onnx, file.name)
        } else {
            file.name.to_string()
        }
    }

    fn asset(&self, file: &ModelFile) -> String {
        if self.dir.is_empty() {
            file.name.to_string()
        } else {
            format!("{}__{}", self.id, file.name)
        }
    }
}

/// The model for a translation direction, the base one or the big one.
pub fn translation_model(direction: Direction, big: bool) -> &'static Model {
    match (direction, big) {
        (Direction::RuEn, false) => &OPUS_MT,
        (Direction::RuEn, true) => &OPUS_MT_BIG,
        (Direction::EnRu, false) => &OPUS_MT_EN_RU,
        (Direction::EnRu, true) => &OPUS_MT_EN_RU_BIG,
    }
}

/// The dictation language picked in settings, each served by its own engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Russian,
    Other,
}

impl Language {
    pub fn model(self) -> &'static Model {
        match self {
            Language::Russian => &GIGAAM,
            Language::Other => &PARAKEET,
        }
    }
}

const REPO: &str = "relaxcorp/boltay";
const TAG: &str = "models-v1";

/// Where model files come from, tried in order.
pub enum Source {
    /// Direct links to release assets.
    Release {
        base: String,
    },
    /// Release assets through the GitHub API, with `GITHUB_TOKEN` when set: another host,
    /// for when the direct links are blocked.
    GithubApi {
        release: String,
        token: Option<String>,
    },
    HuggingFace {
        base: String,
    },
}

impl Source {
    /// The project release, first directly and then through the API with `GITHUB_TOKEN`
    /// if set, then Hugging Face.
    pub fn defaults() -> Vec<Source> {
        vec![
            Source::Release {
                base: format!("https://github.com/{REPO}/releases/download/{TAG}"),
            },
            Source::GithubApi {
                release: format!("https://api.github.com/repos/{REPO}/releases/tags/{TAG}"),
                token: std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()),
            },
            Source::HuggingFace {
                base: "https://huggingface.co".into(),
            },
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub file: &'static str,
    /// Bytes of the whole model on disk, including files downloaded earlier.
    pub done: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Missing,
    /// Some files or parts of them are on disk, `fetch` continues from there.
    Partial(u64),
    Ready,
}

/// Downloaded models under one root directory.
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `Boltay/models` in the per-user local data directory of the OS.
    pub fn default_root() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("Boltay").join("models"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir(&self, model: &Model) -> PathBuf {
        self.root.join(model.dir)
    }

    /// Judged by file sizes: a file only gets its final name after its checksum matched.
    pub fn status(&self, model: &Model) -> Status {
        let dir = self.dir(model);
        let mut done = 0;
        let mut complete = true;
        for file in model.files {
            let size = len(&dir.join(file.name));
            if size == Some(file.size) {
                done += file.size;
            } else {
                complete = false;
                done += len(&part_path(&dir, file)).unwrap_or(0).min(file.size);
            }
        }
        match (complete, done) {
            (true, _) => Status::Ready,
            (false, 0) => Status::Missing,
            (false, done) => Status::Partial(done),
        }
    }

    /// Downloads whatever is missing, resuming partial files. Each file is checked against
    /// its sha256 before it gets its final name. Setting `cancel` stops the download and
    /// keeps the partial file for the next attempt.
    pub fn fetch(
        &self,
        model: &Model,
        sources: &[Source],
        mut progress: impl FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<()> {
        let dir = self.dir(model);
        fs::create_dir_all(&dir).map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        let mut downloader = Downloader {
            agent: agent(Proxy::try_from_env()),
            direct: agent(None),
            assets: None,
            total: model.size(),
            done: 0,
        };
        for file in model.files {
            let path = dir.join(file.name);
            if len(&path) == Some(file.size) {
                downloader.done += file.size;
                progress(Progress {
                    file: file.name,
                    done: downloader.done,
                    total: downloader.total,
                });
                continue;
            }
            downloader.fetch_file(model, file, &path, sources, &mut progress, cancel)?;
        }
        Ok(())
    }

    pub fn engine(
        &self,
        language: Language,
        options: EngineOptions,
    ) -> Result<Box<dyn Transcriber>> {
        let model = language.model();
        self.ensure_ready(model)?;
        Ok(match language {
            Language::Russian => Box::new(Gigaam::load(&self.dir(model), options)?),
            Language::Other => Box::new(Parakeet::load(&self.dir(model), options)?),
        })
    }

    /// Loads one of the translation models.
    pub fn translator(&self, model: &Model) -> Result<Translator> {
        self.ensure_ready(model)?;
        let direction = if [OPUS_MT_EN_RU.id, OPUS_MT_EN_RU_BIG.id].contains(&model.id) {
            Direction::EnRu
        } else {
            Direction::RuEn
        };
        Ok(Translator::load(&self.dir(model), direction)?)
    }

    pub fn vad(&self) -> Result<Vad> {
        self.ensure_ready(&SILERO_VAD)?;
        Vad::load(&self.dir(&SILERO_VAD).join(SILERO_VAD.files[0].name))
    }

    fn ensure_ready(&self, model: &Model) -> Result<()> {
        if self.status(model) == Status::Ready {
            Ok(())
        } else {
            Err(Error::Model(format!(
                "model {} is not downloaded to {}",
                model.id,
                self.root.display()
            )))
        }
    }
}

fn len(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.len())
}

fn part_path(dir: &Path, file: &ModelFile) -> PathBuf {
    dir.join(format!("{}.part", file.name))
}

fn agent(proxy: Option<Proxy>) -> Agent {
    Agent::config_builder()
        .proxy(proxy)
        // The OS trust store, so downloads work behind corporate TLS proxies too.
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .user_agent(concat!("boltay/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    url: String,
}

/// True for localhost and loopback addresses, which never go through a proxy.
fn is_loopback(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    };
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

struct Downloader {
    /// Honors the proxy settings of the environment.
    agent: Agent,
    /// For loopback hosts, an HTTP(S)_PROXY without NO_PROXY would send them away.
    direct: Agent,
    /// Asset list of the release, fetched once when the API source is first used.
    assets: Option<Vec<Asset>>,
    total: u64,
    done: u64,
}

impl Downloader {
    fn agent_for(&self, url: &str) -> &Agent {
        if is_loopback(url) {
            &self.direct
        } else {
            &self.agent
        }
    }

    fn fetch_file(
        &mut self,
        model: &Model,
        file: &ModelFile,
        path: &Path,
        sources: &[Source],
        progress: &mut impl FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<()> {
        let part = part_path(path.parent().unwrap_or(Path::new(".")), file);
        let mut failures = Vec::new();
        for source in sources {
            if matches!(source, Source::HuggingFace { .. }) && model.hugging_face.is_empty() {
                continue;
            }
            let result = self
                .download(source, model, file, &part, progress, cancel)
                .and_then(|()| verify(file, &part));
            match result {
                Ok(()) => {
                    // Saved by a mirror: the sources that failed still go to the log.
                    for e in &failures {
                        log::warn!("{e}");
                    }
                    fs::rename(&part, path).map_err(|source| Error::Io {
                        path: path.into(),
                        source,
                    })?;
                    self.done += file.size;
                    return Ok(());
                }
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(e) => failures.push(e),
            }
        }
        // A single failure is returned as is; with mirrors, the first source's reason
        // (often the real one, like a 404 on a missing asset) must not be lost.
        match failures.len() {
            0 => Err(Error::Model(format!("no sources for {}", file.name))),
            1 => Err(failures.remove(0)),
            _ => Err(Error::Sources {
                file: file.name.into(),
                failures,
            }),
        }
    }

    fn download(
        &mut self,
        source: &Source,
        model: &Model,
        file: &ModelFile,
        part: &Path,
        progress: &mut impl FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<()> {
        let io_error = |source| Error::Io {
            path: part.into(),
            source,
        };
        let mut have = len(part).unwrap_or(0);
        if have > file.size {
            have = 0;
        }
        if have == file.size {
            return Ok(());
        }

        let (url, headers) = self.request(source, model, file)?;
        let mut request = self.agent_for(&url).get(&url);
        for (name, value) in &headers {
            request = request.header(*name, value);
        }
        if have > 0 {
            request = request.header("Range", format!("bytes={have}-"));
        }
        let failed = |reason: String| Error::Download {
            url: url.clone(),
            reason,
        };
        let response = request.call().map_err(|e| failed(e.to_string()))?;

        let resumed = match response.status().as_u16() {
            206 => {
                let range = response
                    .headers()
                    .get("content-range")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default();
                if !range.starts_with(&format!("bytes {have}-")) {
                    return Err(failed(format!("unexpected range {range:?}")));
                }
                true
            }
            200 => false,
            status => return Err(failed(format!("HTTP {status}"))),
        };
        if !resumed {
            have = 0;
        }

        let mut out = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(!resumed)
            .open(part)
            .map_err(io_error)?;
        out.seek(SeekFrom::Start(have)).map_err(io_error)?;

        let mut body = response.into_body().into_reader();
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let n = match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(failed(e.to_string())),
            };
            let n = n.min((file.size - have) as usize);
            out.write_all(&buf[..n]).map_err(io_error)?;
            have += n as u64;
            progress(Progress {
                file: file.name,
                done: self.done + have,
                total: self.total,
            });
            if have == file.size {
                break;
            }
        }
        out.sync_all().map_err(io_error)?;
        if have < file.size {
            return Err(failed(format!(
                "connection closed at {have} of {} bytes",
                file.size
            )));
        }
        Ok(())
    }

    fn request(
        &mut self,
        source: &Source,
        model: &Model,
        file: &ModelFile,
    ) -> Result<(String, Vec<(&'static str, String)>)> {
        Ok(match source {
            Source::Release { base } => (format!("{base}/{}", model.asset(file)), Vec::new()),
            Source::HuggingFace { base } => (
                format!(
                    "{base}/{}/resolve/main/{}",
                    model.hugging_face,
                    model.hugging_face_path(file)
                ),
                Vec::new(),
            ),
            Source::GithubApi { release, token } => {
                let mut headers = vec![("Accept", "application/octet-stream".to_string())];
                if let Some(token) = token {
                    headers.push(("Authorization", format!("Bearer {token}")));
                }
                if self.assets.is_none() {
                    let mut request = self.agent_for(release).get(release);
                    if let Some(token) = token {
                        request = request.header("Authorization", format!("Bearer {token}"));
                    }
                    let failed = |reason: String| Error::Download {
                        url: release.clone(),
                        reason,
                    };
                    let mut response = request
                        .header("Accept", "application/vnd.github+json")
                        .call()
                        .map_err(|e| failed(e.to_string()))?;
                    if response.status() != 200 {
                        return Err(failed(format!("HTTP {}", response.status().as_u16())));
                    }
                    let listing: Release = response
                        .body_mut()
                        .read_json()
                        .map_err(|e| failed(e.to_string()))?;
                    self.assets = Some(listing.assets);
                }
                let name = model.asset(file);
                let asset = self
                    .assets
                    .iter()
                    .flatten()
                    .find(|a| a.name == name)
                    .ok_or_else(|| Error::Model(format!("{release}: no asset {name}")))?;
                (asset.url.clone(), headers)
            }
        })
    }
}

/// Checks the finished part file, a corrupt one is removed so the next attempt starts over.
fn verify(file: &ModelFile, part: &Path) -> Result<()> {
    let io_error = |source| Error::Io {
        path: part.into(),
        source,
    };
    let mut hasher = Sha256::new();
    let mut input = File::open(part).map_err(io_error)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io_error(e)),
        }
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex == file.sha256 {
        Ok(())
    } else {
        let _ = fs::remove_file(part);
        Err(Error::Checksum(file.name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts() {
        for url in [
            "http://127.0.0.1:8080/a",
            "http://127.8.9.1/",
            "http://localhost:1234/x",
            "https://LOCALHOST",
            "http://[::1]:80/a",
            "http://user:pw@127.0.0.1:5/",
        ] {
            assert!(is_loopback(url), "{url}");
        }
        for url in [
            "https://github.com/a",
            "https://api.github.com:443/x",
            "http://10.0.0.1/",
            "http://[2001:db8::1]/",
            "http://localhost.example.com/",
            "http://127.0.0.1.example.com/",
        ] {
            assert!(!is_loopback(url), "{url}");
        }
    }
}
