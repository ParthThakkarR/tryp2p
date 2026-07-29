use p2ptransfer_core::compress::zstd;
use p2ptransfer_core::transfer::hasher;
use std::ffi::CString;
use std::os::raw::c_char;

/// Returns the p2ptransfer core version string.
/// Caller must free with `p2ptransfer_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn p2ptransfer_version() -> *mut c_char {
    let version = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    version.into_raw()
}

/// Free a string previously returned by p2ptransfer FFI.
///
/// # Safety
/// `s` must be a valid pointer from `p2ptransfer_version` or another p2ptransfer FFI string return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2ptransfer_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Compute BLAKE3 hash of a byte buffer.
/// Returns hex-encoded hash string; caller must free with `p2ptransfer_free_string`.
///
/// # Safety
/// `data` must be a valid pointer for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2ptransfer_hash(data: *const u8, len: usize) -> *mut c_char {
    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    let hash = hasher::blake3_hash(slice);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    CString::new(hex).unwrap().into_raw()
}

/// Compress data with Zstd. Returns allocated buffer with compressed data.
/// First 4 bytes of output = original uncompressed length as u32 LE.
/// Caller must free with `p2ptransfer_free_buffer`.
///
/// # Safety
/// `data` must be a valid pointer for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2ptransfer_compress(
    data: *const u8,
    len: usize,
    level: i32,
    out_len: *mut usize,
) -> *mut u8 {
    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    let compressed = zstd::compress(slice, level).unwrap_or_else(|_| slice.to_vec());
    let mut out = (compressed.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&compressed);
    unsafe {
        *out_len = out.len();
    }
    out.leak().as_mut_ptr()
}

/// Free a buffer returned by p2ptransfer FFI.
///
/// # Safety
/// `ptr` must be a valid pointer from `p2ptransfer_compress` or another p2ptransfer FFI buffer return.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2ptransfer_free_buffer(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len, len);
        }
    }
}

/// Helper to convert C string to Rust str slice safely.
unsafe fn c_str_to_str<'a>(s: *const c_char) -> &'a str {
    if s.is_null() {
        return "";
    }
    unsafe { std::ffi::CStr::from_ptr(s).to_str().unwrap_or("") }
}

/// Discover peers on local network for `duration_secs`.
/// Returns JSON array of discovered peers as a C string (free with `p2ptransfer_free_string`).
///
/// # Safety
/// `device_name` must be a null-terminated C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2ptransfer_discover_peers(
    device_name: *const c_char,
    duration_secs: u64,
) -> *mut c_char {
    let name = unsafe { c_str_to_str(device_name) };
    let device = if name.is_empty() { "MobileDevice" } else { name };

    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(r) => r,
        Err(_) => return CString::new("[]").unwrap().into_raw(),
    };

    let result = rt.block_on(async {
        if let Ok(mut discovery) = p2ptransfer_core::p2p::discovery::DiscoveryService::new(device.to_string(), 9877, 9876).await {
            let _ = discovery.start().await;
            tokio::time::sleep(std::time::Duration::from_secs(duration_secs.max(1))).await;
            let peers = discovery.get_peers().await;
            discovery.stop().await;
            let json = serde_json::to_string(&peers).unwrap_or_else(|_| "[]".into());
            json
        } else {
            "[]".to_string()
        }
    });

    CString::new(result).unwrap().into_raw()
}

