use std::path::{Path, PathBuf};
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

const MAX_CHUNK: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Media {
    origin: String,
    token: String,
}

impl Media {
    pub fn url(&self, recording_id: &str) -> String {
        format!("{}/{}/{}", self.origin, self.token, recording_id)
    }
}

pub fn start(app: &AppHandle) -> std::io::Result<Media> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let app = app.clone();

    serve_on(listener, move |id| audio_of(&app, id))
}

fn serve_on<R>(listener: std::net::TcpListener, resolve: R) -> std::io::Result<Media>
where
    R: Fn(&str) -> Option<PathBuf> + Send + Sync + 'static,
{
    listener.set_nonblocking(true)?;

    let port = listener.local_addr()?.port();
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );

    let media = Media {
        origin: format!("http://127.0.0.1:{port}"),
        token: token.clone(),
    };

    let token = Arc::new(token);
    let resolve = Arc::new(resolve);

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(err) => {
                tracing::error!(%err, "the audio server could not start");
                return;
            }
        };

        tracing::info!(port, "serving recording audio on loopback");

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(err) => {
                    tracing::warn!(%err, "the audio server stopped accepting");
                    return;
                }
            };

            let resolve = Arc::clone(&resolve);
            let token = Arc::clone(&token);

            tauri::async_runtime::spawn(async move {
                let service = service_fn(move |request| {
                    let resolve = Arc::clone(&resolve);
                    let token = Arc::clone(&token);
                    async move {
                        Ok::<_, std::convert::Infallible>(route(resolve, &token, request).await)
                    }
                });

                if let Err(err) = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                {
                    tracing::debug!(%err, "an audio connection ended badly");
                }
            });
        }
    });

    Ok(media)
}

async fn route<R>(
    resolve: Arc<R>,
    token: &str,
    request: Request<hyper::body::Incoming>,
) -> Response<Full<Bytes>>
where
    R: Fn(&str) -> Option<PathBuf> + Send + Sync + 'static,
{
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }

    let Some(id) = recording_id(request.uri().path(), token) else {
        return empty(StatusCode::FORBIDDEN);
    };

    let range = request
        .headers()
        .get(hyper::header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    let head_only = request.method() == Method::HEAD;

    let answered = tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = resolve(&id) else {
            return empty(StatusCode::NOT_FOUND);
        };
        answer(&path, range.as_deref(), head_only)
    })
    .await;

    answered.unwrap_or_else(|err| {
        tracing::error!(%err, "the audio thread ended unexpectedly");
        empty(StatusCode::INTERNAL_SERVER_ERROR)
    })
}

fn recording_id(path: &str, token: &str) -> Option<String> {
    let rest = path.strip_prefix('/')?;
    let (given, id) = rest.split_once('/')?;

    if given.len() != token.len() || given != token {
        return None;
    }

    let id = id.trim_end_matches(".wav");

    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| id.to_owned())
}

fn audio_of(app: &AppHandle, recording_id: &str) -> Option<PathBuf> {
    let state = app.state::<AppState>();

    let stored = state
        .recordings
        .lock()
        .expect("recordings lock poisoned")
        .get(recording_id)
        .ok()??
        .recording
        .audio_path?;

    within(&state.paths.recordings_dir(), Path::new(&stored))
}

fn within(recordings: &Path, path: &Path) -> Option<PathBuf> {
    let recordings = recordings.canonicalize().ok()?;

    let resolved = match path.canonicalize() {
        Ok(resolved) => resolved,

        Err(_) => path.parent()?.canonicalize().ok()?.join(path.file_name()?),
    };

    resolved.starts_with(&recordings).then_some(resolved)
}

fn answer(path: &Path, range: Option<&str>, head_only: bool) -> Response<Full<Bytes>> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => {
            tracing::debug!(path = %path.display(), %err, "the audio could not be opened");
            return empty(StatusCode::NOT_FOUND);
        }
    };

    let Ok(length) = file.metadata().map(|data| data.len()) else {
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let base = Response::builder()
        .header(hyper::header::CONTENT_TYPE, "audio/wav")
        .header(hyper::header::ACCEPT_RANGES, "bytes")
        .header(hyper::header::CACHE_CONTROL, "no-store");

    let body = |bytes: Vec<u8>| if head_only { Vec::new() } else { bytes };

    let Some(range) = range else {
        let mut whole = Vec::new();
        if !head_only && file.read_to_end(&mut whole).is_err() {
            return empty(StatusCode::INTERNAL_SERVER_ERROR);
        }
        return base
            .status(StatusCode::OK)
            .header(hyper::header::CONTENT_LENGTH, length)
            .body(Full::new(Bytes::from(body(whole))))
            .unwrap_or_else(|_| empty(StatusCode::INTERNAL_SERVER_ERROR));
    };

    let Some((start, end)) = parse_range(range, length) else {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(hyper::header::CONTENT_RANGE, format!("bytes */{length}"))
            .body(Full::new(Bytes::new()))
            .unwrap_or_else(|_| empty(StatusCode::RANGE_NOT_SATISFIABLE));
    };

    let wanted = (end - start + 1) as usize;
    let mut slice = vec![0u8; wanted];

    if !head_only
        && (file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut slice).is_err())
    {
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    }

    base.status(StatusCode::PARTIAL_CONTENT)
        .header(
            hyper::header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{length}"),
        )
        .header(hyper::header::CONTENT_LENGTH, wanted)
        .body(Full::new(Bytes::from(body(slice))))
        .unwrap_or_else(|_| empty(StatusCode::INTERNAL_SERVER_ERROR))
}

