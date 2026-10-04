use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ureq::tls::{RootCerts, TlsConfig};
use ureq::{Agent, Proxy};

use crate::Shared;

const LATEST: &str = "https://api.github.com/repos/relaxcorp/boltay/releases/latest";
const RELEASES: &str = "https://github.com/relaxcorp/boltay/releases";
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// What the settings window and the tray show about new versions. Nothing is downloaded:
/// "Download" opens the release page.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub checking: bool,
    /// A check went through since the app started.
    pub checked: bool,
    pub available: Option<Release>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub notes: String,
}

#[derive(Default)]
pub struct Updates {
    status: Mutex<Status>,
    last: Mutex<Option<Instant>>,
}

impl Updates {
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// Checks at start and then once a day while the setting is on.
pub fn watch(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || loop {
        let shared = app.state::<Shared>();
        let due = shared
            .updates
            .last
            .lock()
            .unwrap()
            .is_none_or(|at| at.elapsed() >= DAY);
        if due && shared.settings().check_updates {
            check(&app);
        }
        thread::sleep(Duration::from_secs(60 * 60));
    });
}

/// Asks GitHub for the latest release. Failures only go to the log: no network or a proxy
/// in the way are normal.
pub fn check(app: &AppHandle) -> Status {
    let shared = app.state::<Shared>();
    shared.updates.status.lock().unwrap().checking = true;
    let _ = app.emit("update-changed", ());
    let result = latest();
    *shared.updates.last.lock().unwrap() = Some(Instant::now());
    let status = {
        let mut status = shared.updates.status.lock().unwrap();
        status.checking = false;
        match result {
            Ok(release) => {
                status.checked = true;
                status.available = release.filter(|r| newer(&r.version, env!("CARGO_PKG_VERSION")));
            }
            Err(e) => log::info!("update check: {e:#}"),
        }
        status.clone()
    };
    if let Some(release) = &status.available {
        log::info!("version {} is out", release.version);
    }
    let _ = app.emit("update-changed", ());
    crate::refresh_tray(app);
    status
}

fn latest() -> Result<Option<Release>> {
    let agent: Agent = Agent::config_builder()
        .proxy(Proxy::try_from_env())
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("boltay/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut response = agent
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()?;
    match response.status().as_u16() {
        200 => {}
        // No published release yet.
        404 => return Ok(None),
        status => bail!("{LATEST}: HTTP {status}"),
    }
    let release: GithubRelease = response.body_mut().read_json()?;
    if release.prerelease || release.draft {
        return Ok(None);
    }
    Ok(Some(Release {
        version: release.tag_name.trim_start_matches('v').to_string(),
        url: page(&release.tag_name),
        notes: release.body.unwrap_or_default(),
    }))
}

/// "Download" leads to our own releases, whatever the response says.
fn page(tag: &str) -> String {
    let plain = !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    if plain {
        format!("{RELEASES}/tag/{tag}")
    } else {
        RELEASES.into()
    }
}

/// `1.10.0` is newer than `1.9.3`. Anything after a `-` or `+` is ignored.
fn newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_stays_on_our_releases() {
        assert_eq!(page("v0.9.0"), format!("{RELEASES}/tag/v0.9.0"));
        for tag in ["", "../../evil", "v1?x=https://evil", "v1#frag", "v1/../x"] {
            assert_eq!(page(tag), RELEASES, "{tag}");
        }
    }

    #[test]
    fn compares_versions_as_numbers() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("v1.10.0", "1.9.3"));
        assert!(newer("0.1.1", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("1.0.0-beta.1", "1.0.0"));
    }

    #[test]
    fn junk_is_never_newer() {
        assert!(!newer("nightly-5", "0.1.0"));
        assert!(!newer("", "0.1.0"));
        assert!(!newer("latest", "0.1.0"));
    }
}
