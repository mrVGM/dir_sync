use std::{convert::Infallible, fmt::Display, net::AddrParseError, path::StripPrefixError, sync::mpsc::{RecvError, SendError}, time::SystemTimeError};

pub trait Error: std::error::Error + Send {}

#[derive(Debug)]
pub enum GenericError {
    GenericError(Box<dyn std::error::Error + Send + 'static>),
    CustomError(String)
}

pub fn new_custom_error(message: &str) -> GenericError {
    GenericError::CustomError(message.into())
}

impl Display for GenericError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericError(e) => write!(f, "GenericError({:?})", e),
            Self::CustomError(s) => write!(f, "CustomError({})", s)
        }
    }
}

impl std::error::Error for GenericError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GenericError::GenericError(boxed_err) => {
                let err = boxed_err.as_ref();
                Some(err)
            }
            _ => None
        }
    }
}

impl<T: Error + 'static> From<T> for GenericError {
    fn from(value: T) -> Self {
        let b = Box::new(value);
        GenericError::GenericError(b)
    }
}

impl Error for std::io::Error {}
impl Error for serde_json::Error {}
impl Error for StripPrefixError {}
impl Error for Infallible {}
impl Error for RecvError {}
impl<T: Send> Error for SendError<T> {}
impl Error for AddrParseError {}
impl Error for SystemTimeError {}
