mod memory;

use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use boltay_core::audio::{self, SAMPLE_RATE};
use boltay_core::{
    translation_model, Detailed, Direction, EngineOptions, FileJob, FileLanguage, Language, Model,
    Progress, Recognizer, Source, SpanKind, Status, Store, Transcriber, Translator, MODELS,
};
use boltay_text::{Config, Profanity};
use clap::{Args, Parser, Subcommand, ValueEnum};

const AUDIO_EXTENSIONS: &[&str] = &["wav", "flac", "ogg", "oga", "opus", "mp3", "m4a"];

#[derive(Parser)]
#[command(name = "boltay", version, about = "Offline speech recognition")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribe an audio file (wav, flac, ogg vorbis or opus, mp3, m4a)
    Transcribe {
        file: PathBuf,
        #[command(flatten)]
        engine: EngineArgs,
        #[command(flatten)]
        text: TextArgs,
        /// Print the recognized text as is, without text processing
        #[arg(long)]
        raw: bool,
        /// Translate the result into this language unless it is already in it
        #[arg(long, value_enum)]
        to: Option<Target>,
        /// Translate into English with the large model
        #[arg(long, requires = "to")]
        big: bool,
        /// Start a new paragraph after a pause of this many seconds
        #[arg(long, value_name = "SECONDS", default_value_t = 2.0)]
        paragraph_pause: f32,
    },
    /// Translate Russian text to English and English to Russian, by the letters each line
    /// is written in; line by line from stdin if no text is given
    Translate {
        text: Option<String>,
        #[command(flatten)]
        store: StoreArgs,
        /// Print latency per line and peak memory to stderr
        #[arg(long)]
        timing: bool,
        /// Translate into English with the large model
        #[arg(long)]
        big: bool,
        /// Also list the doubtful and rare words and the spelled-out abbreviations
        #[arg(long)]
        detailed: bool,
    },
    /// Measure speed and memory on every audio file in a directory
    Bench {
        dir: PathBuf,
        #[command(flatten)]
        engine: EngineArgs,
        /// Runs per file, the median is reported
        #[arg(long, default_value_t = 3)]
        runs: usize,
    },
    /// Run text through the processing pipeline
    Clean {
        text: String,
        #[command(flatten)]
        options: TextArgs,
        /// Override the profanity filter of the config
        #[arg(long, value_enum)]
        profanity: Option<ProfanityArg>,
    },
    /// Manage downloaded models
    Models {
        #[command(subcommand)]
        command: ModelsCommand,
        #[command(flatten)]
        store: StoreArgs,
    },
}

#[derive(Subcommand)]
enum ModelsCommand {
    /// Show every model and whether it is downloaded
    List,
    /// Download a model, or `all`, resuming a previous attempt
    Fetch { model: String },
    /// Print where models are stored
    Path { model: Option<String> },
}

#[derive(Args)]
struct StoreArgs {
    /// Models directory, by default the one in the OS data directory
    #[arg(long, env = "BOLTAY_MODELS", global = true)]
    models: Option<PathBuf>,
}

impl StoreArgs {
    fn store(&self) -> Result<Store> {
        match self.models.clone().or_else(Store::default_root) {
            Some(root) => Ok(Store::new(root)),
            None => bail!("no data directory on this system, pass --models"),
        }
    }
}

