use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct BandwidthSnapshot {
    pub bytes_per_second: f64,
    pub total_bytes: u64,
    pub elapsed_secs: f64,
}

pub struct BandwidthMonitor {
    total_bytes: Arc<AtomicU64>,
    start_time: Instant,
    window_bytes: Arc<AtomicU64>,
    window_start: Arc<std::sync::Mutex<Instant>>,
    window_duration: Duration,
}

impl BandwidthMonitor {
    pub fn new(window_duration: Duration) -> Self {
        Self {
            total_bytes: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
            window_bytes: Arc::new(AtomicU64::new(0)),
            window_start: Arc::new(std::sync::Mutex::new(Instant::now())),
            window_duration,
        }
    }

    pub fn record_bytes(&self, n: u64) {
        self.total_bytes.fetch_add(n, Ordering::Relaxed);
        self.window_bytes.fetch_add(n, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> BandwidthSnapshot {
        let total_bytes = self.total_bytes.load(Ordering::Relaxed);
        let elapsed = self.start_time.elapsed();

        let window_bytes = self.window_bytes.load(Ordering::Relaxed);
        let mut window_start = self.window_start.lock().unwrap();
        let window_elapsed = window_start.elapsed();

        let bytes_per_second = if window_elapsed > Duration::ZERO {
            if window_elapsed >= self.window_duration {
                let rate = window_bytes as f64 / window_elapsed.as_secs_f64();
                self.window_bytes.store(0, Ordering::Relaxed);
                *window_start = Instant::now();
                rate
            } else {
                window_bytes as f64 / window_elapsed.as_secs_f64()
            }
        } else {
            0.0
        };

        BandwidthSnapshot {
            bytes_per_second,
            total_bytes,
            elapsed_secs: elapsed.as_secs_f64(),
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.total_bytes.store(0, Ordering::Relaxed);
        self.window_bytes.store(0, Ordering::Relaxed);
        *self.window_start.lock().unwrap() = Instant::now();
    }
}

pub struct RateLimiter {
    max_bytes_per_second: u64,
    accumulated: Arc<std::sync::Mutex<(f64, Instant)>>,
}

impl RateLimiter {
    pub fn new(max_bytes_per_second: u64) -> Self {
        Self {
            max_bytes_per_second,
            accumulated: Arc::new(std::sync::Mutex::new((0.0, Instant::now()))),
        }
    }

    pub fn allow(&self, bytes: u64) -> bool {
        let mut state = self.accumulated.lock().unwrap();
        let elapsed = state.1.elapsed().as_secs_f64();
        state.0 = (state.0 - (self.max_bytes_per_second as f64 * elapsed)).max(0.0);
        state.1 = Instant::now();
        state.0 += bytes as f64;
        state.0 <= self.max_bytes_per_second as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandwidth_monitor_records_bytes() {
        let monitor = BandwidthMonitor::new(Duration::from_secs(1));
        monitor.record_bytes(1024);
        monitor.record_bytes(2048);
        assert_eq!(monitor.total_bytes(), 3072);
    }

    #[test]
    fn test_bandwidth_snapshot() {
        let monitor = BandwidthMonitor::new(Duration::from_secs(1));
        monitor.record_bytes(1024);
        let snap = monitor.snapshot();
        assert_eq!(snap.total_bytes, 1024);
        assert!(snap.elapsed_secs > 0.0);
    }

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(1024);
        assert!(limiter.allow(512));
        assert!(limiter.allow(512));
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(1024);
        assert!(limiter.allow(1024));
        assert!(!limiter.allow(1));
    }

    #[test]
    fn test_bandwidth_reset() {
        let monitor = BandwidthMonitor::new(Duration::from_secs(1));
        monitor.record_bytes(9999);
        monitor.reset();
        assert_eq!(monitor.total_bytes(), 0);
    }
}
