extern crate lru;
extern crate tree_magic;

use std::string::String;
use std::vec::Vec;
use std::collections::{HashMap,HashSet};
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;
use std::hash::{Hash, Hasher};


use lru::LruCache;

use super::archive::Archive;
use super::archive::Zip;

pub type PathU8 = std::path::PathBuf;

pub type Binary = std::vec::Vec<u8>;

const VIRTUAL_ROOT_PATH: &str = "*virtual_root*";

type NodeId = u64;

fn path_to_id(path: &PathU8) -> NodeId {
    let s = std::collections::hash_map::DefaultHasher::new();
    s.finish()
}

struct SizedLru {
    lru: LruCache<NodeId, Binary>,
    size: usize,
    limit: usize,
}

impl SizedLru {
    fn new(limit: usize) -> SizedLru {
        SizedLru {
            lru: LruCache::new(4096),
            size: 0,
            limit: limit,
        }
    }

    fn contains_key(&self, key: &NodeId) -> bool {
        self.lru.contains(key)
    }

    fn put(&mut self, key: &NodeId, binary: Binary) {
        assert!(self.lru.get(key).is_none());

        self.size += binary.len();

        self.lru.put(key.clone(), binary);

        if self.size > self.limit {
            self.recycle();
        }
    }

    fn get(&mut self, key: &NodeId) -> Option<&Binary> {
        self.lru.get(key)
    }

    fn remove(&mut self, node_id: &NodeId) {
        match self.lru.get(node_id) {
            None => {
                return;
            }
            Some(v) => {
                assert!(self.size >= v.len());
                self.size -= v.len();
            }
        }
    }

    fn recycle(&mut self) {
        assert!(self.size > self.limit);

        let mut keys: Vec<NodeId> = Vec::new();

        for it in self.lru.iter() {
            keys.push(it.0.clone());

            assert!(self.size >= it.1.len());

            self.size -= it.1.len();

            if self.size <= self.limit {
                break;
            }
        }

        for key in keys.iter() {
            self.lru.pop(key);
        }
    }
}

pub struct ArchiveCache{
    file_cache: SizedLru,
    dir_tree: HashMap<NodeId, HashMap<String,NodeId>>,
    archive_cache: LruCache<NodeId,Box<dyn Archive>>
}

impl Display for ArchiveCache{
    fn fmt(&self, f: &mut Formatter)->std::fmt::Result<> {

        let virtual_root_path = PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id = path_to_id(&virtual_root_path);

        let root = String::from(VIRTUAL_ROOT_PATH);
        let walked = Vec::from(vec![(&root,&virtual_root_id,true)]);

        self.recurisve_walk(&walked, true, f);

        write!(f,"\n")
    }
}

