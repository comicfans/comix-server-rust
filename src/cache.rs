extern crate libarchive;
extern crate lru;

use libarchive::reader::Builder;
use libarchive::reader::Reader;

use std::string::String;
use std::vec::Vec;

use lru::LruCache;

pub type PathU8 = std::path::PathBuf;

pub type Binary = std::vec::Vec<u8>;

type NodeId = PathU8;

fn path_to_id(path: &PathU8) -> NodeId {
    return path.clone();
}

struct SizedLru {
    lru: LruCache<NodeId, Binary>,
    size: usize,
    limit: usize,
}

impl SizedLru {
    fn new(limit: usize) -> SizedLru {
        SizedLru {
            lru: LruCache::new(std::usize::MAX),
            size: 0,
            limit: limit,
        }
    }

    fn contains_key(&self, key: &PathU8) -> bool {
        self.lru.contains(key)
    }

    fn put(&mut self, key: &PathU8, binary: Binary) {
        assert!(self.lru.get(key).is_none());

        self.size += binary.len();

        self.lru.put(key.clone(), binary);

        if self.size > self.limit {
            self.recycle();
        }
    }

    fn get(&mut self, key: &PathU8) -> Option<&Binary> {
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

pub struct CacheFsTree {
    file_cache: SizedLru,
    dir_tree: std::collections::HashMap<NodeId, (Vec<String>, Vec<NodeId>)>,
    archive_readers: std::collections::HashMap<NodeId, Box<Reader>>
}

pub enum NodeContents<'a> {
    File(&'a Vec<u8>),
    Dir(&'a Vec<String>),
}

impl CacheFsTree {
    pub fn new(limit: usize) -> CacheFsTree {
        CacheFsTree {
            file_cache: SizedLru::new(limit),
            dir_tree: std::collections::HashMap::new(),
            archive_readers: std::collections::HashMap::new()
        }
    }

    pub fn set_children(&mut self, root: &PathU8, children: &Vec<String>) {
        assert!(self.check_dir_exists(root));

        let mut ids = std::vec::Vec::new();

        for name in children.iter() {
            let mut temp_path = root.clone();

            temp_path.push(name.clone());

            ids.push(path_to_id(&temp_path));
        }

        let node_id = &path_to_id(root);
        assert!(!self.dir_tree.contains_key(node_id));

        self.dir_tree
            .insert(node_id.clone(), (children.clone(), ids));
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

            for id in entry.unwrap().1.iter() {
                ids.push(id.clone());
            }
        }

        for id in ids.iter() {
            self.remove_by_id(&id.clone());
        }

        self.dir_tree.remove(node_id);
        self.archive_readers.remove(node_id);
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

    pub fn get(&mut self, path: &PathU8) -> Option<NodeContents> {

        // first test cache hit
        if let Some(binary) = self.file_cache.get(path) {
            return Some(NodeContents::File(binary));
        }

        if let Some(children) = self.dir_tree.get(path) {
            return Some(NodeContents::Dir(&children.0));
        }

        // second test if this is inside a existing archive

        return None;
    }

    pub fn set_archive(&mut self, path: &PathU8, archive_path:&PathU8 ) -> Option<libarchive::error::ArchiveError>{

        let node_id  = path_to_id(path);

        assert!(!self.file_cache.contains_key(&node_id));

        if self.dir_tree.contains_key(&node_id) {
            //already read. ignore
            return None;
        }

        //first time read
        //
        let builder = Builder::new();


        let res = builder.open_file(archive_path);

        if res.is_err() {
            return res.err();
        }

        let reader = res.unwrap();

        self.archive_readers.insert(path.clone(), Box::new(reader));

        //list files in archive


        return None;
    }
}
