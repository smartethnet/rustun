use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub(crate) enum FrameError {
    TooShort,
    Invalid,
}

impl std::error::Error for FrameError {}

impl Display for FrameError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FrameError::TooShort => "stream ended early".fmt(fmt),
            FrameError::Invalid => "invalid frame".fmt(fmt),
        }
    }
}