//! A simple drop guard for [`MaybeUninit`]
//!
//! Because the inner value of [`MaybeUninit`] (*hereinafter abbreviated as* MUI) never gets dropped unless it is converted to the concrete type (by [`assume_init`](`MaybeUninit::assume_init`)) or manually dropped (by [`assume_init_drop`](`MaybeUninit::assume_init_drop`)),
//! a memory leak happens and the data on the memory goes out of management if the program panics during initialization (This is the specification of unions).  
//! This crate provides a simple guard type to avoid this problem and reduce some unsafeness.
//!
//! # Example
//! ## A basic usage
//! ```
//! # use mui::MuiGuard;
//! # use core::mem::MaybeUninit;
//! let data = {
//!     let mut data = MaybeUninit::<u32>::uninit();
//! 
//!     let mut guard = MuiGuard::new(&mut data);
//!
//!     guard.write(42);
//!     guard.finish().unwrap();
//!
//!     unsafe { data.assume_init() }
//! };
//!
//! assert_eq!(
//!     data,
//!     42
//! );
//! ```
//!
//! ## Field-by-field initialization
//! ```
//! # extern crate alloc;
//! # use core::mem::MaybeUninit;
//! # use alloc::{ vec::Vec, string::String };
//! # use mui::MuiGuard;
//! #[derive(Debug, PartialEq)]
//! struct Hoge {
//!     title: String,
//!     list: Vec<u32>,
//! }
//!
//! let hoge = {
//!     let mut hoge = MaybeUninit::<Hoge>::uninit();
//!     let mut guard = MuiGuard::new(&mut hoge);
//!
//!     let ptr = guard.as_mut_ptr();
//!
//!     unsafe { (&raw mut (*ptr).title).write("some text".to_string()); }
//!     unsafe { (&raw mut (*ptr).list).write(vec![810, 114514, 1919]); }
//!
//!     // Because the guard cannot detect initialization of the value via the pointer,
//!     // validated finalization would fail regardless of the true state of the MUI.
//!     guard.finish_unchecked();
//! 
//!     unsafe { hoge.assume_init() }
//! };
//!
//! assert_eq!(
//!     hoge,
//!     Hoge {
//!         title: "some text".to_string(),
//!         list: vec![810, 114514, 1919]
//!     }
//! );
//! ```
#![no_std]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
use core::{
    fmt::{self, Debug, Formatter},
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr,
};

#[cfg(feature = "zeroize")]
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A simple wrapper type of a mutable [`MaybeUninit`] (MUI) reference to clean its inner data when
/// dropped unexpectedly (e.g. on a panic) during initialization.
#[must_use = "The value of the underlying MUI will be discarded on drop of this guard"]
pub struct MuiGuard<'a, T> {
    mui: &'a mut MaybeUninit<T>,
    inited: bool,
}

impl<'a, T> MuiGuard<'a, T> {
    /// Creates a new guard
    pub const fn new(mui: &'a mut MaybeUninit<T>) -> Self {
        Self { mui, inited: false }
    }

    /// Initializes the underlying MUI with the given value.
    ///
    /// # Warning
    /// As with the original [`MaybeUninit::write`], this overwrites the previous content of the underlying MUI without dropping it.
    /// This should not be called after the MUI has been in an initialized state, unless you intend to avoid triggering the destructor.
    ///
    /// You can use [`Self::get_mut`] to overwrite an initialized value.
    /// ```
    /// # extern crate alloc;
    /// # use core::mem::MaybeUninit;
    /// # use alloc::string::String;
    /// # use mui::MuiGuard;
    /// #
    /// let mut mui = MaybeUninit::<String>::uninit();
    /// let mut guard = MuiGuard::new(&mut mui);
    ///
    /// // First initialization
    /// guard.write("first text".to_string());
    ///
    /// // Overwriting the initialized object
    /// // guard.write("second text".to_string()); <- This results in a memory leak.
    /// *(guard.get_mut().unwrap()) = "second text".to_string();
    ///
    /// // Finalization
    /// guard.finish().unwrap_or_else(|_| unreachable!("The value has been already initialized as above"));
    ///
    /// assert_eq!(
    ///     unsafe { mui.assume_init() },
    ///     "second text"
    /// );
    /// ```
    pub const fn write(&mut self, value: T) -> &mut T {
        self.inited = true;
        self.mui.write(value)
    }

