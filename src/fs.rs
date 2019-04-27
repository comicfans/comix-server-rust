
use super::cache::CacheFsTree;
use notify::{RecommendedWatcher, Watcher, RecursiveMode};
use std::sync::mpsc::channel;
use std::time::Duration;



pub fn watch() -> notify::Result<()> {
    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(2))?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch("/home/comicfans/Downloads", RecursiveMode::Recursive)?;

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

    c: cache::CacheFsTree,

    watch_thread : std::thread,

    mutex: mutex,
}

impl Fs{

    pub fn changed(self,path : std::path::PathBuf){

        lock(mutex);

        self.cache.remove(path);
    }

    pub fn new(path: std::path::PathBuf){
        assert (path.is_absolute());

        Fs{
            root: path,
            cache: CacheFsTree::new(),
            watch_thread: thread,
            mutex:mutex
        }

        //access root . root is special since it can be virtual root of all 
        //partition driver under windows
        //
        if path.empty() {
            
            if system.multiroot() {

                Vec<String> children= std::vec::new();
                for drive in windows.emuerate_drive ().iter(){
                    children.push(drive);
                }
                self.cache.add(sjdkf);
            }

            else{
                self.access('/');
            }

            return;
        }

    }

    fn access(&mut self, path : PathU8){

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

        self.set_children(path,children);

        return children`
    }
    

    pub fn get(self,path: std::path::PathBuf)-> Result<NodeResult,String>{

        lock(mutex);

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
