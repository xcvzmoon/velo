use std::{
    error::Error,
    io,
    sync::atomic::{AtomicU16, Ordering},
};

use reqwest::{Method, Request, StatusCode, header, multipart};
use tokio::io::AsyncReadExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    config::ValidatedConfig,
    http_options::{PreparedPart, UPLOAD_BUFFER_BYTES},
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Failure {
    Timeout,
    Transport(Category),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Category {
    Connect,
    ConnectionReset,
    BrokenPipe,
    ConnectionRefused,
    Body,
    Decode,
    Request,
    RedirectPolicy,
    Upload,
    Other,
}

impl Failure {
    fn diagnostic(self) -> (&'static str, u16) {
        match self {
            Self::Timeout => ("timeout", 1 << 10),
            Self::Transport(category) => (category.name(), 1 << category as u8),
        }
    }
}

impl Category {
    fn name(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::ConnectionReset => "connection_reset",
            Self::BrokenPipe => "broken_pipe",
            Self::ConnectionRefused => "connection_refused",
            Self::Body => "body",
            Self::Decode => "decode",
            Self::Request => "request",
            Self::RedirectPolicy => "redirect_policy",
            Self::Upload => "upload",
            Self::Other => "other",
        }
    }
}

#[derive(Default)]
pub(crate) struct FailureDiagnostics(AtomicU16);

impl FailureDiagnostics {
    fn first(&self, failure: Failure) -> bool {
        let (_, mask) = failure.diagnostic();
        self.0.fetch_or(mask, Ordering::Relaxed) & mask == 0
    }

    pub(crate) fn record(&self, failure: Failure) {
        if self.first(failure) {
            let (category, _) = failure.diagnostic();
            tracing::warn!(
                domain = "http",
                code = "HTTP_REQUEST_FAILED",
                category,
                "First request failure in category for this run"
            );
        }
    }
}

fn io_category(error: &io::Error) -> Option<Category> {
    match error.kind() {
        io::ErrorKind::ConnectionReset => Some(Category::ConnectionReset),
        io::ErrorKind::BrokenPipe => Some(Category::BrokenPipe),
        io::ErrorKind::ConnectionRefused => Some(Category::ConnectionRefused),
        _ => None,
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self::Transport(io_category(&error).unwrap_or(Category::Upload))
    }
}

impl From<reqwest::Error> for Failure {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::Timeout;
        }
        let mut source = error.source();
        for _ in 0..16 {
            let Some(cause) = source else { break };
            if let Some(category) = cause.downcast_ref::<io::Error>().and_then(io_category) {
                return Self::Transport(category);
            }
            source = cause.source();
        }
        Self::Transport(if error.is_connect() {
            Category::Connect
        } else if error.is_body() {
            Category::Body
        } else if error.is_decode() {
            Category::Decode
        } else if error.is_request() || error.is_builder() {
            Category::Request
        } else {
            Category::Other
        })
    }
}