    /// Gets a raw pointer to the content of the MUI.
    pub const fn as_ptr(&self) -> *const T {
        self.mui.as_ptr()
    }

    /// Gets a mutable raw pointer to the content of the MUI.
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.mui.as_mut_ptr()
    }

    /// Deems the underlying value to be properly initialized.
    ///
    /// # Safety
    /// The caller **MUST** guarantee the underlying value is properly initialized
    /// with the byte representation valid for the type of the value before calling this function,
    /// or this **immediately** results in an undefined behavior.
    ///
    /// This also sets the inner flag of this guard for determining if the underlying value is properly initialized,
    /// thus a misuse of this disrupts the safety assumption of this guard.
    pub const unsafe fn assume_init_mut(&mut self) -> &mut T {
        self.inited = true;
        unsafe { self.mui.assume_init_mut() }
    }

    /// Returns if the underlying MUI has been determined to be initialized.
    pub const fn is_inited(&self) -> bool {
        self.inited
    }

    /// Returns a shared ref to the initialized content of the MUI,
    /// or `None` if it's not determined to be initialized.
    ///
    /// ```
    /// # use mui::MuiGuard;
    /// # use core::mem::MaybeUninit;
    /// let mut mui = MaybeUninit::<u32>::uninit();
    /// let mut guard = MuiGuard::new(&mut mui);
    ///
    /// assert_eq!(guard.get_ref(), None);
    ///
    /// guard.write(42);
    /// assert_eq!(guard.get_ref(), Some(&42));
    /// ```
    pub fn get_ref(&self) -> Option<&T> {
        self.inited.then(|| unsafe { self.mui.assume_init_ref() })
    }

    /// Returns a shared ref to the initialized content of the MUI,
    /// or `None` if it's not determined to be initialized.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.inited.then(|| unsafe { self.mui.assume_init_mut() })
    }

    /// Drops the content of the MUI.
    ///
    /// No-op if the value is not determined to be initialized.
    pub fn drop_val(&mut self) -> bool {
        if self.inited {
            // SAFETY: The value has been determined to be initialized.
            unsafe {
                self.mui.assume_init_drop();
            }
            self.inited = false;
            true
        } else {
            false
        }
    }

    /// Consumes this guard and enable use of the underlying MUI.
    ///
    /// Because this guard holds the mutable reference to the MUI,
    /// it is mandatory to free it before using its instance (e.g. applying [`assume_init`](`MaybeUninit::assume_init`)).  
    /// However, just dropping this guard drops the MUI content, which may lead to a use-after-free issue.
    ///
    /// This function suppresses the destructor to avoid the problem.
    ///
    /// # Safety
    /// This returns `Err(self)` instead of finalizing the guard if the underlying value is not ensured to be initialized.
    /// **The destructor is not suppressed to run in such a situation, and still discards the inner value on drop.**
    ///
    /// Ignoring this may cause an access to a malformed value, which leads to an undefined behavior.
    #[must_use = "Returns `Err(self)` if the underlying value is not initialized. Ignoring this may cause an access to an invalid value"]
    pub fn finish(self) -> Result<(), Self> {
        if self.inited {
            self.finish_unchecked();
            Ok(())
        } else {
            Err(self)
        }
    }

    /// Consumes this guard and enable use of the underlying MUI  
    /// without validating if it has been properly initialized.
    ///
    /// This function has the same effect as [`forget(self)`](core::mem::forget), and the equivalent [`let _ = ManuallyDrop::new(self);`](ManuallyDrop).
    #[inline]
    pub fn finish_unchecked(self) {
        let _ = ManuallyDrop::new(self);
    }
}

