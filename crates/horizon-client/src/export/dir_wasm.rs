//! Browser stub for `export::dir`: offscreen video/PNG export is unavailable on wasm32.
use super::{ExportSpec, Reservation};
use serde::Serialize;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug)]
pub enum DirError {
    InvalidName,
    Exists,
    Folder(io::Error),
}

impl fmt::Display for DirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => f.write_str("invalid export name"),
            Self::Exists => f.write_str("export folder already exists"),
            Self::Folder(err) => write!(f, "could not create export folder: {err}"),
        }
    }
}

impl std::error::Error for DirError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Folder(err) => Some(err),
            Self::InvalidName | Self::Exists => None,
        }
    }
}

pub struct ExportFolder {
    path: PathBuf,
}

pub(super) struct ExportDestination<'a> {
    _name: &'a str,
}

#[derive(Debug, Serialize)]
pub struct ExportManifest<'a> {
    pub spec: &'a ExportSpec,
    pub state: &'a str,
    pub frames_written: u32,
    pub build: &'a str,
    pub gpu: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ManifestState {
    Pending = 0,
    Publishing,
    Published,
    Cancelled,
    Failed,
}

impl ManifestState {
    fn from_byte(value: u8) -> Self {
        match value {
            0 => Self::Pending,
            1 => Self::Publishing,
            2 => Self::Published,
            3 => Self::Cancelled,
            4 => Self::Failed,
            _ => unreachable!("invalid manifest publication state"),
        }
    }
}

#[derive(Default)]
pub struct ManifestPublication(AtomicU8);

impl ManifestPublication {
    pub fn state(&self) -> ManifestState {
        ManifestState::from_byte(self.0.load(Ordering::Acquire))
    }

    pub fn cancel(&self) -> ManifestState {
        match self.0.compare_exchange(
            ManifestState::Pending as u8,
            ManifestState::Cancelled as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => ManifestState::Cancelled,
            Err(state) => ManifestState::from_byte(state),
        }
    }

    #[doc(hidden)]
    pub fn publish(&self, rename: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
        self.0
            .compare_exchange(
                ManifestState::Pending as u8,
                ManifestState::Publishing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| io::Error::from(io::ErrorKind::Interrupted))?;
        let result = rename();
        let state = if result.is_ok() {
            ManifestState::Published
        } else {
            ManifestState::Failed
        };
        self.0.store(state as u8, Ordering::Release);
        result
    }
}

fn unsupported() -> io::Error {
    io::Error::from(io::ErrorKind::Unsupported)
}

impl<'a> ExportDestination<'a> {
    pub(super) fn open(_game_dir: &Path, _name: &'a str) -> Result<Self, DirError> {
        Err(DirError::Folder(unsupported()))
    }

    pub(super) fn check_absent(&self) -> Result<(), DirError> {
        Err(DirError::Folder(unsupported()))
    }

    pub(super) fn stage(&self) -> io::Result<Staging<'_>> {
        Err(unsupported())
    }

    pub(super) fn create(
        &self,
        _charge: Option<Reservation<'_>>,
    ) -> Result<ExportFolder, DirError> {
        Err(DirError::Folder(unsupported()))
    }
}

impl ExportFolder {
    pub fn create(_game_dir: &Path, _name: &str) -> Result<Self, DirError> {
        Err(DirError::Folder(unsupported()))
    }

    pub fn create_charged(
        _game_dir: &Path,
        _name: &str,
        _charge: Reservation<'_>,
    ) -> Result<Self, DirError> {
        Err(DirError::Folder(unsupported()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn create_new(&self, _file: &str) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn check_absent(&self, _name: &str) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn stage(&self) -> io::Result<Staging<'_>> {
        Err(unsupported())
    }

    pub fn write_manifest(&self, _manifest: &ExportManifest<'_>) -> io::Result<()> {
        Err(unsupported())
    }

    pub fn write_manifest_tracked(
        &self,
        _manifest: &ExportManifest<'_>,
        _publication: &ManifestPublication,
    ) -> io::Result<()> {
        Err(unsupported())
    }
}

pub(super) struct Staging<'a> {
    _marker: std::marker::PhantomData<&'a ()>,
}

impl Staging<'_> {
    pub(super) fn create_new<'file>(
        &'file self,
        _name: &'file str,
    ) -> io::Result<StagedFile<'file>> {
        Err(unsupported())
    }
}

pub(super) struct StagedFile<'a> {
    pub(super) file: File,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl StagedFile<'_> {
    pub(super) fn publish(&mut self) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn publish_to(&mut self, _folder: &ExportFolder) -> io::Result<()> {
        Err(unsupported())
    }
}