async fn request(
    config: &ValidatedConfig,
    record: Option<&crate::config::PreparedRequest>,
    include_body: bool,
    cancel: &CancellationToken,
    uploads_done: &TaskTracker,
) -> Result<Request, Failure> {
    let template = record.map_or(&config.request, |record| &record.request);
    let mut request = template
        .try_clone()
        .ok_or(Failure::Transport(Category::Request))?;
    if !include_body {
        *request.body_mut() = None;
        for name in [
            header::CONTENT_TYPE,
            header::CONTENT_LENGTH,
            header::CONTENT_ENCODING,
            header::TRANSFER_ENCODING,
        ] {
            request.headers_mut().remove(name);
        }
        return Ok(request);
    }
    let Some(parts) = record.map_or(config.multipart.as_ref(), |record| {
        record.multipart.as_ref()
    }) else {
        return Ok(request);
    };
    let mut form = multipart::Form::new();
    for part in parts {
        form = match part {
            PreparedPart::Text(pair) => form.text(pair.name.clone(), pair.value.clone()),
            PreparedPart::File {
                name,
                path,
                filename,
                content_type,
                length,
                modified,
            } => {
                let file = tokio::fs::File::open(path).await.map_err(Failure::from)?;
                let metadata = file.metadata().await.map_err(Failure::from)?;
                if !metadata.is_file()
                    || metadata.len() != *length
                    || metadata.modified().ok() != *modified
                {
                    return Err(Failure::Transport(Category::Upload));
                }
                let (send, receive) = tokio::sync::mpsc::channel(1);
                let mut uploads = config.uploads.lock().await;
                while let Some(result) = uploads.try_join_next() {
                    if let Err(error) = result
                        && error.is_panic()
                    {
                        std::panic::resume_unwind(error.into_panic());
                    }
                }
                let cancel = cancel.clone();
                let remaining_length = *length;
                let tracked = uploads_done.token();
                uploads.spawn(async move {
                    let _tracked = tracked;
                    let mut remaining = remaining_length;
                    let mut file = file;
                    while remaining > 0 && !cancel.is_cancelled() {
                        let mut chunk = vec![0; remaining.min(UPLOAD_BUFFER_BYTES as u64) as usize];
                        let result = match file.read(&mut chunk).await {
                            Ok(0) => Err(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "UPLOAD_SOURCE_CHANGED",
                            )),
                            Ok(length) => {
                                chunk.truncate(length);
                                remaining -= length as u64;
                                Ok(chunk)
                            }
                            Err(error) => Err(error),
                        };
                        let failed = result.is_err();
                        tokio::select! {
                            biased;
                            _ = cancel.cancelled() => break,
                            result = send.send(result) => { if result.is_err() { break; } }
                        }
                        if failed {
                            break;
                        }
                    }
                    drop(file.into_std().await);
                });
                drop(uploads);
                let stream = ReceiverStream::new(receive);
                let part = multipart::Part::stream_with_length(
                    reqwest::Body::wrap_stream(stream),
                    *length,
                )
                .file_name(filename.clone())
                .mime_str(content_type)?;
                form.part(name.clone(), part)
            }
        };
    }
    config
        .client
        .request(request.method().clone(), request.url().clone())
        .headers(request.headers().clone())
        .multipart(form)
        .build()
        .map_err(Failure::from)
}

pub(crate) async fn execute(
    config: &ValidatedConfig,
    record: Option<&crate::config::PreparedRequest>,
    bytes: &mut u64,
) -> Result<bool, Failure> {
    let cancel = config.upload_stop.child_token();
    let _cleanup = cancel.clone().drop_guard();
    let uploads_done = TaskTracker::new();
    let result = tokio::time::timeout(
        config.timeout,
        consume(config, record, bytes, &cancel, &uploads_done, None),
    )
    .await
    .map_err(|_| Failure::Timeout)
    .and_then(|result| result);
    cancel.cancel();
    uploads_done.close();
    uploads_done.wait().await;
    result
}

pub(crate) async fn preview(
    config: &ValidatedConfig,
) -> Result<crate::preview::PreviewResponse, Failure> {
    let cancel = config.upload_stop.child_token();
    let _cleanup = cancel.clone().drop_guard();
    let uploads_done = TaskTracker::new();
    let started = std::time::Instant::now();
    let mut response = crate::preview::PreviewResponse::default();
    let mut bytes = 0;
    let result = tokio::time::timeout(
        config.timeout,
        consume(
            config,
            None,
            &mut bytes,
            &cancel,
            &uploads_done,
            Some(&mut response),
        ),
    )
    .await
    .map_err(|_| Failure::Timeout)
    .and_then(|result| result);
    cancel.cancel();
    uploads_done.close();
    uploads_done.wait().await;
    result?;
    response.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(response)
}

