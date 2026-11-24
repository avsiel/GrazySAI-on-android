//! Android stubs.
//!
//! Native file dialogs on Android are provided by the system document picker
//! (SAF) and require a Java/Kotlin `Activity` plus a result callback. The
//! desktop build of GrazySAI uses `rfd` for dialogs; until an Android frontend
//! is wired up the calls simply report "cancelled" instead of failing to
//! compile. GrazySAI's Android entry point uses its own in-app file browser.

use crate::backend::{
    AsyncFilePickerDialogImpl, AsyncFileSaveDialogImpl, AsyncFolderPickerDialogImpl,
    AsyncMessageDialogImpl, DialogFutureType, FilePickerDialogImpl, FileSaveDialogImpl,
    FolderPickerDialogImpl, MessageDialogImpl,
};
use crate::{FileDialog, FileHandle, MessageDialog, MessageDialogResult};
use std::future::ready;
use std::path::PathBuf;

impl FilePickerDialogImpl for FileDialog {
    fn pick_file(self) -> Option<PathBuf> {
        None
    }
    fn pick_files(self) -> Option<Vec<PathBuf>> {
        None
    }
}

impl FileSaveDialogImpl for FileDialog {
    fn save_file(self) -> Option<PathBuf> {
        None
    }
}

impl FolderPickerDialogImpl for FileDialog {
    fn pick_folder(self) -> Option<PathBuf> {
        None
    }
    fn pick_folders(self) -> Option<Vec<PathBuf>> {
        None
    }
}

impl MessageDialogImpl for MessageDialog {
    fn show(self) -> MessageDialogResult {
        MessageDialogResult::Cancel
    }
}

impl AsyncFilePickerDialogImpl for FileDialog {
    fn pick_file_async(self) -> DialogFutureType<Option<FileHandle>> {
        Box::pin(ready(None))
    }
    fn pick_files_async(self) -> DialogFutureType<Option<Vec<FileHandle>>> {
        Box::pin(ready(None))
    }
}

impl AsyncFolderPickerDialogImpl for FileDialog {
    fn pick_folder_async(self) -> DialogFutureType<Option<FileHandle>> {
        Box::pin(ready(None))
    }
    fn pick_folders_async(self) -> DialogFutureType<Option<Vec<FileHandle>>> {
        Box::pin(ready(None))
    }
}

impl AsyncFileSaveDialogImpl for FileDialog {
    fn save_file_async(self) -> DialogFutureType<Option<FileHandle>> {
        Box::pin(ready(None))
    }
}

impl AsyncMessageDialogImpl for MessageDialog {
    fn show_async(self) -> DialogFutureType<MessageDialogResult> {
        Box::pin(ready(MessageDialogResult::Cancel))
    }
}
