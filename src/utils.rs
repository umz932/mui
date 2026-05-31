//! Utilities for in-place initialization
//!
#![allow(clippy::unit_arg)]

use core::{
    convert::Infallible,
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    mem::MaybeUninit,
};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::{
    boxed::Box,
    alloc::{dealloc, Layout}
};

use crate::{MuiGuard, MuiGuardSeq};

pub(crate) const UNINIT_ERR_MSG: &str = "The value was not initialized in the initiator";

/// An error type for [`try_init`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitErr<E> {
    /// An error that returned in the initiator function
    ErrOnInit(E),
    /// An error that the value was not determined to be initialized during initialization
    NotInited,
}

impl<E> InitErr<E> {
    /// Returns the inner error if `self` is `ErrOnInit` variant, panics otherwise.
    pub fn unwrap_init_err(self) -> E {
        match self {
            Self::ErrOnInit(err) => err,
            Self::NotInited => panic!("{UNINIT_ERR_MSG}"),
        }
    }
}

impl<E: Display> Display for InitErr<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ErrOnInit(err) => write!(f, "An error was returned during initialization: {err}"),
            Self::NotInited => f.write_str(UNINIT_ERR_MSG),
        }
    }
}

impl<E: Error + 'static> Error for InitErr<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ErrOnInit(err) => Some(err),
            _ => None,
        }
    }
}