async fn consume(
    config: &ValidatedConfig,
    record: Option<&crate::config::PreparedRequest>,
    bytes: &mut u64,
    cancel: &CancellationToken,
    uploads_done: &TaskTracker,
    mut preview: Option<&mut crate::preview::PreviewResponse>,
) -> Result<bool, Failure> {
    let template = record.map_or(&config.request, |record| &record.request);
    let mut url = template.url().clone();
    let mut method = template.method().clone();
    let mut include_body = true;
    let mut followed = 0_u32;
    loop {
        let hop_cancel = cancel.child_token();
        let _hop_cleanup = hop_cancel.clone().drop_guard();
        let mut request = request(config, record, include_body, &hop_cancel, uploads_done).await?;
        *request.url_mut() = url.clone();
        *request.method_mut() = method.clone();
        let mut response = config.client.execute(request).await?;
        let status = response.status();
        let location = response.headers().get(header::LOCATION).cloned();
        if let Some(capture) = preview.as_deref_mut() {
            capture.status = status.as_u16();
            capture.headers.clear();
            capture.body.clear();
            capture.truncated = false;
            let mut retained = 0;
            for (name, value) in response.headers() {
                let value = value.to_str().unwrap_or("[binary]");
                if retained + name.as_str().len() + value.len() > 16_384 {
                    capture.truncated = true;
                    break;
                }
                retained += name.as_str().len() + value.len();
                capture.headers.push(crate::NameValue {
                    name: name.to_string(),
                    value: value.into(),
                });
            }
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            *bytes = bytes.saturating_add(chunk.len() as u64);
            if let Some(capture) = preview.as_deref_mut() {
                let remaining = 65_536_usize.saturating_sub(body.len());
                body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
                if chunk.len() > remaining {
                    capture.truncated = true;
                    break;
                }
            }
        }
        if let Some(capture) = preview.as_deref_mut() {
            capture.body = String::from_utf8_lossy(&body).into_owned();
        }
        hop_cancel.cancel();
        uploads_done.close();
        uploads_done.wait().await;
        if config.max_redirects == 0
            || !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
            || location.is_none()
        {
            return Ok((200..400).contains(&status.as_u16()));
        }
        if followed >= config.max_redirects {
            return Err(Failure::Transport(Category::RedirectPolicy));
        }
        let location = location.ok_or(Failure::Transport(Category::RedirectPolicy))?;
        let mut next = url
            .join(
                location
                    .to_str()
                    .map_err(|_| Failure::Transport(Category::RedirectPolicy))?,
            )
            .map_err(|_| Failure::Transport(Category::RedirectPolicy))?;
        next.set_fragment(None);
        if next.origin() != url.origin() || !next.username().is_empty() || next.password().is_some()
        {
            return Err(Failure::Transport(Category::RedirectPolicy));
        }
        if (status == StatusCode::SEE_OTHER && method != Method::HEAD)
            || (matches!(status.as_u16(), 301 | 302) && method == Method::POST)
        {
            method = Method::GET;
            include_body = false;
        }
        followed += 1;
        url = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MultipartPart, RequestBody, RequestMethod, RunConfig};

    #[test]
    fn diagnostic_categories_do_not_retain_infrastructure_messages() {
        for (kind, expected) in [
            (io::ErrorKind::ConnectionReset, "connection_reset"),
            (io::ErrorKind::BrokenPipe, "broken_pipe"),
            (io::ErrorKind::ConnectionRefused, "connection_refused"),
        ] {
            let failure = Failure::from(io::Error::new(kind, "AUDIT_DUMMY_SECRET"));
            assert_eq!(failure.diagnostic().0, expected);
            assert!(!format!("{failure:?}").contains("AUDIT_DUMMY_SECRET"));
        }
    }

    #[test]
    fn diagnostics_are_bounded_once_per_category_and_reset_per_run() {
        let diagnostics = FailureDiagnostics::default();
        for category in [
            Category::Connect,
            Category::ConnectionReset,
            Category::BrokenPipe,
            Category::ConnectionRefused,
            Category::Body,
            Category::Decode,
            Category::Request,
            Category::RedirectPolicy,
            Category::Upload,
            Category::Other,
        ] {
            let failure = Failure::Transport(category);
            assert!(diagnostics.first(failure));
            for _ in 0..100 {
                assert!(!diagnostics.first(failure));
            }
        }
        assert!(diagnostics.first(Failure::Timeout));
        assert!(!diagnostics.first(Failure::Timeout));
        assert_eq!(diagnostics.0.load(Ordering::Relaxed).count_ones(), 11);
        assert!(FailureDiagnostics::default().first(Failure::Timeout));
    }

    #[tokio::test]
    async fn reqwest_connection_failures_keep_a_safe_cause_category() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}/AUDIT_DUMMY_SECRET"))
            .send()
            .await
            .unwrap_err();
        let failure = Failure::from(error);
        assert_eq!(failure.diagnostic().0, "connection_refused");
        assert!(!format!("{failure:?}").contains("AUDIT_DUMMY_SECRET"));
    }

    #[tokio::test]
    async fn changed_upload_source_fails_before_sending_and_releases_handles() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let config = crate::config::validate(RunConfig {
            url: "http://127.0.0.1:1/".into(),
            method: RequestMethod::POST,
            body: RequestBody::Multipart(vec![MultipartPart::File {
                name: "upload".into(),
                path: file.path().to_str().unwrap().into(),
                filename: "test.bin".into(),
                content_type: "application/octet-stream".into(),
            }]),
            ..Default::default()
        })
        .unwrap();
        file.as_file().set_len(10).unwrap();
        assert!(matches!(
            request(
                &config,
                None,
                true,
                &CancellationToken::new(),
                &TaskTracker::new()
            )
            .await,
            Err(Failure::Transport(Category::Upload))
        ));
        file.close().unwrap();
    }
}
