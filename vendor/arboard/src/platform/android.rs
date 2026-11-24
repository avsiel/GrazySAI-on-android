//! Android clipboard stub.
//!
//! The system clipboard on Android can be reached through JNI
//! (`android.content.ClipboardManager`), but GrazySAI's desktop clipboard code
//! only needs a working API during the port. Keeping the stub in `arboard`
//! lets all desktop modules compile unchanged.

use crate::common::Error;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub(crate) struct Clipboard;

impl Clipboard {
	pub(crate) fn new() -> Result<Self, Error> {
		Ok(Self)
	}
}

pub(crate) struct Get<'clipboard>(&'clipboard Clipboard);

impl<'clipboard> Get<'clipboard> {
	pub(crate) fn new(clipboard: &'clipboard mut Clipboard) -> Self {
		Self(clipboard)
	}

	pub(crate) fn text(self) -> Result<String, Error> {
		Err(Error::ClipboardNotSupported)
	}

	#[cfg(feature = "image-data")]
	pub(crate) fn image(self) -> Result<crate::ImageData<'static>, Error> {
		Err(Error::ClipboardNotSupported)
	}

	pub(crate) fn html(self) -> Result<String, Error> {
		Err(Error::ClipboardNotSupported)
	}

	pub(crate) fn file_list(self) -> Result<Vec<PathBuf>, Error> {
		Err(Error::ClipboardNotSupported)
	}
}

pub(crate) struct Set<'clipboard>(&'clipboard mut Clipboard);

impl<'clipboard> Set<'clipboard> {
	pub(crate) fn new(clipboard: &'clipboard mut Clipboard) -> Self {
		Self(clipboard)
	}

	pub(crate) fn text(self, _text: Cow<'_, str>) -> Result<(), Error> {
		Err(Error::ClipboardNotSupported)
	}

	pub(crate) fn html(
		self,
		_html: Cow<'_, str>,
		_alt: Option<Cow<'_, str>>,
	) -> Result<(), Error> {
		Err(Error::ClipboardNotSupported)
	}

	#[cfg(feature = "image-data")]
	pub(crate) fn image(self, _image: crate::ImageData<'_>) -> Result<(), Error> {
		Err(Error::ClipboardNotSupported)
	}

	pub(crate) fn file_list(self, _paths: &[impl AsRef<Path>]) -> Result<(), Error> {
		Err(Error::ClipboardNotSupported)
	}
}

pub(crate) struct Clear<'clipboard>(&'clipboard mut Clipboard);

impl<'clipboard> Clear<'clipboard> {
	pub(crate) fn new(clipboard: &'clipboard mut Clipboard) -> Self {
		Self(clipboard)
	}

	pub(crate) fn clear(self) -> Result<(), Error> {
		Err(Error::ClipboardNotSupported)
	}
}