impl<T> Debug for MuiGuard<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MuiGuard")
            .field("mui", &self.mui)
            .field("inited", &self.inited)
            .finish()
    }
}

impl<'a, T> From<&'a mut MaybeUninit<T>> for MuiGuard<'a, T> {
    fn from(value: &'a mut MaybeUninit<T>) -> Self {
        Self::new(value)
    }
}

/// When the program panicked before the underlying MUI is converted to the concrete (droppable) type,
/// the MUI value will be dropped and, if `zeroize` feature is enabled, zeroized away subsequent to drop of this guard by the panic handler.
///
/// This destructor must be disabled when the guard is dropped to use the underlying MUI.
impl<T> Drop for MuiGuard<'_, T> {
    fn drop(&mut self) {
        if self.inited {
            unsafe {
                // SAFETY: The value has been determined to be initialized.
                self.mui.assume_init_drop();
            }
        }

        #[cfg(feature = "zeroize")]
        // FIXED(2026/5/25): This must be operated after dropping the value because 
        // 
        self.mui.zeroize();
    }
}

#[cfg(feature = "zeroize")]
/// # Warning
/// This simply fills the memory area for the underlying value with zeroes without dropping the previous value
/// (*via* the [`Zeroize`](https://docs.rs/zeroize/latest/zeroize/trait.Zeroize.html#impl-Zeroize-for-MaybeUninit%3CZ%3E) impl of `MaybeUninit<Z>`).
///
/// # Safety
/// Access (read, move and drop) to the inner value after this operation may cause UB, because it breaks the invariants of the type `T`.
/// The underlying value is flagged as uninitialized after the operation.
impl<T> Zeroize for MuiGuard<'_, T> {
    fn zeroize(&mut self) {
        self.mui.zeroize();

        // Clear inited flag because all zero byte repr is not valid for some types
        self.inited = false;
    }
}

#[cfg(feature = "zeroize")]
impl<T> ZeroizeOnDrop for MuiGuard<'_, T> {}

/// A sequence of [`MuiGuard`] for MUI treating an array of data.
/// 
/// This is a slight wrapper of `[MuiGuard<'a, T>; N]` to simplify the code.
/// 
/// # Example
/// ## Element-by-element initialization
/// ```
/// # use core::mem::MaybeUninit;
/// # use mui::SeqGuard;
/// let array = {
///     let mut array = MaybeUninit::<[u32; 8]>::uninit();
/// 
///     let mut guard_seq = SeqGuard::new(&mut array);
/// 
///     for (i, guard) in guard_seq.iter_mut().enumerate() {
///         guard.write((i * i) as u32);
///     }
/// 
///     guard_seq.finish().unwrap();
/// 
///     unsafe { array.assume_init() }
/// };
/// 
/// assert_eq!(
///     array,
///     [0, 1, 4, 9, 16, 25, 36, 49]
/// );
/// ```
#[must_use = "The values of the underlying MUI are discarded on drop"]
pub struct SeqGuard<'a, T, const N: usize>([MuiGuard<'a, T>; N]);

impl<'a, T, const N: usize> SeqGuard<'a, T, N> {
    /// Creates a new sequential guard from those convertible to the referenced array of MUIs
    /// (e.g. [`MaybeUninit<[T; N]>`](https://doc.rust-lang.org/stable/std/mem/union.MaybeUninit.html#impl-AsMut%3C[MaybeUninit%3CT%3E;+N]%3E-for-MaybeUninit%3C[T;+N]%3E)).
    #[inline]
    pub fn new(mui_seq: &'a mut impl AsMut<[MaybeUninit<T>; N]>) -> Self {
        SeqGuard(mui_seq.as_mut().each_mut().map(MuiGuard::new))
    }

