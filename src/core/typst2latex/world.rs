use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::fs;
use std::io;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source, VirtualPath, RootedPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::Library;
use typst::LibraryExt;
use typst::utils::LazyHash;
use typst::World;
use typst::ecow::EcoString;

pub struct TylaxWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    main_id: FileId,
    root: PathBuf,
    sources: Mutex<HashMap<FileId, Source>>,
    files: Mutex<HashMap<FileId, Bytes>>,
}

impl TylaxWorld {
    pub fn new(main_path: &Path, root: &Path) -> Self {
        // Anchor the main file to an absolute path under `root` before deriving
        // its virtual path. Passing a relative `main_path` with an absolute
        // `root` used to make `strip_prefix` fail and silently fall back to
        // `/main.typ`, converting the wrong file.
        let abs_main = if main_path.is_absolute() {
            main_path.to_path_buf()
        } else {
            root.join(main_path)
        };
        // Compute virtual path for the main file relative to root.
        let vpath_str = match abs_main.strip_prefix(root) {
            Ok(rel) => format!("/{}", rel.display()),
            // Main file is outside `root`; fall back to its own file name
            // rather than a hardcoded `main.typ` so we target the right file.
            Err(_) => match abs_main.file_name() {
                Some(name) => format!("/{}", Path::new(name).display()),
                None => "/main.typ".to_string(),
            },
        };
        let vpath = VirtualPath::new(vpath_str).unwrap();
        let main_id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));

        Self {
            library: LazyHash::new(typst::Library::builder().build()),
            book: LazyHash::new(FontBook::new()),
            main_id,
            root: root.to_path_buf(),
            sources: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
        }
    }

    fn resolve_path(&self, id: FileId) -> FileResult<PathBuf> {
        if let VirtualRoot::Package(spec) = id.root() {
            let home = std::env::var("HOME").unwrap_or_default();
            let mut package_dir = PathBuf::from(&home).join(".cache/typst/packages").join(spec.namespace.as_str()).join(spec.name.as_str()).join(spec.version.to_string());
            if !package_dir.exists() {
                package_dir = PathBuf::from(&home).join(".local/share/typst/packages").join(spec.namespace.as_str()).join(spec.name.as_str()).join(spec.version.to_string());
            }
            let p = id.vpath().realize(&package_dir).map_err(|_| FileError::NotFound(PathBuf::from(id.vpath().get_without_slash())))?;
            return Ok(p);
        }

        let p = id.vpath().realize(&self.root).map_err(|_| FileError::NotFound(PathBuf::from(id.vpath().get_without_slash())))?;
        Ok(p)
    }

    fn io_to_file_error(e: io::Error, path: PathBuf) -> FileError {
        match e.kind() {
            io::ErrorKind::NotFound => FileError::NotFound(path),
            io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other(Some(EcoString::from(e.to_string()))),
        }
    }
}

impl World for TylaxWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let mut sources = self.sources.lock().unwrap();
        if let Some(source) = sources.get(&id) {
            return Ok(source.clone());
        }

        if let VirtualRoot::Package(spec) = id.root() {
            if spec.name == "curryst" && id.vpath().get_without_slash() == "curryst.typ" {
                let shim = include_str!("packages/curryst.typ");
                let source = Source::new(id, shim.to_string());
                sources.insert(id, source.clone());
                return Ok(source);
            }
        }

        let path = self.resolve_path(id)?;
        let text = fs::read_to_string(&path)
            .map_err(|e| Self::io_to_file_error(e, path))?;
        
        let source = Source::new(id, text);
        sources.insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let mut files = self.files.lock().unwrap();
        if let Some(bytes) = files.get(&id) {
            return Ok(bytes.clone());
        }

        let path = self.resolve_path(id)?;
        let data = fs::read(&path)
            .map_err(|e| Self::io_to_file_error(e, path))?;
        
        let bytes = Bytes::new(data);
        files.insert(id, bytes.clone());
        Ok(bytes)
    }

    fn font(&self, _index: usize) -> Option<Font> {
        None
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}
