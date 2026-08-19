use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub title: String,
    pub duration_ms: Option<u32>,

    pub site: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("yt-dlp is not installed")]
    NotInstalled,

    #[error("that link could not be read: {0}")]
    Rejected(String),

    #[error("the download failed: {0}")]
    Failed(String),

    #[error("yt-dlp could not be run: {0}")]
    Unusable(String),
}

pub fn nightly_install_command(tools_dir: Option<&Path>) -> Option<String> {
    let target = tools_target(tools_dir?);
    let url = format!(
        "https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/latest/download/{}",
        standalone_asset()
    );

    Some(if cfg!(target_os = "windows") {
        format!(
            "curl.exe -L --create-dirs -o \"{}\" {url}",
            target.display()
        )
    } else {
        format!(
            "curl -L --create-dirs -o \"{0}\" {url} && chmod +x \"{0}\"",
            target.display()
        )
    })
}

impl DownloadError {
    pub fn remedy(&self, tools_dir: Option<&Path>) -> String {
        match self {
            DownloadError::NotInstalled => format!(
                "Install it with `{}`, then try again. It does not need Python — the \
                 `{}` build from the yt-dlp releases page carries its own.",
                install_hint(),
                standalone_asset()
            ),
            DownloadError::Rejected(_) => {
                "Check the link opens in a browser. Private, age-restricted and members-only \
                 videos cannot be fetched without signing in, which ZScribe does not do."
                    .to_owned()
            }
            DownloadError::Failed(message) => {
                if message.contains("js runtime")
                    || message.contains("JS runtime")
                    || message.contains("nsig")
                    || message.contains("challenge")
                {
                    return format!(
                        "YouTube needs a JavaScript runtime to hand over the audio, and none \
                         was found. Install Deno (`{}`), or Node — ZScribe uses whichever is \
                         there.",
                        js_runtime_install_hint()
                    );
                }

                if message.contains("403") || message.contains("Forbidden") {
                    let base = "The site handed over a link and then refused the audio itself. \
                                That is YouTube turning away this version of yt-dlp, not a \
                                problem with the link or the connection — and no format or \
                                setting here gets around it. Its stable release runs weeks \
                                behind YouTube's changes; the nightly build carries the fix.";

                    return match nightly_install_command(tools_dir) {
                        Some(command) => format!("{base}\n\n{command}"),
                        None => format!("{base} Install the nightly standalone build."),
                    };
                }

                "The connection may have dropped, or the site may have changed its player. \
                 Updating yt-dlp fixes the second one; it moves faster than the sites do."
                    .to_owned()
            }
            DownloadError::Unusable(_) => {
                "yt-dlp was found but would not run. Check it works from a terminal with \
                 `yt-dlp --version`."
                    .to_owned()
            }
        }
    }
}

pub fn install_hint() -> String {
    install_hint_with(|program| which(program).is_some())
}

pub fn js_runtime_install_hint() -> String {
    install_hint().replace("yt-dlp", "deno")
}

fn install_hint_with(available: impl Fn(&str) -> bool) -> String {
    if cfg!(target_os = "windows") {
        return "winget install yt-dlp".to_owned();
    }
    if cfg!(target_os = "macos") {
        return "brew install yt-dlp".to_owned();
    }

    for (manager, command) in [
        ("pacman", "sudo pacman -S yt-dlp"),
        ("apt", "sudo apt install yt-dlp"),
        ("dnf", "sudo dnf install yt-dlp"),
        ("zypper", "sudo zypper install yt-dlp"),
        ("apk", "sudo apk add yt-dlp"),
        ("pipx", "pipx install yt-dlp"),
    ] {
        if available(manager) {
            return command.to_owned();
        }
    }

    "pipx install yt-dlp".to_owned()
}

pub const fn standalone_asset() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else {
        "yt-dlp_linux"
    }
}

fn executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

pub fn tools_target(tools_dir: &Path) -> PathBuf {
    tools_dir.join(executable_name())
}

const JS_RUNTIMES: [(&str, &str); 3] = [("deno", "deno"), ("node", "node"), ("qjs", "quickjs")];

pub fn js_runtime() -> Option<&'static str> {
    JS_RUNTIMES
        .iter()
        .find(|(program, _)| which(program).is_some())
        .map(|(_, name)| *name)
}

