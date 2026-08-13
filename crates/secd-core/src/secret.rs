use std::sync::Once;

use zeroize::Zeroize;

/// Bytes that must never be printed, serialized, or deref-coerced.
pub struct Secret {
    inner: Vec<u8>,
}

impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        deny_core_dumps();
        lock_pages(&bytes);
        Self { inner: bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        let len = self.inner.len();
        let ptr = self.inner.as_mut_ptr();
        self.inner.zeroize();
        unlock_pages(ptr, len);
    }
}

impl Zeroize for Secret {
    fn zeroize(&mut self) {
        self.inner.zeroize();
    }
}

fn deny_core_dumps() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let lim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `lim` is a valid rlimit; RLIMIT_CORE 0 disables core dumps for this process.
        let _ = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &lim) };
    });
}

fn lock_pages(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let ptr = bytes.as_ptr();
    let len = bytes.len();
    // SAFETY: `ptr`/`len` are the live `Vec` allocation; mlock until `Drop`.
    let _ = unsafe { libc::mlock(ptr.cast(), len) };
    #[cfg(target_os = "linux")]
    {
        // SAFETY: same allocation; MADV_DONTDUMP keeps the pages out of core files.
        let _ = unsafe {
            libc::madvise(
                ptr.cast::<libc::c_void>().cast_mut(),
                len,
                libc::MADV_DONTDUMP,
            )
        };
    }
}

fn unlock_pages(ptr: *mut u8, len: usize) {
    if len == 0 {
        return;
    }
    // SAFETY: `ptr`/`len` are the allocation that `lock_pages` locked; contents already zeroized.
    let _ = unsafe { libc::munlock(ptr.cast(), len) };
}