/// A fallible variant of [`init`].
///
/// Returns error if the initiator returns an error ([`InitErr::ErrOnInit`]) or the value is not flagged as initialized ([`InitErr::NotInited`]).
/// If you want to panic on an uninitialized error, consider using [`InitErr::unwrap_init_err`] as below.
/// ```should_panic
/// use mui::utils::{try_init, InitErr};
/// # struct SomeErr;
/// let data: Result<u32, SomeErr> = try_init::<u32, SomeErr>(|guard| {
///     // left the MUI uninitialized.
///     Ok(())
/// }).map_err(InitErr::unwrap_init_err);
/// ```
#[inline]
pub fn try_init<T, E>(
    f: impl FnOnce(&mut MuiGuard<'_, T>) -> Result<(), E>,
) -> Result<T, InitErr<E>> {
    let mut mui = MaybeUninit::<T>::uninit();
    let mut g = MuiGuard::new(&mut mui);

    f(&mut g).map_err(InitErr::ErrOnInit)?;

    g.finish_init_err().map(|_| unsafe { mui.assume_init() })
}

/// Instantly creates an array by initializing its value element-by-element *via* [`MuiGuardSeq`] type.
///
/// This is useful as an alternative to [`core::array::try_from_fn`] which is now in an experimental state.
/// 
/// ```
/// # use core::convert::Infallible;
/// use mui::utils::{InitErr, try_init_ary};
///
/// fn try_from_fn<T, E, const N: usize>(mut f: impl FnMut(usize) -> Result<T, E>) -> Result<[T; N], E> {
///     try_init_ary(|mui| {
///         for (i, elm) in mui.iter_mut().enumerate() {
///             elm.write(f(i)?);
///         };
///         Ok(())
///     }).map_err(InitErr::unwrap_init_err)
/// }
/// #
/// # #[derive(Debug, PartialEq)]
/// # struct SomeErr;
/// #
/// # // Successful case
/// # let arr1: Result<[usize; 5], Infallible> = try_from_fn(|i| Ok(i * i));
/// #
/// # // Failing case
/// # let arr2: Result<[usize; 5], SomeErr> = try_from_fn(|_| Err(SomeErr));
/// #
/// # assert_eq!(arr1, Ok([0, 1, 4, 9, 16]));
/// # assert_eq!(arr2, Err(SomeErr));
/// ```
/// 
/// # Note
/// [`core::array::from_fn`] may achieve the similar functionality of its infallible variant.
pub fn try_init_ary<T, E, const N: usize>(
    f: impl FnOnce(&mut MuiGuardSeq<'_, T, N>) -> Result<(), E>,
) -> Result<[T; N], InitErr<E>> {
    let mut mui = MaybeUninit::<[T; N]>::uninit();
    let mut g = MuiGuardSeq::new(&mut mui);

    f(&mut g).map_err(InitErr::ErrOnInit)?;

    g.finish_init_err().map(|_| unsafe { mui.assume_init() })
}

/// A fallible variant of [`init_boxed`]
/// and a variant of [`try_init`] for a boxed value.
#[cfg(feature = "alloc")]
pub fn try_init_boxed<T, E>(
    f: impl FnOnce(&mut MuiGuard<'_, T>) -> Result<(), E>,
) -> Result<Box<T>, InitErr<E>> {
    let mut boxed = Box::new_uninit();
    let mut g = MuiGuard::new(boxed.as_mut());

    f(&mut g).map_err(InitErr::ErrOnInit)?;

    g.finish_init_err().map(|_| unsafe { boxed.assume_init() })
}

/// Creates a boxed slice initialized element-by-element *via* a slice of [`MuiGuard`].
/// 
/// Early-returns a blank slice `Ok(box [])` if `size` is 0.
/// 
/// # Example
/// ```
/// use mui::utils::{InitErr, try_init_slice};
/// 
/// #[derive(Debug, PartialEq)]
/// struct OverThousand;
/// 
/// fn checked_expn(num: u32, exp: u32) -> Result<u32, OverThousand> {
///     let ret = num.pow(exp);
///     if ret <= 1_000 {
///         Ok(ret)
///     } else {
///         Err(OverThousand)
///     }
/// }
/// 
/// let arr1 = try_init_slice::<_, OverThousand>(6, |muis| {
///     for (i, elm) in muis.iter_mut().enumerate() {
///         elm.write(checked_expn((i+1) as u32, 3)?);
///     }
///     Ok(())
/// });
/// 
/// let arr2 = try_init_slice(6, |muis| {
///     for (i, elm) in muis.iter_mut().enumerate() {
///         elm.write(checked_expn((i+1) as u32, 4)?);
///     }
///     Ok(())
/// });
/// 
/// assert_eq!(arr1, Ok(vec![1, 8, 27, 64, 125, 216].into_boxed_slice()));
/// assert_eq!(arr2, Err(InitErr::ErrOnInit(OverThousand)));
/// ```
/// 
/// # Note
/// The code below may achieve the similar functionality of its infallible variant:
/// (equivalent to the code including an experimental function: `Vec::from_fn(size, f).into_boxed_slice()`.)
/// ```ignore
/// (0..size).map(f).collect::<Vec<_>>().into_boxed_slice()
/// ```
#[cfg(feature = "alloc")]
pub fn try_init_slice<T, E>(
    size: usize,
    f: impl FnOnce(&mut [MuiGuard<'_, T>]) -> Result<(), E>,
) -> Result<Box<[T]>, InitErr<E>> {
    if size == 0 { return Ok(Box::new([])) }
    
    let mut boxed_ary = Box::new_uninit_slice(size);

    let mut boxed_mui_slice = {
        let mut dst = Box::new_uninit_slice(size);

        // SAFETY:
        // All values of `dst` will be in an initialized state and 
        // out-of-bounds accesses will never happen since boxed_ary.len() == dst.len().
        for (i, mui) in boxed_ary.iter_mut().enumerate() {
            dst[i].write(MuiGuard::new(mui));
        }

        unsafe { dst.assume_init() }
    };

    f(boxed_mui_slice.as_mut()).map_err(InitErr::ErrOnInit)?;

    if boxed_mui_slice.iter().all(MuiGuard::is_inited) {
        let ptr = Box::into_raw(boxed_mui_slice);
        
        let layout = Layout::array::<MuiGuard<'_, T>>(size).unwrap();
        unsafe {
            dealloc(ptr as *mut u8, layout);
            Ok(boxed_ary.assume_init())
        }
    } else {
        Err(InitErr::NotInited)
    }
}

/// Instantly creates a value with the given initiator.
///
/// # Panics
/// Panics if the value has not been determined to be initialized in the initiator.
///
/// # Example
/// ## A basic usage
/// ```
/// use mui::utils::init;
///
/// let data = init(|guard| {
///
///     guard.write(42);
/// });
///
/// assert_eq!(
///     data,
///     42
/// );
/// ```
///
/// ## Field-by-field initialization
/// ```
/// # extern crate alloc;
/// # use alloc::{ vec::Vec, string::String };
/// use mui::utils::init;
///
/// #[derive(Debug, PartialEq)]
/// struct Hoge {
///     title: String,
///     list: Vec<u32>,
/// }
///
/// let hoge = init::<Hoge>(|guard| {
///
///     let ptr = guard.as_mut_ptr();
///
///     unsafe { (&raw mut (*ptr).title).write("hoge".to_string()); }
///     unsafe { (&raw mut (*ptr).list).write(vec![810, 114514, 1919]); }
///
///     // Because the guard cannot detect initialization of the value via the pointer,
///     // you need to assert the inner value to be initialized.
///     unsafe { guard.assume_init_mut() };
/// });
///
/// assert_eq!(
///     hoge,
///     Hoge {
///         title: "hoge".to_string(),
///         list: vec![810, 114514, 1919]
///     }
/// );
/// ```
pub fn init<T>(f: impl FnOnce(&mut MuiGuard<'_, T>)) -> T {
    try_init::<_, Infallible>(|mui| Ok(f(mui))).expect(UNINIT_ERR_MSG)
}

/// A variant of [`init`] for a boxed value.
///
/// This first creates an uninitialized value on the heap, then initializes it with the given
/// initiator function.
#[cfg(feature = "alloc")]
pub fn init_boxed<T>(f: impl FnOnce(&mut MuiGuard<'_, T>)) -> Box<T> {
    try_init_boxed::<_, Infallible>(|mui| Ok(f(mui))).expect(UNINIT_ERR_MSG)
}