    /// Gets a shared reference to the MUI (if initialized) at the specified index.
    /// 
    /// Returns `None` if the MUI at the index is not initialized or the index is out of bounds.
    #[inline]
    pub fn get_ref(&self, idx: usize) -> Option<&T> {
        self.0.get(idx).and_then(MuiGuard::get_ref)
    }

    /// Gets a mutable reference to the MUI (if initialized) at the specified index.
    /// 
    /// Returns `None` if the MUI at the index is not initialized or the index is out of bounds.
    #[inline]
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.0.get_mut(idx).and_then(MuiGuard::get_mut)
    }

    /// Checks if all of the values have been properly initialized.
    #[inline]
    pub fn is_all_inited(&self) -> bool {
        self.iter().all(MuiGuard::is_inited)
    }

    /// Consumes this guard and enable use of the underlying MUI(s) without validation.
    /// 
    /// Similar to [`MuiGuard::finish_unchecked`].
    #[inline]
    pub fn finish_unchecked(self) {
        let _ = ManuallyDrop::new(self);
    }

    /// Consumes this guard and enable use of the underlying MUI(s).
    /// 
    /// This validates if all of the values have been properly initialized,
    /// and returns `Err(self)` instead of finalizing the guard if not.
    ///
    /// Similar to [`MuiGuard::finish`]
    #[inline]
    #[must_use = "Returns `Err(self)` if the underlying value is not initialized. Ignoring this may cause an access to an invalid value"]
    pub fn finish(self) -> Result<(), Self> {
        if self.is_all_inited() {
            self.finish_unchecked();
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl<'a, T, const N: usize> Debug for SeqGuard<'a, T, N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SeqGuard")
         .field(&self.0)
         .finish()
    }
}

impl<'a, T, const N: usize> Deref for SeqGuard<'a, T, N> {
    type Target = [MuiGuard<'a, T>; N];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T, const N: usize> DerefMut for SeqGuard<'a, T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, T, const N: usize> From<MuiGuard<'a, [T; N]>> for SeqGuard<'a, T, N> {
    #[inline]
    fn from(value: MuiGuard<'a, [T; N]>) -> Self {
        // Disable drop to avoid double drop.
        let value = ManuallyDrop::new(value);

        // SAFETY: Holding ownership of `value`, and it won't be used anymore.
        let mui = unsafe { ptr::read(&value.mui) };

        Self::new(mui)
    }
}

pub mod utils;

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use super::utils::init;
    use std::{
        sync::{Arc, Mutex},
        thread::spawn,
    };

    #[derive(Clone)]
    struct DropChecker(Arc<Mutex<bool>>);

    impl DropChecker {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(false)))
        }

        fn get(&self) -> bool {
            *(match self.0.lock() {
                Ok(lock) => lock,
                Err(poisoned) => poisoned.into_inner(),
            })
        }
    }

    impl Drop for DropChecker {
        fn drop(&mut self) {
            let mut lock = self.0.lock().unwrap();
            *lock = true;
        }
    }

    #[test]
    fn drop_on_panic() {
        let drop_chk = DropChecker::new();

        let chk = drop_chk.clone();
        let thread = spawn(move || {
            init::<DropChecker>(|mui| {
                mui.write(chk);
                panic!();
            })
        });

        // The thread intentionally panics.
        let _ = thread.join();
        assert!(drop_chk.get());
    }

    #[test]
    fn drop_on_panic_for_seq() {
        let drop_chk = DropChecker::new();

        let chk = drop_chk.clone();
        let thread = spawn(move || {
            let mut chker = MaybeUninit::<[DropChecker; 3]>::uninit();
            let mut guard = SeqGuard::new(&mut chker);

            guard[0].write(chk);
            panic!();
        });

        let _ = thread.join();
        assert!(drop_chk.get());
    }
}
