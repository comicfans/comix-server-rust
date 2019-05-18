extern crate relative_path;
extern crate tree_magic;
extern crate pretty_bytes;

use pretty_bytes::converter::convert;

use super::cache::{is_archive, join_may_empty, ArchiveCache, FileOrMem, NodeContents, PathU8};
use relative_path::RelativePathBuf;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::{mpsc::channel, Mutex};
use std::time::Duration;
use std::{fs, fs::File, io::BufReader};

use std::io::{Error, ErrorKind, Read, Write};

pub const DEFAULT_MEM_LIMIT: usize = 256 * 1024 * 1024;
pub const DEFAULT_ARCHIVE_LIMIT: usize = 20;

pub struct Fs {
    root: std::path::PathBuf,
}

unsafe impl Send for Fs {}
unsafe impl Sync for Fs {}

const MIME_TEXT: &str = "Content-Type: text/plain; charset=utf-8";

impl<'a> NodeContents<'a> {
    pub fn write_to(&self, w: &mut Write) -> std::io::Result<String> {
        match self {
            NodeContents::File(bin) => {
                w.write_all(bin)?;
                trace!("file size {}",convert(bin.len() as f64));
                return Ok(tree_magic::from_u8(&bin));
            }
            NodeContents::Dir(dirs) => {
                for d in dirs {
                    w.write_all(d.as_bytes())?;
                    w.write_all(b"\n")?;
                }
                return Ok(String::from(MIME_TEXT));
            }
        }
    }
}

