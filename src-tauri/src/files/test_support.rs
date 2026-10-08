//! A chosen-file opener for tests: plain paths, as a desktop opens them.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use tauri_plugin_fs::FilePath;

use super::picked::{Opener, PickedFile};

/// Opens paths with `std::fs`; `roots` stands in for a device without free paths. With
/// `documents`, an address `content://…/<name>` stands for the file `<name>` in that folder, as a
/// document provider would hand it over.
#[derive(Clone, Default)]
pub struct PathOpener {
    pub roots: Option<Vec<PathBuf>>,
    pub documents: Option<PathBuf>,
}

impl Opener for PathOpener {
    fn open(&self, file: FilePath, write: bool) -> io::Result<File> {
        let path = match (file, &self.documents) {
            (FilePath::Path(path), _) => path,
            (FilePath::Url(url), Some(folder)) if url.scheme() == "content" => {
                let name = url
                    .path_segments()
                    .and_then(|mut s| s.next_back())
                    .unwrap_or("");
                folder.join(name)
            }
            _ => return Err(io::Error::other("no document provider in tests")),
        };
        if write {
            File::create(path)
        } else {
            File::open(path)
        }
    }

    fn path_roots(&self) -> Option<Vec<PathBuf>> {
        self.roots.clone()
    }

    fn provider_name(&self, _uri: &str) -> Option<String> {
        Some("Bericht.pdf".to_string())
    }
}

/// A path as the dialog hands it over.
pub fn picked(path: &Path) -> PickedFile {
    PickedFile(path.to_string_lossy().into_owned())
}