fn js_runtime_flag(available: impl Fn(&str) -> bool) -> Option<&'static str> {
    if available("deno") {
        return None;
    }

    JS_RUNTIMES
        .iter()
        .skip(1)
        .find(|(program, _)| available(program))
        .map(|(_, name)| *name)
}

fn supports_flag(program: &Path, flag: &str) -> bool {
    Command::new(program)
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(flag))
        .unwrap_or(false)
}

pub fn version(program: &Path) -> Option<String> {
    let output = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!version.is_empty()).then_some(version)
}

pub fn release_age_days(version: &str, today: time::Date) -> Option<u32> {
    let mut parts = version.split('.');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;

    let released =
        time::Date::from_calendar_date(year, time::Month::try_from(month).ok()?, day).ok()?;

    Some((today - released).whole_days().clamp(0, u32::MAX.into()) as u32)
}

pub const STALE_AFTER_DAYS: u32 = 30;

fn js_runtime_args(program: &Path) -> Vec<String> {
    let Some(runtime) = js_runtime_flag(|name| which(name).is_some()) else {
        return Vec::new();
    };

    if !supports_flag(program, "--js-runtimes") {
        tracing::debug!("this yt-dlp has no --js-runtimes flag; leaving it to choose");
        return Vec::new();
    }

    tracing::info!(
        runtime,
        "no Deno found; telling yt-dlp to use another runtime"
    );
    vec!["--js-runtimes".to_owned(), runtime.to_owned()]
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

pub fn find(tools_dir: Option<&Path>) -> Option<PathBuf> {
    let name = executable_name();

    if let Some(explicit) = std::env::var_os("ZSCRIBE_YT_DLP") {
        let explicit = PathBuf::from(explicit);
        if explicit.is_file() {
            return Some(explicit);
        }
        tracing::warn!(
            path = %explicit.display(),
            "ZSCRIBE_YT_DLP does not point at a file; ignoring it"
        );
    }

    if let Some(candidate) = tools_dir.map(|dir| dir.join(name)) {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(found) = which(name) {
        return Some(found);
    }

    let extra: Vec<PathBuf> = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(|local| {
                let local = PathBuf::from(local);
                vec![
                    local.join("Microsoft\\WindowsApps"),
                    local.join("Programs\\Python\\Scripts"),
                    local.join("pipx\\venvs\\yt-dlp\\Scripts"),
                ]
            })
            .unwrap_or_default()
    } else {
        dirs_home()
            .into_iter()
            .flat_map(|home| {
                [
                    home.join(".local/bin"),
                    home.join("bin"),
                    home.join(".local/pipx/venvs/yt-dlp/bin"),
                ]
            })
            .chain([
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/opt/homebrew/bin"),
            ])
            .collect()
    };

    extra
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn inspect(url: &str, tools_dir: Option<&Path>) -> Result<LinkInfo, DownloadError> {
    let program = find(tools_dir).ok_or(DownloadError::NotInstalled)?;

    let output = Command::new(&program)
        .args([
            "--dump-single-json",
            "--no-playlist",
            "--no-warnings",
            "--skip-download",
        ])
        .args(js_runtime_args(&program))
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| DownloadError::Unusable(err.to_string()))?;

    if !output.status.success() {
        return Err(DownloadError::Rejected(last_error_line(&output.stderr)));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| DownloadError::Rejected(err.to_string()))?;

    Ok(LinkInfo {
        title: json
            .get("title")
            .and_then(|value| value.as_str())
            .filter(|title| !title.trim().is_empty())
            .unwrap_or("Imported from a link")
            .to_owned(),
        duration_ms: json
            .get("duration")
            .and_then(serde_json::Value::as_f64)
            .map(|seconds| (seconds * 1000.0) as u32),
        site: json
            .get("extractor_key")
            .or_else(|| json.get("extractor"))
            .and_then(|value| value.as_str())
            .unwrap_or("a link")
            .to_owned(),
    })
}

pub const FORMAT: &str =
    "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/best[ext=mp4]/bestaudio[ext=mp3]/best";

pub fn fetch(
    url: &str,
    destination: &Path,
    tools_dir: Option<&Path>,
    mut on_progress: impl FnMut(u8),
) -> Result<(), DownloadError> {
    let program = find(tools_dir).ok_or(DownloadError::NotInstalled)?;

    let mut child = Command::new(&program)
        .args([
            "--no-playlist",
            "--no-warnings",
            "--newline",
            "--no-part",
            "-f",
            FORMAT,
        ])
        .args(js_runtime_args(&program))
        .arg("-o")
        .arg(destination)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| DownloadError::Unusable(err.to_string()))?;

    if let Some(stdout) = child.stdout.take() {
        let mut reported = 0u8;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(percent) = parse_percent(&line) {
                if percent > reported {
                    reported = percent;
                    on_progress(percent);
                }
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| DownloadError::Unusable(err.to_string()))?;

    if !output.status.success() {
        return Err(DownloadError::Failed(last_error_line(&output.stderr)));
    }

    if !destination.is_file() {
        return Err(DownloadError::Failed(
            "yt-dlp reported success but wrote no file".to_owned(),
        ));
    }

    Ok(())
}

fn parse_percent(line: &str) -> Option<u8> {
    if !line.starts_with("[download]") {
        return None;
    }

    let token = line.split_whitespace().find(|word| word.ends_with('%'))?;
    let value: f32 = token.trim_end_matches('%').parse().ok()?;

    Some(value.clamp(0.0, 100.0) as u8)
}

fn last_error_line(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);

    let best = text
        .lines()
        .rev()
        .find(|line| line.contains("ERROR:"))
        .or_else(|| text.lines().rev().find(|line| !line.trim().is_empty()));

    best.map(|line| line.trim().trim_start_matches("ERROR:").trim().to_owned())
        .unwrap_or_else(|| "no reason given".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_progress_line_yields_its_percentage() {
        assert_eq!(
            parse_percent("[download]  45.3% of  4.56MiB at 1.20MiB/s ETA 00:03"),
            Some(45)
        );
        assert_eq!(
            parse_percent("[download] 100% of 4.56MiB in 00:03"),
            Some(100)
        );
        assert_eq!(parse_percent("[download]   0.0% of ~10.00MiB"), Some(0));
    }

    #[test]
    fn lines_that_are_not_progress_are_ignored() {
        assert_eq!(parse_percent("[youtube] Extracting URL: https://..."), None);
        assert_eq!(parse_percent("[download] Destination: out.m4a"), None);
        assert_eq!(parse_percent(""), None);
        assert_eq!(parse_percent("[info] Available formats:"), None);
    }

    #[test]
    fn a_nonsense_percentage_cannot_escape_the_range() {
        assert_eq!(parse_percent("[download] 120.0% of 1MiB"), Some(100));
        assert_eq!(parse_percent("[download] abc% of 1MiB"), None);
    }

    #[test]
    fn the_useful_line_is_pulled_out_of_the_noise() {
        let stderr = b"Traceback (most recent call last):\n  File \"x.py\"\nERROR: [youtube] abc: Video unavailable\n";
        assert_eq!(last_error_line(stderr), "[youtube] abc: Video unavailable");
    }

    #[test]
    fn a_failure_with_no_error_line_still_says_something() {
        assert_eq!(last_error_line(b""), "no reason given");
        assert_eq!(last_error_line(b"something odd\n"), "something odd");
    }

    #[test]
    fn the_install_hint_follows_the_package_manager_that_is_present() {
        if cfg!(target_os = "linux") {
            assert_eq!(
                install_hint_with(|p| p == "pacman"),
                "sudo pacman -S yt-dlp"
            );
            assert_eq!(install_hint_with(|p| p == "apt"), "sudo apt install yt-dlp");
            assert_eq!(install_hint_with(|p| p == "dnf"), "sudo dnf install yt-dlp");
            assert_eq!(install_hint_with(|_| false), "pipx install yt-dlp");
        }
    }

    #[test]
    fn a_system_package_is_preferred_over_pipx() {
        if cfg!(target_os = "linux") {
            let both = |p: &str| p == "pacman" || p == "pipx";
            assert_eq!(install_hint_with(both), "sudo pacman -S yt-dlp");
        }
    }

    #[test]
    fn deno_is_left_to_yt_dlp_to_find() {
        assert_eq!(js_runtime_flag(|r| r == "deno"), None);
        assert_eq!(js_runtime_flag(|r| r == "deno" || r == "node"), None);
    }

    #[test]
    fn node_is_named_when_deno_is_missing() {
        assert_eq!(js_runtime_flag(|r| r == "node"), Some("node"));
        assert_eq!(js_runtime_flag(|r| r == "qjs"), Some("quickjs"));
    }

    #[test]
    fn nothing_is_named_when_no_runtime_exists() {
        assert_eq!(js_runtime_flag(|_| false), None);
    }

    #[test]
    fn a_challenge_failure_points_at_the_runtime_not_at_the_network() {
        let err = DownloadError::Failed("Unable to solve nsig challenge".to_owned());
        let remedy = err.remedy(None);
        assert!(remedy.contains("JavaScript runtime"), "got: {remedy}");
        assert!(remedy.contains("Node"), "got: {remedy}");

        let other = DownloadError::Failed("Connection reset by peer".to_owned());
        assert!(other.remedy(None).contains("connection may have dropped"));
    }

    #[test]
    fn a_403_is_blamed_on_a_stale_yt_dlp_not_on_the_network() {
        let err = DownloadError::Failed(
            "unable to download video data: HTTP Error 403: Forbidden".to_owned(),
        );
        let remedy = err.remedy(None);

        assert!(
            remedy.contains("turning away this version of yt-dlp"),
            "got: {remedy}"
        );

        assert!(!remedy.to_lowercase().contains("internet"), "got: {remedy}");
        assert!(
            !remedy.contains("connection may have dropped"),
            "got: {remedy}"
        );

        let with_dir = err.remedy(Some(Path::new("/home/e/.local/share/zscribe/bin")));
        assert!(with_dir.contains("nightly-builds"), "got: {with_dir}");
        assert!(
            with_dir.contains("/home/e/.local/share/zscribe/bin"),
            "got: {with_dir}"
        );
        assert!(with_dir.contains(standalone_asset()), "got: {with_dir}");

        assert!(with_dir.contains("--create-dirs"), "got: {with_dir}");
    }

    #[test]
    fn the_standalone_build_is_named_for_this_platform() {
        let asset = standalone_asset();
        assert!(asset.starts_with("yt-dlp"));
        assert!(!asset.contains(' '));
    }

    #[test]
    fn the_nightly_command_writes_the_name_that_will_actually_be_found() {
        let dir = Path::new("/home/e/.local/share/zscribe/bin");
        let command = nightly_install_command(Some(dir)).expect("a directory was given");

        assert!(
            command.contains(&tools_target(dir).display().to_string()),
            "got: {command}"
        );
        assert!(command.contains(standalone_asset()), "got: {command}");

        if cfg!(target_os = "windows") {
            assert!(command.starts_with("curl.exe "), "got: {command}");
        } else {
            assert!(command.contains("chmod +x"), "got: {command}");
        }
    }

    #[test]
    fn a_version_is_read_as_the_date_it_is() {
        let today = time::Date::from_calendar_date(2026, time::Month::August, 16).unwrap();

        assert_eq!(release_age_days("2026.07.04", today), Some(43));
        assert_eq!(release_age_days("2026.08.16.232411", today), Some(0));
    }

    #[test]
    fn a_version_that_is_not_a_date_reports_no_age_rather_than_a_wrong_one() {
        assert_eq!(release_age_days("nightly", time::Date::MIN), None);
        assert_eq!(release_age_days("2026.08", time::Date::MIN), None);
        assert_eq!(release_age_days("", time::Date::MIN), None);
        assert_eq!(release_age_days("2026.13.40", time::Date::MIN), None);
    }

    #[test]
    fn a_clock_behind_the_release_does_not_report_a_negative_age() {
        let today = time::Date::from_calendar_date(2026, time::Month::August, 1).unwrap();
        assert_eq!(release_age_days("2026.08.16", today), Some(0));
    }

    #[test]
    fn an_explicit_path_that_does_not_exist_is_ignored_rather_than_used() {
        temp_env("ZSCRIBE_YT_DLP", "/nonexistent/yt-dlp", || {
            let found = find(None);
            assert!(
                found.as_deref() != Some(Path::new("/nonexistent/yt-dlp")),
                "a missing override must not be returned"
            );
        });
    }

    fn temp_env(key: &str, value: &str, body: impl FnOnce()) {
        let previous = std::env::var_os(key);

        unsafe { std::env::set_var(key, value) };
        body();
        match previous {
            Some(old) => unsafe { std::env::set_var(key, old) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    #[test]
    fn the_format_selector_never_asks_for_opus() {
        assert!(!FORMAT.contains("opus"));
        assert!(FORMAT.starts_with("bestaudio[ext=m4a]"));
    }
}
