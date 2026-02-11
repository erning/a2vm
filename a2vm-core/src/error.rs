use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    UnsupportedRomSize { actual: usize },
    InvalidDiskSize { expected: usize, actual: usize },
    InvalidDiskLocation { drive: usize, track: u8, sector: u8 },
    DiskNotLoaded,
    DiskWriteProtected,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<Error> for io::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Io(err) => err,
            Error::UnsupportedRomSize { .. }
            | Error::InvalidDiskSize { .. }
            | Error::InvalidDiskLocation { .. }
            | Error::DiskNotLoaded
            | Error::DiskWriteProtected => io::Error::new(io::ErrorKind::InvalidData, value),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
