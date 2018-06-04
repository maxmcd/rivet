use glib;
use gst;
use std::io;

#[derive(Debug)]
pub enum Error {
    GlibError(glib::Error),
    IoError(io::Error),
    GlibBoolError(glib::BoolError),
    GstStateChangeError(gst::StateChangeError),
    Empty,
}

impl From<glib::Error> for Error {
    fn from(error: glib::Error) -> Self {
        Error::GlibError(error)
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IoError(error)
    }
}

impl From<glib::BoolError> for Error {
    fn from(error: glib::BoolError) -> Self {
        Error::GlibBoolError(error)
    }
}

impl From<gst::StateChangeError> for Error {
    fn from(error: gst::StateChangeError) -> Self {
        Error::GstStateChangeError(error)
    }
}

impl From<()> for Error {
    fn from(_none: ()) -> Self {
        Error::Empty
    }
}
