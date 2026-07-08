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
