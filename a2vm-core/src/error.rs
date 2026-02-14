#[cfg(not(feature = "std"))]
use core::fmt;
#[cfg(feature = "std")]
use std::fmt;

#[cfg(feature = "std")]
use std::io;

#[derive(Debug)]
pub enum Error {
    #[cfg(feature = "std")]
    Io(io::Error),
    UnsupportedRomSize {
        actual: usize,
    },
    InvalidDiskSize {
        expected: usize,
        actual: usize,
    },
    InvalidDiskLocation {
        drive: usize,
        track: u8,
        sector: u8,
    },
    DiskNotLoaded,
    DiskWriteProtected,
    DiskDecodeFailed {
        track: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "std")]
            Error::Io(err) => write!(f, "{err}"),
            Error::UnsupportedRomSize { actual } => write!(
                f,
                "Unsupported ROM size: {actual} ({actual:#X}). Only Apple II / Apple II+ ROMs are supported (12K or 20K)."
            ),
            Error::InvalidDiskSize { expected, actual } => {
                write!(f, "DSK image must be {expected} bytes, got {actual}")
            }
            Error::InvalidDiskLocation {
                drive,
                track,
                sector,
            } => {
                write!(
                    f,
                    "invalid drive/track/sector: drive={drive} track={track} sector={sector}"
                )
            }
            Error::DiskNotLoaded => write!(f, "no disk loaded"),
            Error::DiskWriteProtected => write!(f, "disk is write protected"),
            Error::DiskDecodeFailed { track } => {
                write!(f, "failed to decode nibblized disk data on track {track}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(feature = "std")]
impl From<Error> for io::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Io(err) => err,
            Error::UnsupportedRomSize { .. }
            | Error::InvalidDiskSize { .. }
            | Error::InvalidDiskLocation { .. }
            | Error::DiskNotLoaded
            | Error::DiskWriteProtected
            | Error::DiskDecodeFailed { .. } => io::Error::new(io::ErrorKind::InvalidData, value),
        }
    }
}

#[cfg(feature = "std")]
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(not(feature = "std"))]
pub type Result<T> = core::result::Result<T, Error>;
