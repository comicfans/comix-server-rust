extern crate lru;


use std::string::String;
use std::vec::Vec;

use lru::LruCache;


pub type PathU8 = std::path::PathBuf;

pub type Binary = std::vec::Vec<u8>;

type NodeId = PathU8;

fn path_to_id(path : &PathU8)->NodeId{
    return path.clone();
}


struct SizedLru{
    lru: LruCache<NodeId, Binary>,
    size: usize,
    limit: usize,
}

impl SizedLru{
    fn new(limit: usize)->SizedLru{

        SizedLru{
            lru: LruCache::new(std::usize::MAX),
            size:0,
            limit:limit
        }
    }

    fn put(&mut self, key: &PathU8, binary: Binary){

        assert!(self.lru.get(key).is_none());

        self.size += binary.len();

        self.lru.put(key.clone(),binary);

        if  self.size > self.limit {
            self.recycle();
        }
    }

    fn get (&mut self, key: &PathU8)-> Option<&Binary>{
        self.lru.get(key)
    }

    fn remove(&mut self, node_id: &NodeId){
        match self.lru.get(node_id){
            None=>{
                return;
            }
            Some(v)=>{

                assert!(self.size>= v.len());
                self.size -= v.len();
            }
        }
    }


fn recycle(&mut self){

    assert!(self.size > self.limit);

    let mut keys: Vec <NodeId> = Vec::new();

        for it in self.lru.iter() {
            keys.push(it.0.clone());

            assert!(self.size >= it.1.len());

            self.size -= it.1.len();

            if self.size <= self.limit{
                break
            }
        }

    for key in keys.iter(){
        self.lru.pop(key);
    }
}
}



pub struct CacheFsTree{

    file_cache : SizedLru,
    dir_cache: std::collections::HashMap<NodeId, Vec<(String,NodeId)> >
}


impl CacheFsTree {
    pub fn new(limit: usize)-> CacheFsTree{

        let mut ret = CacheFsTree{
            file_cache: SizedLru::new(limit),
            dir_cache:std::collections::HashMap::new()
        };

        let virtual_root_id = path_to_id(&PathU8::new());

        ret.dir_cache.insert(virtual_root_id, std::vec::Vec::new());

        ret
    }

    pub fn set_children(&mut self, root: &PathU8, children: &Vec<String>) {

        assert!(self.check_dir_exists(root));

        let mut dir_children = std::vec::Vec::new();

        for name in children.iter(){
        
            let mut temp_path = root.clone();

            temp_path.push(name.clone());

            dir_children.push((name.clone(),path_to_id(&temp_path)));

        }

        let node_id = &path_to_id(root);
        assert!(!self.dir_cache.contains_key(node_id));

        self.dir_cache.insert(node_id.clone(), dir_children);

    }

    fn remove_by_path(&mut self,path :&PathU8){
        self.remove_by_id(&path_to_id(&path));
    }

    fn remove_by_id(&mut self,node_id : &NodeId){

        let mut ids = std::vec::Vec::new();

        {
        let entry = self.dir_cache.get(node_id);

        if entry.is_none() {
            //this is not a dir object
            
            self.file_cache.remove(node_id);
            return;
        }


        for (_,id) in entry.unwrap().iter(){
            ids.push(id.clone());
        }
        }


        for id in ids.iter() {
            self.remove_by_id(&id.clone());
        }
        

        self.dir_cache.remove(node_id);
    }

    fn check_dir_exists(&mut self, path: &PathU8)->bool{

        for i in path.iter() {
            return true;
        }

        return false;
    }

    pub fn set_binary(&mut self,path : &PathU8, bytes: Binary ) {

        let node_id = path_to_id(&path);

        assert!(!self.dir_cache.contains_key(&node_id));

        assert!(self.check_dir_exists(path));

        self.file_cache.put(&node_id, bytes);
    }

    pub fn try_write<W: std::io::Write> (& mut self, path: &PathU8, writer: &mut W) -> bool {

       if let Some(binary) = self.file_cache.get(path) {
           let _ = writer.write(binary);
           return true;
       }

       if let Some(children) = self.dir_cache.get(path){

           //ignore write error
           let mut first = true;
           for (child,_) in children.iter(){
               if !first {
                   let _= writer.write("\n".as_bytes());
               }
               let _=writer.write(child.as_bytes());
               first=false;
           }

           return true;
       }

       return false;
    }
}
