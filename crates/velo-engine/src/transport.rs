use reqwest::{Request, header, multipart};
use tokio::io::AsyncReadExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    config::ValidatedConfig,
    http_options::{PreparedPart, UPLOAD_BUFFER_BYTES},
};

pub(crate) enum Failure {
    Timeout,
    Transport,
}

impl From<reqwest::Error> for Failure {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else {
            Self::Transport
        }
    }
}

async fn request(
    config: &ValidatedConfig,
    include_body: bool,
    cancel: &CancellationToken,
    uploads_done: &TaskTracker,
) -> Result<Request, Failure> {
    let mut request = config.request.try_clone().ok_or(Failure::Transport)?;
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
    let Some(parts) = &config.multipart else {
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
                let file = tokio::fs::File::open(path)
                    .await
                    .map_err(|_| Failure::Transport)?;
                let metadata = file.metadata().await.map_err(|_| Failure::Transport)?;
                if !metadata.is_file()
                    || metadata.len() != *length
                    || metadata.modified().ok() != *modified
                {
                    return Err(Failure::Transport);
                }
                let (send, receive) = tokio::sync::mpsc::channel(1);
                let mut uploads = config.uploads.lock().await;
                while uploads.try_join_next().is_some() {}
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

pub(crate) async fn execute(config: &ValidatedConfig, bytes: &mut u64) -> Result<bool, Failure> {
    let cancel = config.upload_stop.child_token();
    let _cleanup = cancel.clone().drop_guard();
    let uploads_done = TaskTracker::new();
    let result = tokio::time::timeout(
        config.timeout,
        consume(config, bytes, &cancel, &uploads_done),
    )
    .await
    .map_err(|_| Failure::Timeout)
    .and_then(|result| result);
    cancel.cancel();
    uploads_done.close();
    uploads_done.wait().await;
    result
}

async fn consume(
    config: &ValidatedConfig,
    bytes: &mut u64,
    cancel: &CancellationToken,
    uploads_done: &TaskTracker,
) -> Result<bool, Failure> {
    let request = request(config, true, cancel, uploads_done).await?;
    let mut response = config.client.execute(request).await?;
    let success = (200..400).contains(&response.status().as_u16());
    while let Some(chunk) = response.chunk().await? {
        *bytes = bytes.saturating_add(chunk.len() as u64);
    }
    Ok(success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MultipartPart, RequestBody, RequestMethod, RunConfig};

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
                true,
                &CancellationToken::new(),
                &TaskTracker::new()
            )
            .await,
            Err(Failure::Transport)
        ));
        file.close().unwrap();
    }
}
