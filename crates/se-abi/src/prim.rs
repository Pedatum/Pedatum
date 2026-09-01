//! Primitives that may cross a `.so` boundary.
//!
//! Every one of these is `#[repr(C)]` and owns nothing. A `Str` or `Slice`
//! handed to the host points into the module's own image, so it is valid
//! exactly as long as that library stays loaded — the host copies before it
//! ever lets a module go.

use core::marker::PhantomData;

/// Bumped when the shape of anything in this crate changes. A module built
/// against a different major is refused rather than trusted.
pub const ABI_MAJOR: u32 = 1;
pub const ABI_MINOR: u32 = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbiVersion {
    pub major: u32,
    pub minor: u32,
}

impl AbiVersion {
    pub const CURRENT: AbiVersion = AbiVersion { major: ABI_MAJOR, minor: ABI_MINOR };
    /// Same major is compatible; the host may be newer in minor.
    pub fn accepts(self, module: AbiVersion) -> bool {
        self.major == module.major && self.minor >= module.minor
    }
}

/// A borrowed UTF-8 string with a stable layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}

impl Str {
    pub const EMPTY: Str = Str { ptr: core::ptr::NonNull::<u8>::dangling().as_ptr(), len: 0 };

    pub const fn new(s: &'static str) -> Str {
        Str { ptr: s.as_ptr(), len: s.len() }
    }

    /// # Safety
    /// The owning library must still be loaded.
    pub unsafe fn as_str<'a>(&self) -> &'a str {
        if self.len == 0 {
            return "";
        }
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.ptr, self.len))
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl From<&'static str> for Str {
    fn from(s: &'static str) -> Str {
        Str::new(s)
    }
}

// Safe to move across threads: it is a plain pointer pair the host only reads
// while holding the library alive.
unsafe impl Send for Str {}
unsafe impl Sync for Str {}

/// A borrowed slice with a stable layout.
#[repr(C)]
pub struct Slice<T> {
    pub ptr: *const T,
    pub len: usize,
    _own: PhantomData<*const T>,
}

impl<T> Clone for Slice<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Slice<T> {}

impl<T> Slice<T> {
    pub const fn empty() -> Slice<T> {
        Slice { ptr: core::ptr::NonNull::<T>::dangling().as_ptr(), len: 0, _own: PhantomData }
    }

    pub const fn new(s: &'static [T]) -> Slice<T> {
        Slice { ptr: s.as_ptr(), len: s.len(), _own: PhantomData }
    }

    /// Point at memory the *host* owns, for the handful of calls that hand
    /// data the other way.
    ///
    /// # Safety
    /// `ptr` must be valid for `len` elements for as long as the result lives.
    pub const unsafe fn from_raw(ptr: *const T, len: usize) -> Slice<T> {
        Slice { ptr, len, _own: PhantomData }
    }

    /// # Safety
    /// The owning library must still be loaded.
    pub unsafe fn as_slice<'a>(&self) -> &'a [T] {
        if self.len == 0 {
            return &[];
        }
        core::slice::from_raw_parts(self.ptr, self.len)
    }
}

unsafe impl<T: Sync> Send for Slice<T> {}
unsafe impl<T: Sync> Sync for Slice<T> {}

/// FNV-1a. Used for the structural hashes that make the bundle contract
/// checkable rather than merely documented.
pub const fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

pub const fn mix(a: u64, b: u64) -> u64 {
    (a ^ b).wrapping_mul(0x0000_0100_0000_01b3)
}
