use p2ptransfer_core::network::alpn::PauseFlag;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// PauseFlag should start in the not-paused, not-cancelled state.
/// wait_if_paused() should return immediately with false.
#[tokio::test]
async fn test_pause_flag_starts_clear() {
    let flag = PauseFlag::new();
    let result = flag.wait_if_paused().await;
    assert!(!result, "wait_if_paused must return false when not paused and not cancelled");
}

/// pause() → wait_if_paused should block → resume() wakes it up returning false.
#[tokio::test]
async fn test_pause_then_resume() {
    let flag = PauseFlag::new();
    flag.pause();

    let flag2 = flag.clone();
    // Use an atomic to confirm the task is actually running and blocked
    let started = Arc::new(AtomicBool::new(false));
    let started2 = started.clone();

    let jh = tokio::spawn(async move {
        started2.store(true, Ordering::SeqCst);
        flag2.wait_if_paused().await
    });

    // Yield until the task starts (it will immediately block on wait_if_paused)
    for _ in 0..100 {
        if started.load(Ordering::SeqCst) { break; }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    // Give a little more time for it to enter the blocking select
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!jh.is_finished(), "Task should be blocked while paused");

    flag.resume();

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        jh,
    ).await
        .expect("Task did not complete in time")
        .expect("Task panicked");

    assert!(!result, "wait_if_paused must return false after resume (not cancelled)");
}

/// cancel() while blocked on wait_if_paused should unblock and return true.
#[tokio::test]
async fn test_cancel_while_paused() {
    let flag = PauseFlag::new();
    flag.pause();

    let flag2 = flag.clone();
    let started = Arc::new(AtomicBool::new(false));
    let started2 = started.clone();

    let jh = tokio::spawn(async move {
        started2.store(true, Ordering::SeqCst);
        flag2.wait_if_paused().await
    });

    for _ in 0..100 {
        if started.load(Ordering::SeqCst) { break; }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!jh.is_finished(), "Task should be blocked while paused");

    flag.cancel();

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        jh,
    ).await
        .expect("Task did not complete in time")
        .expect("Task panicked");

    assert!(result, "wait_if_paused must return true after cancel");
}

/// cancel() before wait_if_paused — should return true immediately without blocking.
#[tokio::test]
async fn test_cancel_before_wait() {
    let flag = PauseFlag::new();
    flag.cancel();

    // Give the watch channel time to process the send
    tokio::task::yield_now().await;

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        flag.wait_if_paused(),
    ).await
        .expect("wait_if_paused must return immediately when already cancelled");
    assert!(result, "wait_if_paused must return true when already cancelled");
}

/// wait_for_cancel() resolves once cancel() is called.
#[tokio::test]
async fn test_wait_for_cancel_resolves() {
    let flag = PauseFlag::new();

    let flag2 = flag.clone();
    let started = Arc::new(AtomicBool::new(false));
    let started2 = started.clone();

    let jh = tokio::spawn(async move {
        started2.store(true, Ordering::SeqCst);
        flag2.wait_for_cancel().await;
    });

    for _ in 0..100 {
        if started.load(Ordering::SeqCst) { break; }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!jh.is_finished(), "wait_for_cancel should block before cancel");

    flag.cancel();

    tokio::time::timeout(
        std::time::Duration::from_millis(500),
        jh,
    ).await
        .expect("wait_for_cancel did not complete in time")
        .expect("Task panicked");
}

/// wait_for_cancel() already cancelled — must return immediately.
#[tokio::test]
async fn test_wait_for_cancel_already_cancelled() {
    let flag = PauseFlag::new();
    flag.cancel();

    // Yield to let the watch channel propagate
    tokio::task::yield_now().await;

    tokio::time::timeout(
        std::time::Duration::from_millis(200),
        flag.wait_for_cancel(),
    ).await
        .expect("wait_for_cancel must resolve immediately when already cancelled");
}

/// Multiple resumes are idempotent; flag stays in running state.
#[tokio::test]
async fn test_multiple_resumes_idempotent() {
    let flag = PauseFlag::new();
    flag.resume();
    flag.resume();
    flag.resume();

    let result = flag.wait_if_paused().await;
    assert!(!result, "After multiple resumes, flag should be in running/not-cancelled state");
}

/// Cancel is idempotent — calling twice does not panic or block.
#[tokio::test]
async fn test_cancel_idempotent() {
    let flag = PauseFlag::new();
    flag.cancel();
    flag.cancel(); // must not panic
    tokio::task::yield_now().await;
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        flag.wait_if_paused(),
    ).await
        .expect("wait_if_paused must not block when cancelled")
        ;
    assert!(result, "Flag must stay cancelled");
}

/// pause() → cancel() → resume() — result must still be true (cancelled wins).
/// Note: the current PauseFlag implementation uses separate channels for
/// pause and cancel, so a resume() after cancel() clears the pause state
/// but does NOT clear the cancel state. wait_if_paused() checks cancel first.
#[tokio::test]
async fn test_cancel_wins_over_resume() {
    let flag = PauseFlag::new();
    flag.pause();
    flag.cancel();
    flag.resume(); // clears pause, but cancel state persists

    tokio::task::yield_now().await;

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        flag.wait_if_paused(),
    ).await
        .expect("wait_if_paused must return immediately (not block) after cancel+resume");
    assert!(result, "Cancelled flag must return true even if resume is called after cancel");
}
