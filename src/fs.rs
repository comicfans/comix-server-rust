extern crate libarchive;

use libarchive::reader::Builder;
use libarchive::reader::Reader;

use super::cache::CacheFsTree;
use super::cache::PathU8;
use super::cache::NodeContents;

use notify::{RecommendedWatcher, Watcher, RecursiveMode};
use std::sync::mpsc::channel;
use std::time::Duration;


fn prefix_virtual_root (input : &PathU8) -> PathU8 {

    if cfg!(windows) {

        let with_virtual_root = PathU8::new(); 
        return with_virtual_root;
    }
    

}


pub const DEFAULT_LIMIT : usize = 256 * 1024 * 1024;

pub fn watch(rwlock : &mut std::sync::RwLock<CacheFsTree> ){
    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher : Result<RecommendedWatcher, notify::Error> = Watcher::new(tx, Duration::from_secs(2));

    if let Err(_) = watcher {
        return;
    }


    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    let res = watcher.unwrap().watch("/home/comicfans/Downloads", RecursiveMode::Recursive);

    if res.is_err() {
        return;
    }

    // This is a simple loop, but you may want to use more complex logic here,
    // for example to handle I/O.
    loop {
        match rx.recv() {
            Ok(event) => println!("{:?}", event),
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

pub struct Fs{
    root : std::path::PathBuf,
    rwlock: std::sync::RwLock<CacheFsTree>,
    watch_thread : std::thread::JoinHandle<()>,
    ar_handle : None

}



impl Fs{

    pub fn changed(self,path : &PathU8){

        let mut cache = self.rwlock.write().unwrap();

        cache.remove(path);
    }

    pub fn new(path: &PathU8, limit :usize)->Result<Fs>{

        
        let abs_root = std::fs::cancanlize(path);

        let mut ret = Fs{
            root: abs_root,
            rwlock:std::sync::RwLock::new(CacheFsTree::new(limit)),
            watch_thread: std::thread::spawn(||{
            })
        };



                
        ret.watch_thread = std::thread::spawn(||{watch(&mut ret.rwlock)});


        //access root . root is special since it can be virtual root of all 
        //partition driver under windows
        
        let _=ret.access(ret.root ,PathU8::from(""))?;

        return ret;
        
    }

    fn access_fs <W : std::io::Write>(&mut self, relative: &PathU8)->Result<NodeContents>{

        let fullpath = self.root.join(relative);

        let attr = std::fs::metadata(fullpath)?;
        
        if attr.is_dir() {

            let children = std::vec::Vec::new();

            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                children.push(entry.file_name());
            }

            let cache = self.rwlock.write().unwrap();

            cache.set_children(relative, children);

            cache.get(relative, w);

            return;
        }


        // is file
        // first test if this is image file
        
        let supported_image = vec!["jpg","jpeg","bmp","gif","png"];

        let ext = path.extension_name();

        // is image file from filesystem, no need to cache
        if ext.ToLower() in supported_image {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            writer.write(reader.ReadAll());
            return;
        }


        // try to open as archive 
        return self.access_in_archive( fullpath, PathU8::from(""),writer);
    }
    

    pub fn read <W : std::io::Write>(self,path: &PathU8, writer: &mut writer){

        let res = self.cache.get(path);
        if res==Ok {
            return res;
        }

        //no such entry, read in fs. file or dir?
        //
        //
        
        return self.access(root+path);
    }

}
