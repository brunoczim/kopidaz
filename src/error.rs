use std::{error::Error as ErrorTrait, fmt};

/// The kind of an error that may happen handling storage.
#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorKind {
    /// An error happened at Sled level.
    Sled(sled::Error),
    /// Serialization or deserialization error.
    Serde(bincode::Error),
    /// A custom error, stored in a trait object.
    Custom(Box<dyn ErrorTrait + Send + Sync>),
}

impl ErrorKind {
    /// Returns this error kind as a trait object.
    pub fn as_dyn(&self) -> &(dyn ErrorTrait + 'static + Send + Sync) {
        match self {
            ErrorKind::Serde(error) => error,
            ErrorKind::Sled(error) => error,
            ErrorKind::Custom(error) => &**error,
        }
    }
}

impl From<bincode::Error> for ErrorKind {
    fn from(error: bincode::Error) -> Self {
        ErrorKind::Serde(error)
    }
}

impl From<sled::Error> for ErrorKind {
    fn from(error: sled::Error) -> Self {
        ErrorKind::Sled(error)
    }
}

impl From<Box<dyn ErrorTrait + Send + Sync>> for ErrorKind {
    fn from(error: Box<dyn ErrorTrait + Send + Sync>) -> Self {
        ErrorKind::Custom(error)
    }
}

/// An error that may happen handling storage.
#[derive(Debug)]
pub struct Error {
    /// Kind of the error. Wrapped in a Box to reduce the stack size.
    kind: Box<ErrorKind>,
}

impl Error {
    /// Creates an error from its kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind: Box::new(kind) }
    }

    /// Returns this error as a trait object.
    pub fn as_dyn(&self) -> &(dyn ErrorTrait + 'static + Send + Sync) {
        self.kind.as_dyn()
    }

    /// Returns the kind of the error.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.as_dyn())
    }
}

impl ErrorTrait for Error {
    fn source(&self) -> Option<&(dyn ErrorTrait + 'static)> {
        Some(self.as_dyn())
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind)
    }
}

impl From<bincode::Error> for Error {
    fn from(error: bincode::Error) -> Self {
        Self::new(ErrorKind::from(error))
    }
}

impl From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Self::new(ErrorKind::from(error))
    }
}

impl From<Box<dyn ErrorTrait + Send + Sync>> for Error {
    fn from(error: Box<dyn ErrorTrait + Send + Sync>) -> Self {
        Self::new(ErrorKind::from(error))
    }
}