#[derive(Args)]
struct EngineArgs {
    /// Russian goes to GigaAM, other languages to Parakeet; `auto` lets Parakeet hear the
    /// file first
    #[arg(long, value_enum, default_value_t = Lang::Ru)]
    lang: Lang,
    #[command(flatten)]
    store: StoreArgs,
    /// Pass the whole recording to the engine, without voice activity detection
    #[arg(long)]
    no_vad: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Lang {
    Auto,
    Ru,
    #[value(alias = "other")]
    En,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Target {
    Ru,
    En,
}

impl EngineArgs {
    fn load(&self) -> Result<Recognizer> {
        let store = self.store.store()?;
        let language = match self.lang {
            Lang::Ru => Language::Russian,
            Lang::En => Language::Other,
            Lang::Auto => bail!("pick the language, --lang ru or --lang en"),
        };
        Ok(Recognizer::new(
            load_engine(&store, language)?,
            self.vad(&store)?,
        ))
    }

    fn vad(&self, store: &Store) -> Result<Option<boltay_core::Vad>> {
        if self.no_vad {
            return Ok(None);
        }
        let vad = store
            .vad()
            .context("cannot load Silero VAD, download it with `boltay models fetch silero-vad`")?;
        Ok(Some(vad))
    }
}

fn load_engine(store: &Store, language: Language) -> Result<Box<dyn Transcriber>> {
    // Off by default: the cache doubles the disk space taken by the model.
    let options = EngineOptions {
        graph_cache: std::env::var("BOLTAY_ORT_CACHE").is_ok_and(|v| v == "1"),
    };
    store.engine(language, options).with_context(|| {
        let id = language.model().id;
        format!("cannot load {id}, download it with `boltay models fetch {id}`")
    })
}

#[derive(Args)]
struct TextArgs {
    /// Text processing settings, JSON; missing fields take defaults
    #[arg(long, value_name = "FILE")]
    text_config: Option<PathBuf>,
}

impl TextArgs {
    fn config(&self) -> Result<Config> {
        let Some(path) = &self.text_config else {
            return Ok(Config::default());
        };
        let json =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        serde_json::from_str(&json).with_context(|| format!("invalid {}", path.display()))
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum ProfanityArg {
    Keep,
    Mask,
    Remove,
    Soften,
}

impl From<ProfanityArg> for Profanity {
    fn from(p: ProfanityArg) -> Self {
        match p {
            ProfanityArg::Keep => Profanity::Keep,
            ProfanityArg::Mask => Profanity::Mask,
            ProfanityArg::Remove => Profanity::Remove,
            ProfanityArg::Soften => Profanity::Soften,
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Transcribe {
            file,
            engine,
            text,
            raw,
            to,
            big,
            paragraph_pause,
        } => {
            let config = if raw { None } else { Some(text.config()?) };
            let paragraph = Some(Duration::from_secs_f32(paragraph_pause));
            transcribe(&file, &engine, config, to, big, paragraph)?;
        }
        Command::Translate {
            text,
            store,
            timing,
            big,
            detailed,
        } => translate(text, &store.store()?, timing, big, detailed)?,
        Command::Bench { dir, engine, runs } => bench(&dir, &engine, runs.max(1))?,
        Command::Clean {
            text,
            options,
            profanity,
        } => {
            let mut config = options.config()?;
            if let Some(p) = profanity {
                config.profanity = p.into();
            }
            println!("{}", boltay_text::process(&config, &text));
        }
        Command::Models { command, store } => models(command, &store.store()?)?,
    }
    Ok(())
}

fn transcribe(
    file: &Path,
    args: &EngineArgs,
    config: Option<Config>,
    to: Option<Target>,
    big: bool,
    paragraph: Option<Duration>,
) -> Result<()> {
    let store = args.store.store()?;
    let vad = args.vad(&store)?;
    let language = match args.lang {
        Lang::Auto => FileLanguage::Auto,
        Lang::Ru => FileLanguage::Russian,
        Lang::En => FileLanguage::English,
    };
    // One engine at a time: both do not fit in the memory budget.
    let mut loaded: Option<(Language, Arc<dyn Transcriber>)> = None;
    let mut engine = |language: Language| -> boltay_core::Result<Arc<dyn Transcriber>> {
        if let Some((l, engine)) = &loaded {
            if *l == language {
                return Ok(engine.clone());
            }
        }
        loaded = None;
        let engine: Arc<dyn Transcriber> = Arc::from(
            load_engine(&store, language)
                .map_err(|e| boltay_core::Error::Model(format!("{e:#}")))?,
        );
        loaded = Some((language, engine.clone()));
        Ok(engine)
    };
    let tty = std::io::stderr().is_terminal();
    let mut progress = |share: f32| {
        if tty {
            eprint!("\r{:>3.0}%", share * 100.0);
        }
    };
    let transcript = boltay_core::transcribe_file(
        file,
        language,
        FileJob {
            vad: vad.as_ref(),
            engine: &mut engine,
            paragraph,
            progress: &mut progress,
            cancel: &AtomicBool::new(false),
        },
    )?;
    if tty {
        eprint!("\r");
    }
    drop(loaded);
    let russian = transcript.language == Language::Russian;
    eprintln!("language: {}", if russian { "ru" } else { "en" });

    let text = match config {
        Some(config) if russian => boltay_text::process(&config, &transcript.text),
        // The brand lists are how the Russian model misspells names, in English they would
        // only catch plain words.
        Some(config) => boltay_text::process(
            &Config {
                brands: Default::default(),
                ambiguous: Default::default(),
                ..config
            },
            &transcript.text,
        ),
        None => transcript.text,
    };
    let direction = match (to, russian) {
        (Some(Target::En), true) => Some(Direction::RuEn),
        (Some(Target::Ru), false) => Some(Direction::EnRu),
        _ => None,
    };
    match direction {
        Some(direction) => {
            let translator = load_translator(&store, direction, big)?;
            println!("{}", translator.translate_detailed(&text)?.text);
        }
        None => println!("{text}"),
    }
    Ok(())
}

fn load_translator(store: &Store, direction: Direction, big: bool) -> Result<Translator> {
    let model = translation_model(direction, big);
    store.translator(model).with_context(|| {
        let id = model.id;
        format!("cannot load {id}, download it with `boltay models fetch {id}`")
    })
}

/// Both directions, each loaded when a line first needs it.
struct Translators<'a> {
    store: &'a Store,
    big: bool,
    loaded: Vec<Translator>,
}

impl Translators<'_> {
    fn get(&mut self, direction: Direction) -> Result<&Translator> {
        let i = match self.loaded.iter().position(|t| t.direction() == direction) {
            Some(i) => i,
            None => {
                self.loaded
                    .push(load_translator(self.store, direction, self.big)?);
                self.loaded.len() - 1
            }
        };
        Ok(&self.loaded[i])
    }
}

fn translate(
    text: Option<String>,
    store: &Store,
    timing: bool,
    big: bool,
    detailed: bool,
) -> Result<()> {
    let lines: Vec<String> = match text {
        Some(text) => vec![text],
        None => std::io::stdin().lines().collect::<Result<_, _>>()?,
    };
    let lines: Vec<&String> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let mut translators = Translators {
        store,
        big,
        loaded: Vec::new(),
    };
    if timing {
        if let Some(first) = lines.first() {
            let direction = Direction::detect(first).unwrap_or(Direction::RuEn);
            let started = Instant::now();
            let translator = translators.get(direction)?;
            eprintln!("model load {:.0} ms", ms(started.elapsed()));
            let started = Instant::now();
            translator.translate_detailed(first)?;
            eprintln!("first call {:.0} ms", ms(started.elapsed()));
        }
    }
    let mut latencies = Vec::new();
    for line in lines {
        let Some(direction) = Direction::detect(line) else {
            println!("{line}");
            continue;
        };
        let translator = translators.get(direction)?;
        let started = Instant::now();
        let result = translator.translate_detailed(line)?;
        let latency = started.elapsed();
        println!("{}", result.text);
        if detailed {
            print_spans(line, &result);
        }
        if timing {
            eprintln!(
                "{:>6.0} ms  {:>2} words  {line}",
                ms(latency),
                line.split_whitespace().count()
            );
        }
        latencies.push(latency);
    }
    if timing && !latencies.is_empty() {
        latencies.sort();
        let total: Duration = latencies.iter().sum();
        eprintln!(
            "latency mean {:.0} ms, median {:.0} ms, max {:.0} ms",
            ms(total) / latencies.len() as f64,
            ms(latencies[latencies.len() / 2]),
            ms(latencies[latencies.len() - 1])
        );
        if let Some(bytes) = memory::peak() {
            eprintln!("peak memory {:.0} MB", mb(bytes));
        }
    }
    Ok(())
}

fn print_spans(original: &str, result: &Detailed) {
    for span in &result.spans {
        let kind = match span.kind {
            SpanKind::LowConfidence => "doubtful",
            SpanKind::Rare => "rare",
            SpanKind::Abbreviation => "abbreviation",
            SpanKind::Slang => "slang",
        };
        let text = span
            .translation
            .clone()
            .map(|r| &result.text[r])
            .or_else(|| span.original.clone().map(|r| &original[r]))
            .unwrap_or_default();
        match &span.note {
            Some(note) => println!("  {kind:<12} {text}: {note}"),
            None => println!("  {kind:<12} {text}"),
        }
    }
}

fn find_models(name: &str) -> Result<Vec<&'static Model>> {
    if name == "all" {
        return Ok(MODELS.to_vec());
    }
    match Model::by_id(name) {
        Some(model) => Ok(vec![model]),
        None => {
            let known: Vec<_> = MODELS.iter().map(|m| m.id).collect();
            bail!("unknown model {name}, known: {}", known.join(", "))
        }
    }
}

fn models(command: ModelsCommand, store: &Store) -> Result<()> {
    match command {
        ModelsCommand::List => {
            for model in MODELS {
                let status = match store.status(model) {
                    Status::Ready => "ready".to_string(),
                    Status::Missing => "missing".to_string(),
                    Status::Partial(done) => format!("partial {:.0}%", percent(done, model.size())),
                };
                println!("{:<20} {:>7.1} MB  {status}", model.id, mb(model.size()));
            }
        }
        ModelsCommand::Fetch { model } => {
            let sources = Source::defaults();
            for model in find_models(&model)? {
                // A terminal gets one line updated in place, a log gets a line per 10%.
                let terminal = std::io::stderr().is_terminal();
                let step = if terminal { 1 } else { 10 };
                let mut shown = None;
                let report = |p: Progress| {
                    let pct = percent(p.done, p.total) as u32 / step * step;
                    if shown == Some(pct) {
                        return;
                    }
                    shown = Some(pct);
                    let line = format!(
                        "{} {pct:>3}%  {:.1} / {:.1} MB",
                        model.id,
                        mb(p.done),
                        mb(p.total)
                    );
                    if terminal {
                        eprint!("\r{line}");
                        let _ = std::io::stderr().flush();
                    } else {
                        eprintln!("{line}");
                    }
                };
                let result = store.fetch(model, &sources, report, &AtomicBool::new(false));
                if terminal {
                    eprintln!();
                }
                result.with_context(|| format!("cannot download {}", model.id))?;
            }
        }
        ModelsCommand::Path { model } => match model {
            Some(name) => {
                for model in find_models(&name)? {
                    println!("{}", store.dir(model).display());
                }
            }
            None => println!("{}", store.root().display()),
        },
    }
    Ok(())
}

fn percent(done: u64, total: u64) -> f64 {
    done as f64 * 100.0 / total.max(1) as f64
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

fn bench(dir: &Path, args: &EngineArgs, runs: usize) -> Result<()> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    if files.is_empty() {
        bail!("no audio files in {}", dir.display());
    }
    files.sort();

    let clips = files
        .iter()
        .map(|path| Ok((path, audio::load(path)?)))
        .collect::<Result<Vec<_>>>()?;

    let started = Instant::now();
    let recognizer = args.load()?;
    println!("model load    {:>8.0} ms", ms(started.elapsed()));

    let started = Instant::now();
    recognizer.recognize(&clips[0].1)?;
    println!("first call    {:>8.0} ms", ms(started.elapsed()));
    println!();

    println!(
        "{:<32} {:>8} {:>11} {:>7}",
        "file", "audio s", "latency ms", "RTF"
    );
    let mut latencies = Vec::new();
    let mut total_audio = 0.0;
    for (path, samples) in &clips {
        let mut times = (0..runs)
            .map(|_| {
                let started = Instant::now();
                recognizer.recognize(samples).map(|_| started.elapsed())
            })
            .collect::<boltay_core::Result<Vec<_>>>()?;
        times.sort();
        let latency = times[times.len() / 2];
        let seconds = samples.len() as f64 / SAMPLE_RATE as f64;
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!(
            "{:<32} {:>8.2} {:>11.0} {:>7.3}",
            name,
            seconds,
            ms(latency),
            latency.as_secs_f64() / seconds
        );
        latencies.push(latency);
        total_audio += seconds;
    }

    let total: Duration = latencies.iter().sum();
    latencies.sort();
    println!();
    println!(
        "total         {:>8.2} s audio in {:.2} s, RTF {:.3}",
        total_audio,
        total.as_secs_f64(),
        total.as_secs_f64() / total_audio
    );
    println!(
        "latency       mean {:.0} ms, median {:.0} ms, max {:.0} ms",
        ms(total) / latencies.len() as f64,
        ms(latencies[latencies.len() / 2]),
        ms(latencies[latencies.len() - 1])
    );
    match memory::peak() {
        Some(bytes) => println!("peak memory   {:>8.0} MB", mb(bytes)),
        None => println!("peak memory   unknown"),
    }
    Ok(())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
