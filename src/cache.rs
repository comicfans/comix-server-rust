extern crate lru;

use std::path::PathBuf;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::string::String;

use lru::LruCache;


const ROOT_ID : u64 = 0;

use PathU8 = String

use NodeId = String

fn path_to_id(path : PathU8)->NodeId{
    return path
}


struct Dir{
    id: NodeId,
    contents: std::collections::HashMaps<String,NodeId>,
}

enum Contents{
    File(Vec<u8>),
    Dir(Vec<String>)
}

struct SizedLru{
    lru: LruCache,
    cache_size: u64,
    limit_size: u64,
}

impl SizedLru{
    fn new()->SizedLru(limit_size){

        SizedLru{
            lru: LruCache::new(),
            cache_size:0,
            limit_size:limit_size
        }
    }

    fn add(&mut self, key: PathU8, binary: Vec<u8>){
        assert!(!self.lru.get(key).is_none());

        self.lru.add(key,contents);

        self.cache_size += binary.size();

        if  self.cache_size > self.limit_size {
            self.recycle();
        }
    }

    fn recycle(&mut self){

        assert!(self.cache_size >self.limit_size );
        let keys : Vec<PathU8> = std::vec::new();

        for it  = self.lru.iter() {
            keys.add(it);
            self.cache_size -= it->size();

            assert!(self.cache_size >= 0 );

            if self.cache_size <= self.limit_size  {
                break;
            }
        }

        for key in keys.iter(){
            self.lru.pop(key);
        }
    }

    
}


impl Dir{

    fn new()-> Node{
        Node{
            id:id,
            contents: std::collections::HashMap::new()
        }
    }

    fn set_children(&mut self, children: std::collections::HashMap<String,u64> ){
        assert!(self.contents.empty());
        self.contents = children;
    }
}


pub struct CacheFsTree{

    id_dir_map: std::collections::BTreeMap<NodeId,Node>,
    file_cache : SizedLru,
    dir_cache: std::collections::HashMap<NodeId, Vec<String> >
}


impl CacheFsTree {
    pub fn new()-> CacheFsTree{

        let mut ret = CacheFsTree{
            id_object_map 
            id_objects: std::collections::BTreeMap::new(),
            lru: std::collections::BTreeMap::new(),
            reverse_map:std::collections::HashMap::new()
        };

        ret.id_objects.insert(ROOT_ID,Node::Dir{id:ROOT_ID,children:std::collections::HashMap::new()});

        ret
    }

    pub fn set_children(self, root: PathU8, children: Vec<String>) {

        let node = self.grow_dir(root);

        let mut temp_path = root.clone();

        let children  = std::collections::HashMap::new();

        for name in children.iter(){

            temp_path.push(name);

            children.insert(name, temp_path.hash);

        }

        node.set_children(root, children);
    }

    fn remove_by_path(&mut self,path :PathU8){
        self.remove_by_id(path_to_id(path));
    }

    fn remove(&mut self,node_id : NodeId){

        let entry = self.id_objects.entry(node_id);

        panic!(!entry.is_none());
        
        if let Some(children)= entry.node.children{

            for child in children.iter(){
                self.remove(child);
            }
        }

        self.lru.remove(node_id);
        self.dir_cache.remove(node_id);
        self.id_objects.remove(node_id);

    }

    pub fn set_binary(&mut self,path : PathU8, bytes: Vec<u8> ) {

        let node_id = path_to_id(path);

        self.lru.set(node_id, bytes);
    }

    pub fn get(&mut self, path: PathU8)-> Option<Contents>{

       if let Some(binary) = self.lru.get(path) {
           return Some(binary);
       }

       if let Some(children) = self.dir_cache.get(path){
           return Some(children);
       }

       None
    }
}

