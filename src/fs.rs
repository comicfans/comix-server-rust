
use super::cache::CacheFsTree;
use super::cache::PathU8;
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
    watch_thread : std::thread::JoinHandle<()>
}

impl Fs{

    pub fn changed(self,path : &PathU8){

        let mut cache = self.rwlock.write().unwrap();

        cache.remove(path);
    }

    pub fn new(path: &PathU8, limit :usize){

        //TODO path convert to abs

        let mut ret = Fs{
            root: path,
            rwlock:std::sync::RwLock::new(CacheFsTree::new(limit)),
            watch_thread: std::thread::spawn(||{
            })
        };

                
        ret.watch_thread = std::thread::spawn(||{watch(&mut ret.rwlock)});


        //access root . root is special since it can be virtual root of all 
        //partition driver under windows
        //
        if path.is_empty() {
            
            /*
            if cfg!(windows) {

                let children= std::vec::new();
                for drive in windows.emuerate_drive ().iter(){
                    children.push(drive);
                }
                self.cache.add(sjdkf);
            }

            else{
            }
            */
                
            ret.access('/', PathU8::new(), &std::io::sink());

            return ret;
        }
    }

    fn access <W : std::io::Write>(&mut self, parent : &PathU8 ,left : &PathU8, w: &mut W){

        let segment = self.root;

        let f_stat=stat(path);

        if f.is_file {
            let binary = fread(f);

            match binary{
                Some(v)=>{
                    self.cache.add(path, binary);
                    return binary;
                },
                None(v)=>{
                    return String::From("read file failed");
                }
            };
        }

        // is dir
        let fs_res = readdir_r(path);

        let children = std::vec::Vec::new();

        for i in fs_res{
            children.push(i.name, i.is_file)
        }

        self.cache.set_children(path,children);
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
