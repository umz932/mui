use core::{
    fmt::{self, Debug, Display, Formatter},
    error::Error
};

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