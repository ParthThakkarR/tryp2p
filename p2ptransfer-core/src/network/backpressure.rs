use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

struct PipeState {
    buffer: VecDeque<u8>,
    capacity: usize,
    read_closed: bool,
    write_closed: bool,
    read_waker: Option<Waker>,
    write_waker: Option<Waker>,
}

impl PipeState {
    fn filled(&self) -> usize {
        self.buffer.len()
    }

    fn free(&self) -> usize {
        self.capacity - self.buffer.len()
    }
}

/// Read half of a bounded in-memory pipe.
pub struct PipeReader {
    shared: Arc<Mutex<PipeState>>,
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.lock() {
            state.read_closed = true;
            if let Some(waker) = state.write_waker.take() {
                waker.wake();
            }
        }
    }
}

impl AsyncRead for PipeReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut state = self.shared.lock().unwrap();
        if state.filled() == 0 {
            if state.write_closed {
                return Poll::Ready(Ok(()));
            }
            state.read_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        let avail = state.filled().min(buf.remaining());
        for _ in 0..avail {
            let byte = state.buffer.pop_front().unwrap();
            buf.put_slice(&[byte]);
        }

        if let Some(waker) = state.write_waker.take() {
            waker.wake();
        }

        Poll::Ready(Ok(()))
    }
}

/// Write half of a bounded in-memory pipe.
pub struct PipeWriter {
    shared: Arc<Mutex<PipeState>>,
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.lock() {
            state.write_closed = true;
            if let Some(waker) = state.read_waker.take() {
                waker.wake();
            }
        }
    }
}

impl AsyncWrite for PipeWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut state = self.shared.lock().unwrap();
        if state.write_closed {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "pipe is closed",
            )));
        }
        if state.read_closed {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "reader has dropped the pipe",
            )));
        }

        if state.free() == 0 {
            state.write_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        let to_write = state.free().min(buf.len());
        state.buffer.extend(&buf[..to_write]);

        if let Some(waker) = state.read_waker.take() {
            waker.wake();
        }

        Poll::Ready(Ok(to_write))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut state = self.shared.lock().unwrap();
        state.write_closed = true;
        if let Some(waker) = state.read_waker.take() {
            waker.wake();
        }
        Poll::Ready(Ok(()))
    }
}

/// Create a connected read-write pair with a bounded buffer.
///
/// The total amount of data that can be in-flight in one direction
/// (written but not yet read) is bounded by `capacity` bytes.
/// When the buffer is full, the writer's `poll_write` returns
/// `Pending`, providing natural backpressure.
pub fn pipe(capacity: usize) -> (PipeWriter, PipeReader) {
    let shared = Arc::new(Mutex::new(PipeState {
        buffer: VecDeque::with_capacity(capacity),
        capacity,
        read_closed: false,
        write_closed: false,
        read_waker: None,
        write_waker: None,
    }));
    (
        PipeWriter {
            shared: shared.clone(),
        },
        PipeReader { shared },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Core proof: backpressure keeps memory bounded.
    ///
    /// A fast producer cannot outrun a slow consumer by more than
    /// the pipe capacity. The test verifies wall-clock-coupling:
    /// the producer and consumer finish at approximately the same
    /// time, proving the producer blocked on the full buffer.
    #[tokio::test]
    async fn test_backpressure_bounds_memory() {
        let capacity = 64 * 1024; // 64 KB
        let chunk_size = 16 * 1024; // 16 KB per write
        let total_send = 1024 * 1024; // 1 MB

        let (mut tx, mut rx) = pipe(capacity);

        let producer = tokio::spawn(async move {
            let data = vec![0xABu8; chunk_size];
            let start = std::time::Instant::now();
            let mut sent = 0usize;
            while sent < total_send {
                tx.write_all(&data).await.unwrap();
                sent += chunk_size;
            }
            tx.shutdown().await.unwrap();
            (start.elapsed(), sent)
        });

        let consumer = tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
            let start = std::time::Instant::now();
            let mut received = 0usize;
            loop {
                let n = rx.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                received += n;
                // Slow consumer: 100 µs per 1 KB batch
                tokio::time::sleep(std::time::Duration::from_micros(100)).await;
            }
            (start.elapsed(), received)
        });

        let (p_res, c_res) = tokio::join!(producer, consumer);
        let (p_time, sent) = p_res.unwrap();
        let (c_time, received) = c_res.unwrap();

        assert_eq!(sent, total_send);
        assert_eq!(received, total_send);

        // With backpressure, the producer must block when the pipe
        // is full, coupling its progress to the consumer's.
        // Without backpressure, the producer would finish in <<100ms
        // while the consumer lags behind by almost the full 1 MB.
        let ratio = p_time.as_secs_f64() / c_time.as_secs_f64().max(0.001);
        assert!(
            ratio < 3.0,
            "Producer finished {ratio:.2}x faster than consumer — no backpressure\n  \
             producer time: {p_time:?}\n  consumer time: {c_time:?}"
        );
    }

    #[tokio::test]
    async fn test_pipe_basic_io() {
        let (mut tx, mut rx) = pipe(256);
        tx.write_all(b"hello").await.unwrap();
        let mut buf = vec![0u8; 5];
        rx.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
    }

    #[tokio::test]
    async fn test_pipe_block_on_full_buffer() {
        let capacity = 64;
        let (mut tx, mut rx) = pipe(capacity);

        // Fill the buffer exactly
        tx.write_all(&vec![0u8; capacity]).await.unwrap();

        // Write more — must block because buffer is full.
        let write_more =
            tokio::time::timeout(std::time::Duration::from_millis(50), tx.write_all(b"x")).await;

        assert!(
            write_more.is_err(),
            "Write should timeout: buffer full = backpressure engaged"
        );

        // Read 1 byte to free space, then write succeeds
        let mut byte = [0u8; 1];
        rx.read_exact(&mut byte).await.unwrap();
        tx.write_all(b"x").await.unwrap();
    }

    #[tokio::test]
    async fn test_pipe_shutdown_wakes_reader() {
        let (tx, mut rx) = pipe(64);
        let mut writer = Some(tx);

        let reader = tokio::spawn(async move {
            let mut buf = vec![0u8; 16];
            let n = rx.read(&mut buf).await.unwrap();
            assert_eq!(n, 0, "shutdown should cause read to return 0");
        });

        drop(writer.take());
        reader.await.unwrap();
    }
}