pub enum NodeContents<'a> {
    File(&'a Vec<u8>),
    Dir(Vec<&'a String>),
}

fn to_display_name(v: &String)->String{
    if v.is_empty() {
        return String::from("*EMPTY*");
    }
    return v.clone();
}

impl ArchiveCache{

    fn recurisve_walk (&self, walked_path: &Vec<(&String,&NodeId,bool)>, from_sibling: bool,f: &mut Formatter)
        {

            assert!(walked_path.len()>=1);

            let this_node = &walked_path[walked_path.len()-1];

            let entry = self.dir_tree.get(this_node.1);

            if from_sibling {
                //preserve space of parent
                let _ = write!(f,"\n");

                for (idx,tuple) in walked_path.split_last().unwrap().1.iter().enumerate(){

                    let name = to_display_name(tuple.0);
                    let next_child_is_last = walked_path[idx+1].2;

                    let this_is_last_parent = idx != walked_path.len()-2;

                    let _ = write!(f,"{}"," ".repeat(name.len()-1));


                    if !next_child_is_last{
                        let _ = write!(f,"|");
                    }else{
                        let _ = write!(f," ");
                    }

                    if this_is_last_parent {
                        let _ = write!(f," ");
                    }
                }
            }

            if walked_path.len() !=1 {
                let _ = write!(f,"-");
            }

            let _ = write!(f,"{}", to_display_name(walked_path.last().unwrap().0));

            if let None = entry{
                //this is leaf node
                return;
            }

            let mut first = true;

            let children = entry.unwrap();

            for (idx,name_id) in children.iter().enumerate(){
                {
                    let mut temp = walked_path.clone();
                    temp.push((name_id.0,name_id.1,idx==children.len()-1));
                    self.recurisve_walk(&temp, !first, f);
                }
                first = false;
            }
    }

    pub fn new(binary_limit: usize, archive_limit: usize) -> ArchiveCache{
        let mut ret = ArchiveCache {
            file_cache: SizedLru::new(binary_limit),
            dir_tree: HashMap::new(),
            archive_cache: LruCache::new(archive_limit)
        };

        let virtual_root_path = &PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id = path_to_id(virtual_root_path);

        ret.dir_tree.insert(virtual_root_id,HashMap::new());

        return ret;
    }

    fn remove_by_path(&mut self, path: &PathU8) {
        self.remove_by_id(&path_to_id(&path));
    }

    fn remove_by_id(&mut self, node_id: &NodeId) {
        let mut ids = std::vec::Vec::new();

        {
            let entry = self.dir_tree.get(node_id);

            if entry.is_none() {
                //this is not a dir object
                //maybe not loaded dir, or a file node

                self.file_cache.remove(node_id);
                return;
            }

            for (_,node_id) in entry.unwrap().iter() {
                ids.push(node_id.clone());
            }
        }

        for id in ids.iter() {
            self.remove_by_id(&id.clone());
        }

        self.dir_tree.remove(node_id);
        self.archive_cache.pop(node_id);
    }

    fn check_dir_exists(&mut self, path: &PathU8) -> bool {
        for i in path.iter() {
            return true;
        }

        return false;
    }

    fn set_binary(&mut self, path: &PathU8, bytes: Binary) {
        let node_id = path_to_id(&path);

        assert!(!self.dir_tree.contains_key(&node_id));

        assert!(self.check_dir_exists(path));

        self.file_cache.put(&node_id, bytes);
    }

    pub fn quick_try(&mut self, full_path: &PathU8) -> Option<NodeContents> {

        let node_id = path_to_id(full_path);

        //dir cache take higher than file 
        //because we may save archive 
        if let Some(children) = self.dir_tree.get(&node_id) {

            let ret = Vec::from_iter(children.keys());
            return Some(NodeContents::Dir(ret));
        }

        if let Some(binary) = self.file_cache.get(&node_id) {
            return Some(NodeContents::File(binary));
        }

        

        return None;
    }

    pub fn slow_try(&mut self, virtual_path: &PathU8 ,rel: &PathU8) -> std::io::Result<NodeContents> {

        //we already kown this file under virtual_path (of archive)
        let archive_virtual_node = path_to_id(virtual_path);

        //this must be in archive cache (that means this function should be 
        //called after set_archive)
        assert!(self.archive_cache.contains(&archive_virtual_node));
        //then we try to access file directly inside this archive
        //
        {
            let mut binary: Vec<u8> = Vec::new();

            {
                let ar = self.archive_cache.get_mut(&archive_virtual_node).unwrap();
                let mut res = ar.entry(&String::from(rel.to_str().unwrap()))?;
                res.read_to_end(&mut binary)?;
            }


            //test if file is image
            let mime= tree_magic::from_u8(&binary);
            if mime.starts_with("image") {
                //this is image
                let full_path = virtual_path.join(rel);
                self.set_binary(&full_path,binary);
                let ret = self.file_cache.get(&path_to_id(&full_path)).unwrap();
                return Ok(NodeContents::File(ret));
            }

            if mime.starts_with("xarchive") {
                //this is archive again
            }

        }

        return Err(std::io::Error::new(std::io::ErrorKind::NotFound,"bad"));
    }

    fn grow_under(&mut self, this_root: &PathU8,path : &PathU8) {

        let mut parent = this_root.clone();

        for comp in path.iter(){

            let full_path = parent.join(comp);

            let parent_id = path_to_id(&parent);

            let entry = self.dir_tree.entry(parent_id).or_insert(HashMap::new());


            let utf8 = String::from(comp.to_str().unwrap());
            if entry.contains_key(&utf8) {
                parent = full_path;
                continue;
            }

            let node_id = path_to_id(&full_path);

            entry.insert(utf8,node_id);

            parent = full_path;
        }
    }

    pub fn set_archive(&mut self, virtual_path: &PathU8, archive_path: &PathU8 ) -> Option<std::io::Error>{

        let node_id  = path_to_id(virtual_path);

        assert!(!self.file_cache.contains_key(&node_id));

        if self.dir_tree.contains_key(&node_id) {
            //already read. ignore
            return None;
        }

        //first time read
        //

        //list files in archive
        //
        //
        let ext = archive_path.extension().unwrap().to_str().unwrap().to_owned().to_lowercase();

        let of = std::fs::File::open(archive_path);

        if of.is_err() {
            return of.err();
        }

        let archive = Zip::new(of.unwrap());

        if archive.is_err(){
            return archive.err();
        }

        
        let mut res = Box::new(archive.unwrap());

        let entries = res.list();

        if entries.is_err() {
            return entries.err();
        }

        for f in entries.unwrap(){
            self.grow_under(virtual_path,&PathU8::from(f));
        }
       

        let virtual_root_path = &PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id:NodeId = path_to_id(virtual_root_path).clone();

        let children = self.dir_tree.get_mut(&virtual_root_id).unwrap();

        children.insert(String::from(virtual_path.to_str().unwrap()),node_id);

        self.archive_cache.put(virtual_root_id, res);

        return None;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_read() {

        
        let mut d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    
        d.push("tests/logtrail-6.6.1-0.1.31.zip");

        let mut tree = ArchiveCache::new(1000,10);

        assert!(!tree.dir_tree.is_empty());

        let r = tree.set_archive(&PathU8::from(""),&d.clone());

        print!("{}",tree.to_string());

    }

}