impl Fs {
    pub fn start_watch(&self, cache: &Mutex<ArchiveCache>) {
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Automatically select the best implementation for your platform.
        // You can also access each implementation directly e.g. INotifyWatcher.
        let watcher: Result<RecommendedWatcher, notify::Error> =
            Watcher::new(tx, Duration::from_secs(2));

        if let Err(_) = watcher {
            return;
        }

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        let res = watcher
            .unwrap()
            .watch("/home/comicfans/Downloads", RecursiveMode::Recursive);

        if res.is_err() {
            return;
        }

        // This is a simple loop, but you may want to use more complex logic here,
        // for example to handle I/O.
        loop {
            match rx.recv() {
                Ok(event) => {
                    let path = PathU8::new();

                    let res = path.strip_prefix(self.root.clone());

                    if let Ok(rel) = res {
                        let lock = cache.lock();
                        lock.unwrap().invalid_path(&PathU8::from(rel));
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    }

    pub fn new(path: &PathU8) -> std::io::Result<Fs> {
        debug!("path {:?}",path);
        let abs_root = std::fs::canonicalize(path)?;

        debug!("fs normalized root {:?}", abs_root);

        let ret = Fs { root: abs_root };

        //access root . root is special since it can be virtual root of all
        //partition driver under windows

        return Ok(ret);
    }

    fn try_in_archive(
        &self,
        cache: &Mutex<ArchiveCache>,
        virtual_path: &PathU8,
        archive_path: &PathU8,
        left: &PathU8,
        w: &mut Write,
    ) -> std::io::Result<String> {
        trace!(
            "try in archive {:?}, as virtual_path {:?}, left {:?}",
            archive_path,
            virtual_path,
            left
        );

        let mut lock = cache.lock().unwrap();

        let res = lock.set_archive(virtual_path, &archive_path)?;

        if left.to_str().unwrap().is_empty() {
            trace!("no left, use archive {:?} result", virtual_path);
            return res.write_to(w);
        }

        match lock.quick_try(&join_may_empty(virtual_path, left)) {
            Some(result) => {
                return result.write_to(w);
            }
            _ => {}
        }

        match lock.slow_try(&join_may_empty(virtual_path, left)) {
            Err(er) => {
                return Err(er);
            }
            Ok(contents) => {
                return contents.write_to(w);
            }
        }
    }

    fn direct_file_access(&self, path: &PathU8, w: &mut Write) -> std::io::Result<String> {
        trace!("access {:?} as direct file", path);
        // is image file from filesystem, no need to cache
        let file = File::open(path)?;

        let mut buf: Vec<u8> = Vec::new();
        BufReader::new(file).read_to_end(&mut buf)?;

        w.write_all(&buf)?;
        return Ok(tree_magic::from_filepath(path));
    }

    fn try_access(
        &self,
        cache: &Mutex<ArchiveCache>,
        path: &PathU8,
        w: &mut Write,
    ) -> std::io::Result<String> {
        trace!("try access {:?}", path);

        let mut try_path = self.root.join(path);

        let mut left = PathU8::from("");

        let mut first = true;

        for i in 0..path.iter().count() + 1 {
            if i != 0 {
                let comp = PathU8::from(try_path.iter().last().unwrap());
                try_path.pop();

                left = join_may_empty(&PathU8::from(comp), &left);
            }

            trace!("try {:?}, left {:?}", try_path, left);

            let attr = try_path.metadata();

            if attr.is_err() {
                trace!("can't stat {:?}", try_path);

                //can not read file attr
                //no such file , or no permission
                //just continue
                first = false;
                continue;
            }

            //attr ok is file or dir

            if attr.unwrap().is_dir() {
                if i != 0 {
                    //path has comp left, but parent is a dir (not archive)
                    //so this path can not be a file inside archive
                    //(we already tried fullpath as file and it is not readable)
                    return Err(Error::new(ErrorKind::NotFound, "path is not dir"));
                }
                //only first try to test if target is dir
                //otherwise it must be archive + inner path
                trace!("read {:?} as dir", try_path);
                for entry in try_path.read_dir()? {
                    let entry = entry?;

                    w.write_all(entry.file_name().into_string().unwrap().as_bytes())?;
                    w.write_all(b"\n")?;
                }
                return Ok(MIME_TEXT.to_string());
            }

            // is a file (at previous time)

            let is_archive = is_archive(&try_path, FileOrMem::Path(&try_path));

            if !is_archive {
                return self.direct_file_access(&try_path, w);
            }

            let mut rel_to_archive = path.clone();

            for _ in left.iter() {
                rel_to_archive.pop();
            }

            return self.try_in_archive(cache, &rel_to_archive, &try_path, &left, w);
        }

        return Err(Error::new(
            ErrorKind::NotFound,
            path.to_str().unwrap().to_owned() + "not matched in filesystem",
        ));
    }

    pub fn read<W: std::io::Write>(
        &self,
        cache: &Mutex<ArchiveCache>,
        path: &PathU8,
        writer: &mut W,
    ) -> std::io::Result<String> {
        if path.is_absolute() {
            trace!("access abs path {:?} forbidden", path);
            return Err(Error::new(
                ErrorKind::NotFound,
                "can not access absolute path".to_owned() + path.to_str().unwrap(),
            ));
        }

        let canonicalized = RelativePathBuf::new().to_path(path);

        trace!("access {:?}", canonicalized);

        if canonicalized.starts_with("..") {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                std::format!("cannot access beyond root {:?}", self.root),
            ));
        }

        //test cache first

        {
            let mut lock = cache.lock().unwrap();
            let res = lock.quick_try(&canonicalized);

            if let Some(node_contents) = res {
                trace!("cache hit for {:?}", path);
                return node_contents.write_to(writer);
            }
            trace!("cache no hit for {:?}", path);
        }

        //no such entry, read in fs. file or dir?
        //
        //

        return self.try_access(cache, path, writer);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn run2() {
        let f = Fs::new(&PathU8::from(std::path::PathBuf::from(env!(
            "CARGO_MANIFEST_DIR"
        ))))
        .unwrap();

        let mut cache = Mutex::new(ArchiveCache::new(100, 100));

        let mut c1 = std::io::Cursor::new(Vec::new());

        assert!(f
            .read(
                &cache,
                &PathU8::from("tests/nested.zip/test.zip/dir/under_dir"),
                &mut c1
            )
            .is_ok());
    }

    #[test]
    fn test_can(){

        let canonicalized = RelativePathBuf::new().to_path("../../");
        println!("{:?}",canonicalized);
    }

    #[test]
    fn run() {
        let f = Fs::new(&PathU8::from(std::path::PathBuf::from(env!(
            "CARGO_MANIFEST_DIR"
        ))))
        .unwrap();

        let mut cache = Mutex::new(ArchiveCache::new(100, 100));

        let mut c1 = std::io::Cursor::new(Vec::new());

        assert!(f.read(&cache, &PathU8::from(""), &mut c1).is_ok());

        let mut c2 = std::io::Cursor::new(Vec::new());

        assert!(f.read(&cache, &PathU8::from("tests"), &mut c2).is_ok());

        let mut c3 = std::io::Cursor::new(Vec::new());

        assert!(f
            .read(&cache, &PathU8::from("not exists"), &mut c3)
            .is_err());

        let mut c4 = std::io::Cursor::new(Vec::new());

        assert!(f
            .read(&cache, &PathU8::from("tests/test.zip"), &mut c4)
            .is_ok());

        let mut c5 = std::io::Cursor::new(Vec::new());
        assert!(f
            .read(&cache, &PathU8::from("tests/test.zip/under_root"), &mut c5)
            .is_ok());

        let mut c6 = std::io::Cursor::new(Vec::new());
        assert!(f
            .read(
                &cache,
                &PathU8::from("tests/test.zip/dir/under_dir"),
                &mut c6
            )
            .is_ok());
    }

}
