use core::{
    convert::Infallible,
    mem::MaybeUninit,
    fmt::{self, Debug, Display, Formatter},
    error::Error
};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

use crate::MuiGuard;

pub(crate) const UNINIT_ERR_MSG: &str = "The value was not initialized in the initiator";

/// An error type for [`try_init`](crate::try_init).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitErr<E> {
    /// An error that returned in the initiator function
    ErrOnInit(E),
    /// An error that the value was not determined to be initialized during initialization
    NotInited
}

impl<E> InitErr<E> {
    /// Returns the inner error if `self` is `ErrOnInit` variant, panics otherwise.
    pub fn unwrap_init_err(self) -> E {
        match self {
            Self::ErrOnInit(err) => err,
            Self::NotInited => panic!("{UNINIT_ERR_MSG}")
        }
    }
}

impl<E: Display> Display for InitErr<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ErrOnInit(err) => write!(f, "An error was returned during initialization: {err}"),
            Self::NotInited => f.write_str(UNINIT_ERR_MSG)
        }
    }
}

impl<E: Error + 'static> Error for InitErr<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ErrOnInit(err) => Some(err),
            _ => None
        }
    }
}

/// A fallible version of [`init`].
/// 
/// Returns error if the initiator returns an error ([`InitErr::ErrOnInit`]) or the value is not flagged as initialized ([`InitErr::NotInited`]).  
/// If you want to panic on an uninitialized error, consider using [`InitErr::unwrap_init_err`] as below.
/// ```should_panic
/// use mui::{try_init, InitErr};
/// # struct SomeErr;
/// let data = try_init::<u32, SomeErr>(|guard| {
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

    if g.finish().is_ok() {
        Ok(unsafe { mui.assume_init() })
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
/// # use mui::init;
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
/// # use mui::init;
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
///     unsafe { (&raw mut (*ptr).title).write("some text".to_string()); }
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
///         title: "some text".to_string(),
///         list: vec![810, 114514, 1919]
///     }
/// );
/// ```
pub fn init<T>(f: impl FnOnce(&mut MuiGuard<'_, T>)) -> T {
    try_init::<_, Infallible>(|mui| {
        f(mui);
        Ok(())
    })
    .expect(UNINIT_ERR_MSG)
}

#[cfg(feature = "alloc")]
pub fn try_init_boxed<T, E>(f: impl FnOnce(&mut MuiGuard<'_, T>) -> Result<(), E>) -> Result<Box<T>, InitErr<E>> {
    let mut boxed = Box::new_uninit();
    let mut g = MuiGuard::new(boxed.as_mut());

    f(&mut g).map_err(InitErr::ErrOnInit)?;

    if g.finish().is_ok() {
        Ok(unsafe { boxed.assume_init() })
    } else {
        Err(InitErr::NotInited)
    }
}

#[cfg(feature = "alloc")]
pub fn init_boxed<T>(f: impl FnOnce(&mut MuiGuard<'_, T>)) -> Box<T> {
    try_init_boxed::<_, Infallible>(|g| {
        f(g);
        Ok(())
    }).unwrap()
}