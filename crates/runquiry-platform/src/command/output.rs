//! stdout/stderr 有界并发采集。

use std::fmt;
use std::io::{self, Read};
use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::Duration;

use runquiry_core::{STDERR_LIMIT_BYTES, STDOUT_LIMIT_BYTES};

const DRAIN_GRACE: Duration = Duration::from_secs(1);
const READ_CHUNK: usize = 64 * 1024;

pub(super) type OutputReceiver = Receiver<io::Result<Vec<u8>>>;

pub(super) struct OutputReaders {
    pub stdout: Option<OutputReceiver>,
    pub stderr: Option<OutputReceiver>,
}

pub(super) fn spawn_readers(
    child: &mut Child,
    stdout_hit: Arc<AtomicBool>,
    stderr_hit: Arc<AtomicBool>,
) -> io::Result<OutputReaders> {
    let stdout = child
        .stdout
        .take()
        .map(|pipe| spawn_reader("runquiry-cmd-stdout", pipe, STDOUT_LIMIT_BYTES, stdout_hit))
        .transpose()
        .map_err(|error| stream_start_error("stdout", &error))?;
    let stderr = child
        .stderr
        .take()
        .map(|pipe| spawn_reader("runquiry-cmd-stderr", pipe, STDERR_LIMIT_BYTES, stderr_hit))
        .transpose()
        .map_err(|error| stream_start_error("stderr", &error))?;
    Ok(OutputReaders { stdout, stderr })
}

fn stream_start_error(stream: &str, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("启动 {stream} 读取线程失败：{error}"))
}

fn spawn_reader<R: Read + Send + 'static>(
    name: &str,
    mut stream: R,
    limit: usize,
    limit_hit: Arc<AtomicBool>,
) -> io::Result<Receiver<io::Result<Vec<u8>>>> {
    let (sender, receiver) = channel();
    thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let mut collected = Vec::with_capacity(limit.min(READ_CHUNK));
            let mut chunk = vec![0_u8; READ_CHUNK];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) => {
                        let remaining = limit.saturating_sub(collected.len());
                        collected.extend_from_slice(&chunk[..count.min(remaining)]);
                        if count > remaining {
                            limit_hit.store(true, Ordering::SeqCst);
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        return;
                    }
                }
            }
            let _ = sender.send(Ok(collected));
        })?;
    Ok(receiver)
}

#[derive(Debug)]
pub(super) enum StreamCollectError {
    Read(io::Error),
    Receive(RecvTimeoutError),
}

impl fmt::Display for StreamCollectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "读取失败：{error}"),
            Self::Receive(error) => write!(formatter, "接收失败：{error}"),
        }
    }
}

pub(super) fn collect(
    receiver: Option<Receiver<io::Result<Vec<u8>>>>,
) -> Result<Vec<u8>, StreamCollectError> {
    let Some(receiver) = receiver else {
        return Ok(Vec::new());
    };
    receiver
        .recv_timeout(DRAIN_GRACE)
        .map_err(StreamCollectError::Receive)?
        .map_err(StreamCollectError::Read)
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::{StreamCollectError, collect, spawn_reader};

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("fixture read failure"))
        }
    }

    #[test]
    fn reader_io_failure_is_not_reported_as_successful_output() -> Result<(), String> {
        let receiver = spawn_reader(
            "runquiry-test-failing-reader",
            FailingReader,
            1024,
            Arc::new(AtomicBool::new(false)),
        )
        .map_err(|error| error.to_string())?;

        let result = collect(Some(receiver));

        assert!(matches!(result, Err(StreamCollectError::Read(_))));
        Ok(())
    }
}