fn parse_range(header: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let spec = header.trim().strip_prefix("bytes=")?;
    let (from, to) = spec.split_once('-')?;
    let (from, to) = (from.trim(), to.trim());

    let (start, end) = if from.is_empty() {
        let wanted: u64 = to.parse().ok()?;
        if wanted == 0 {
            return None;
        }
        (length.saturating_sub(wanted), length - 1)
    } else {
        let start: u64 = from.parse().ok()?;
        let end = if to.is_empty() {
            length - 1
        } else {
            to.parse::<u64>().ok()?.min(length - 1)
        };
        (start, end)
    };

    if start > end || start >= length {
        return None;
    }

    Some((start, end.min(start + MAX_CHUNK - 1)))
}

fn empty(status: StatusCode) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::new()))
        .expect("a status and an empty body is always a valid response")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().expect("temp dir");
        let recordings = root.path().join("recordings");
        std::fs::create_dir_all(&recordings).expect("create");

        let audio = recordings.join("take.wav");
        std::fs::write(&audio, (0u8..=255).cycle().take(4_096).collect::<Vec<u8>>())
            .expect("write");

        let secret = root.path().join("secrets.txt");
        std::fs::write(&secret, b"not audio").expect("write");

        (root, recordings, audio)
    }

    #[test]
    fn only_a_url_carrying_the_token_names_a_recording() {
        assert_eq!(
            recording_id(&format!("/{TOKEN}/abc-123"), TOKEN),
            Some("abc-123".to_owned())
        );

        assert_eq!(
            recording_id(&format!("/{TOKEN}/abc-123.wav"), TOKEN),
            Some("abc-123".to_owned())
        );

        assert_eq!(recording_id("/abc-123", TOKEN), None);
        assert_eq!(recording_id("/wrong/abc-123", TOKEN), None);
        assert_eq!(recording_id(&format!("/{TOKEN}x/abc"), TOKEN), None);
        assert_eq!(recording_id(&format!("/{TOKEN}/"), TOKEN), None);
    }

    #[test]
    fn a_path_can_never_be_asked_for_even_with_the_token() {
        for attempt in [
            "../../etc/passwd",
            "..%2F..%2Fetc%2Fpasswd",
            "/etc/passwd",
            "a/b",
            "take.wav.other",
        ] {
            assert_eq!(
                recording_id(&format!("/{TOKEN}/{attempt}"), TOKEN),
                None,
                "{attempt:?} must not reach the lookup"
            );
        }
    }

    #[test]
    fn a_whole_file_comes_back_as_audio_that_can_be_seeked_in() {
        let (_root, _recordings, audio) = fixture();

        let response = answer(&audio, None, false);

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-type"], "audio/wav");

        assert_eq!(response.headers()["accept-ranges"], "bytes");
        assert_eq!(response.headers()["content-length"], "4096");
    }

    #[test]
    fn a_head_asks_what_is_there_without_moving_it() {
        let (_root, _recordings, audio) = fixture();

        let response = answer(&audio, None, true);
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-length"], "4096");
    }

    #[test]
    fn a_range_comes_back_as_exactly_that_slice() {
        let (_root, _recordings, audio) = fixture();

        let response = answer(&audio, Some("bytes=1000-1099"), false);

        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["content-range"], "bytes 1000-1099/4096");
        assert_eq!(response.headers()["content-length"], "100");
    }

    #[test]
    fn a_range_past_the_end_is_answered_with_the_real_length() {
        let (_root, _recordings, audio) = fixture();

        let response = answer(&audio, Some("bytes=9000-9100"), false);
        assert_eq!(response.status(), 416);
        assert_eq!(response.headers()["content-range"], "bytes */4096");
    }

    #[test]
    fn a_recording_that_is_no_longer_there_is_a_404_not_a_crash() {
        let (_root, recordings, _audio) = fixture();

        assert_eq!(
            answer(&recordings.join("deleted.wav"), None, false).status(),
            404
        );
    }

    #[test]
    fn the_scope_check_keeps_to_the_recordings_directory() {
        let (root, recordings, audio) = fixture();

        assert!(within(&recordings, &audio).is_some());
        assert!(within(&recordings, &root.path().join("secrets.txt")).is_none());

        assert!(within(&recordings, &recordings.join("..").join("secrets.txt")).is_none());

        assert!(within(&recordings, &recordings.join("gone.wav")).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_directory_does_not_carry_the_permission_with_it() {
        let (root, recordings, _audio) = fixture();
        let link = recordings.join("innocent.wav");
        std::os::unix::fs::symlink(root.path().join("secrets.txt"), &link).expect("symlink");

        assert!(within(&recordings, &link).is_none());
    }

    #[test]
    fn a_range_is_read_the_way_a_player_writes_it() {
        assert_eq!(parse_range("bytes=0-1023", 8_000), Some((0, 1_023)));

        assert_eq!(parse_range("bytes=1000-", 8_000), Some((1_000, 7_999)));

        assert_eq!(parse_range("bytes=-500", 8_000), Some((7_500, 7_999)));
    }

    #[test]
    fn a_range_past_the_end_is_clamped_or_refused_but_never_read() {
        assert_eq!(parse_range("bytes=0-99999", 8_000), Some((0, 7_999)));
        assert_eq!(parse_range("bytes=8000-", 8_000), None);
        assert_eq!(parse_range("bytes=9000-9500", 8_000), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
    }

    #[test]
    fn nonsense_ranges_are_refused_rather_than_guessed_at() {
        assert_eq!(parse_range("frames=0-10", 8_000), None);
        assert_eq!(parse_range("bytes=abc-def", 8_000), None);
        assert_eq!(parse_range("bytes=", 8_000), None);
        assert_eq!(parse_range("bytes=-0", 8_000), None);

        assert_eq!(parse_range("bytes=500-100", 8_000), None);
    }

    #[test]
    fn one_answer_is_capped_however_much_is_asked_for() {
        let huge = 500 * 1024 * 1024;
        let (start, end) = parse_range("bytes=0-", huge).expect("a range");
        assert_eq!(start, 0);
        assert_eq!(end - start + 1, MAX_CHUNK);
    }

    fn request(media: &Media, path: &str, extra: &str) -> (String, Vec<u8>) {
        use std::io::{Read, Write};

        let address = media.origin.trim_start_matches("http://");
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        stream
            .write_all(
                format!(
                    "GET {path} HTTP/1.1\r\nHost: {address}\r\n{extra}Connection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .expect("write");

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read");

        let split = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("headers end");

        (
            String::from_utf8_lossy(&raw[..split]).to_string(),
            raw[split + 4..].to_vec(),
        )
    }

    #[test]
    fn the_server_hands_a_player_the_bytes_it_asks_for() {
        let (_root, _recordings, audio) = fixture();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");

        let served = audio.clone();
        let media =
            serve_on(listener, move |id| (id == "take").then(|| served.clone())).expect("start");

        let (headers, body) = request(&media, &format!("/{}/take", media.token), "");
        assert!(headers.starts_with("HTTP/1.1 200 "), "got: {headers}");
        assert!(
            headers.contains("content-type: audio/wav"),
            "got: {headers}"
        );
        assert!(headers.contains("accept-ranges: bytes"), "got: {headers}");
        assert_eq!(body.len(), 4_096);

        let (headers, body) = request(
            &media,
            &format!("/{}/take", media.token),
            "Range: bytes=1000-1099\r\n",
        );
        assert!(headers.starts_with("HTTP/1.1 206 "), "got: {headers}");
        assert!(
            headers.contains("content-range: bytes 1000-1099/4096"),
            "got: {headers}"
        );
        assert_eq!(body.len(), 100);
        assert_eq!(
            body[0],
            (1000 % 256) as u8,
            "the right bytes, not the first"
        );

        let (headers, body) = request(&media, "/guessing/take", "");
        assert!(headers.starts_with("HTTP/1.1 403 "), "got: {headers}");
        assert!(body.is_empty());

        let (headers, _) = request(&media, &format!("/{}/other", media.token), "");
        assert!(headers.starts_with("HTTP/1.1 404 "), "got: {headers}");
    }

    #[test]
    fn a_url_names_the_recording_and_nothing_about_the_disk() {
        let media = Media {
            origin: "http://127.0.0.1:4123".to_owned(),
            token: TOKEN.to_owned(),
        };

        let url = media.url("abc-123");
        assert_eq!(url, format!("http://127.0.0.1:4123/{TOKEN}/abc-123"));
        assert_eq!(
            recording_id(&url["http://127.0.0.1:4123".len()..], TOKEN).as_deref(),
            Some("abc-123")
        );
    }
}
