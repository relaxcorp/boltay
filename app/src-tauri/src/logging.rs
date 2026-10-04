use std::path::Path;

use log::LevelFilter;
use tauri::{AppHandle, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const MAX_FILE_SIZE: u128 = 2 * 1024 * 1024;
/// Rotated files kept next to the active one, so five files at most.
const KEEP: usize = 4;

pub fn init(app: &AppHandle, dir: &Path) -> tauri::Result<()> {
    // Error messages carry file paths; the user name inside them is nobody's business.
    let home = app
        .path()
        .home_dir()
        .ok()
        .map(|h| h.to_string_lossy().into_owned())
        .filter(|h| h.len() > 1);
    let plugin = tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::Stderr))
        .target(Target::new(TargetKind::Folder {
            path: dir.into(),
            file_name: Some("boltay".into()),
        }))
        .level(LevelFilter::Info)
        .max_file_size(MAX_FILE_SIZE)
        .rotation_strategy(RotationStrategy::KeepSome(KEEP))
        .format(move |out, message, record| {
            let now = TimezoneStrategy::UseLocal.get_now();
            let mut message = message.to_string();
            if let Some(home) = &home {
                message = message.replace(home.as_str(), "~");
            }
            out.finish(format_args!(
                "{} {:02}:{:02}:{:02} {:<5} {}: {message}",
                now.date(),
                now.hour(),
                now.minute(),
                now.second(),
                record.level(),
                record.target(),
            ))
        })
        .build();
    app.plugin(plugin)?;

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("{info}");
        previous(info);
    }));

    log::info!(
        "Boltay {} on {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    Ok(())
}
