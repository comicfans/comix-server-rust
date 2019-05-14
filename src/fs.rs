use super::cache::ArchiveCache;
use super::cache::PathU8;
use crossbeam;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::{mpsc::channel, Mutex};
use std::time::Duration;
use std::{fs, fs::File, io::BufReader};

use std::io::{Error, ErrorKind, Read, Write};

pub const DEFAULT_MEM_LIMIT: usize = 256 * 1024 * 1024;
pub const DEFAULT_ARCHIVE_LIMIT: usize = 20;

pub fn watch(fs: &Fs, cache: &mut ArchiveCache) {
    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher: Result<RecommendedWatcher, notify::Error> =
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

                let res = path.strip_prefix(fs.root.clone());

                if let Ok(rel) = res {
                    cache.invalid_path(&PathU8::from(rel));
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

pub struct Fs {
    root: std::path::PathBuf,
}

unsafe impl Send for Fs {}
unsafe impl Sync for Fs {}

impl Fs {
    pub fn start_watch(&self, cache: &Mutex<ArchiveCache>) {
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Automatically select the best implementation for your platform.
        // You can also access each implementation directly e.g. INotifyWatcher.
        let mut watcher: Result<RecommendedWatcher, notify::Error> =
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

    pub fn new(path: &PathU8, memory_limit: usize, archive_limit: usize) -> std::io::Result<Fs> {
        let abs_root = std::fs::canonicalize(path)?;

        let mut ret = Fs { root: abs_root };

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
    ) -> std::io::Result<usize> {
        let mut lock = cache.lock().unwrap();

        let res = lock.set_archive(virtual_path, &archive_path)?;

        if left.to_str().unwrap().is_empty() {
            return res.write_to(w);
        }

        match lock.get(&left) {
            Err(er) => {
                return Err(er);
            }
            Ok(contents) => {
                return contents.write_to(w);
            }
        }
    }

    fn direct_file_access(&self, path: &PathU8, w: &mut Write) -> std::io::Result<usize> {
        // is image file from filesystem, no need to cache
        let file = File::open(path)?;

        let mut buf: Vec<u8> = Vec::new();
        BufReader::new(file).read_to_end(&mut buf)?;

        w.write_all(&buf)?;
        return Ok(0);
    }

    fn try_access(
        &self,
        cache: &Mutex<ArchiveCache>,
        path: &PathU8,
        w: &mut Write,
    ) -> std::io::Result<usize> {
        let mut try_path = self.root.join(path);

        let mut left = PathU8::from("");

        let mut first = true;

        for comp in path.iter() {
            let attr = std::fs::metadata(try_path.clone());

            if attr.is_err() {
                //can not read file attr
                //no such file , or no permission
                //just continue
                first = false;
                try_path.pop();
                left = PathU8::from(comp).join(left);
                continue;
            }

            // has attr

            if first & attr.unwrap().is_dir() {
                //only first try to test if target is dir
                //otherwise it must be archive + inner path
                for entry in fs::read_dir(try_path)? {
                    let entry = entry?;

                    w.write_all(entry.file_name().into_string().unwrap().as_bytes())?;
                    w.write_all(b"\n")?;
                }
                return Ok(0);
            }

            if !first {
                try_path.pop();
                left = PathU8::from(comp).join(left);
            }

            // is file
            // first test if this is image file
            //

            first = false;

            // is a file (at previous time)

            let archive_exts = vec!["zip", "cbz", "rar", "cbr", "tar", "7z"];

            let ext = try_path.extension().unwrap().to_str();

            if !archive_exts.contains(&ext.unwrap()) && left.to_str().unwrap().is_empty() {
                return self.direct_file_access(&try_path, w);
            }

            //no ext , try to open as archive

            return self.try_in_archive(cache, path, &try_path, &left, w);
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
    ) -> std::io::Result<usize> {
        let canonicalized = std::fs::canonicalize(path)?;

        if path.is_absolute() {
            return Err(Error::new(
                ErrorKind::NotFound,
                "can not access absolute path".to_owned() + path.to_str().unwrap(),
            ));
        }

        //test cache first

        {
            let mut lock = cache.lock().unwrap();
            let res = lock.quick_try(&canonicalized);

            if let Some(node_contents) = res {
                return node_contents.write_to(writer);
            }
        }

        //no such entry, read in fs. file or dir?
        //
        //

        return self.try_access(cache, path, writer);
    }
}
