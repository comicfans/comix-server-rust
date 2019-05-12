extern crate lru;
extern crate unarr;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::Hasher;
use std::io::{Error, ErrorKind, Read, Write};
use std::iter::FromIterator;
use std::string::String;
use std::vec::Vec;

use lru::LruCache;

use unarr::{ArArchive, ArEntry, ArStream};

pub type PathU8 = std::path::PathBuf;

pub type Binary = std::vec::Vec<u8>;

const VIRTUAL_ROOT_PATH: &str = "*virtual_root*";

type NodeId = u64;

fn path_to_id(_path: &PathU8) -> NodeId {
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

    fn put(&mut self, key: &NodeId, binary: Binary) -> &Binary {
        assert!(self.lru.get(key).is_none());

        self.size += binary.len();

        self.lru.put(key.clone(), binary);

        if self.size > self.limit {
            self.recycle();
        }

        return self.lru.get(key).unwrap();
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

pub struct ArchiveCache {
    file_cache: SizedLru,
    dir_tree: HashMap<NodeId, HashMap<String, NodeId>>,
    //archive , all name=>ArEntry map, is_nested
    archive_cache: LruCache<NodeId, (ArArchive, HashMap<String, ArEntry>, bool)>,
}

impl Display for ArchiveCache {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let virtual_root_path = PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id = path_to_id(&virtual_root_path);

        let root = String::from(VIRTUAL_ROOT_PATH);
        let walked = Vec::from(vec![(&root, &virtual_root_id, true)]);

        self.recurisve_walk(&walked, true, f);

        write!(f, "\n")
    }
}

pub enum NodeContents<'a> {
    File(&'a Vec<u8>),
    Dir(Vec<&'a String>),
}

impl<'a> NodeContents<'a> {
    pub fn write_to(&self, w: &mut Write) -> std::io::Result<usize> {
        match self {
            NodeContents::File(bin) => {
                w.write_all(bin)?;
                return Ok(0);
            }
            NodeContents::Dir(dirs) => {
                for d in dirs {
                    w.write_all(d.as_bytes())?;

                    w.write_all(b"\n")?;
                }
                return Ok(0);
            }
        }
    }
}

fn to_display_name(v: &String) -> String {
    if v.is_empty() {
        return String::from("*EMPTY*");
    }
    return v.clone();
}

impl ArchiveCache {
    fn recurisve_walk(
        &self,
        walked_path: &Vec<(&String, &NodeId, bool)>,
        from_sibling: bool,
        f: &mut Formatter,
    ) {
        assert!(walked_path.len() >= 1);

        let this_node = &walked_path[walked_path.len() - 1];

        let entry = self.dir_tree.get(this_node.1);

        if from_sibling {
            //preserve space of parent
            let _ = write!(f, "\n");

            for (idx, tuple) in walked_path.split_last().unwrap().1.iter().enumerate() {
                let name = to_display_name(tuple.0);
                let next_child_is_last = walked_path[idx + 1].2;

                let this_is_last_parent = idx != walked_path.len() - 2;

                let _ = write!(f, "{}", " ".repeat(name.len() - 1));

                if !next_child_is_last {
                    let _ = write!(f, "|");
                } else {
                    let _ = write!(f, " ");
                }

                if this_is_last_parent {
                    let _ = write!(f, " ");
                }
            }
        }

        if walked_path.len() != 1 {
            let _ = write!(f, "-");
        }

        let _ = write!(f, "{}", to_display_name(walked_path.last().unwrap().0));

        if let None = entry {
            //this is leaf node
            return;
        }

        let mut first = true;

        let children = entry.unwrap();

        for (idx, name_id) in children.iter().enumerate() {
            {
                let mut temp = walked_path.clone();
                temp.push((name_id.0, name_id.1, idx == children.len() - 1));
                self.recurisve_walk(&temp, !first, f);
            }
            first = false;
        }
    }

    pub fn new(binary_limit: usize, archive_limit: usize) -> ArchiveCache {
        let mut ret = ArchiveCache {
            file_cache: SizedLru::new(binary_limit),
            dir_tree: HashMap::new(),
            archive_cache: LruCache::new(archive_limit),
        };

        let virtual_root_path = &PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id = path_to_id(virtual_root_path);

        ret.dir_tree.insert(virtual_root_id, HashMap::new());

        return ret;
    }

    pub fn invalid_path(&mut self, virtual_path: &PathU8) {
        let virtual_root_id = path_to_id(&PathU8::from(VIRTUAL_ROOT_PATH));

        let children = self.dir_tree.get(&virtual_root_id).unwrap().clone();

        for key in children.keys() {
            let prefix: bool = key.starts_with(virtual_path.to_str().unwrap());
            if prefix {
                self.remove_by_id(&path_to_id(&PathU8::from(key)));
            }
        }
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

            for (_, node_id) in entry.unwrap().iter() {
                ids.push(node_id.clone());
            }
        }

        for id in ids.iter() {
            self.remove_by_id(&id.clone());
        }

        self.dir_tree.remove(node_id);
        self.archive_cache.pop(node_id);
    }

    fn set_binary(&mut self, path: &PathU8, bytes: Binary) -> NodeContents {
        let node_id = path_to_id(&path);

        assert!(!self.dir_tree.contains_key(&node_id));

        //the binary to set must be under some dir
        assert!(self
            .dir_tree
            .contains_key(&path_to_id(&path.parent().unwrap().to_path_buf())));

        return NodeContents::File(self.file_cache.put(&node_id, bytes));
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

    fn slow_try_in_archive(
        &mut self,
        virtual_path: &PathU8,
        rel: &PathU8,
    ) -> std::io::Result<NodeContents> {
        assert!(self
            .dir_tree
            .contains_key(&path_to_id(&virtual_path.join(rel))));
        assert!(self
            .file_cache
            .contains_key(&path_to_id(&virtual_path.join(rel))));

        let ar_and_entries = self
            .archive_cache
            .get_mut(&path_to_id(virtual_path))
            .unwrap();

        let mut partical_try = rel.clone();

        let mut left_path = PathU8::from("");

        let mut first = true;
        for comp in rel.iter().rev() {
            if !first {
                partical_try.pop();
                left_path.push(comp);
            }

            first = false;

            let entry = ar_and_entries
                .1
                .entry(partical_try.to_str().unwrap().to_string());
            let mut binary = Vec::new();
            match entry {
                std::collections::hash_map::Entry::Vacant(_) => {
                    continue;
                }
                std::collections::hash_map::Entry::Occupied(kv) => {
                    let reader = ar_and_entries.0.reader_for(kv.get())?;
                    if let Err(e) = reader.read_to_end(&mut binary) {
                        return Err(e);
                    }
                }
            }

            //hit a entry in existing archive
            let mime = tree_magic::from_u8(&binary);

            let is_archive = mime.starts_with("archive");

            if !is_archive {
                if left_path.to_str().unwrap().is_empty() {
                    return Ok(NodeContents::File(
                        self.file_cache.put(&path_to_id(&partical_try), binary),
                    ));
                }

                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    partical_try.to_str().unwrap().to_owned() + "can not be decoded as archive",
                ));
            }

            // this is archive ,look into it

            let ar = ArArchive::new(ArStream::from_memory(binary), None);

            if ar.is_err() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    partical_try.to_str().unwrap().to_owned() + "can not be decoded as archive",
                ));
            }

            if left_path.to_str().unwrap().is_empty() {
                return self.set_archive_internal(&partical_try, ar.unwrap(), true);
            }

            return self.slow_try_in_archive(&partical_try, &left_path);

            // have left path, but partical_path is not archive
        }

        //all path tried, but no matched archive

        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no entry matched ".to_owned()
                + rel.to_str().unwrap()
                + " under "
                + virtual_path.to_str().unwrap(),
        ));
    }

    pub fn get(&mut self, path: &PathU8) -> std::io::Result<NodeContents> {
        {
            //should quick_try() first, and expose this function only
            //but rust has bug that early return still extends lifetime
            //to function scope so we expose quick_try too
        }

        //try to find in archive
        //first we need to find matched archive root
        //due to we support nested archive, we should longest first

        let virtual_root_id = path_to_id(&PathU8::from(VIRTUAL_ROOT_PATH));

        let mut longest_match: Option<PathU8> = None;

        let workaround_rust_nll = self.dir_tree.clone();

        for virtual_path in workaround_rust_nll.get(&virtual_root_id).unwrap().keys() {
            if path.starts_with(virtual_path) {
                if longest_match.is_none() {
                    longest_match = Some(PathU8::from(virtual_path));
                    continue;
                }

                if longest_match.as_ref().unwrap().to_str().unwrap() < path.to_str().unwrap() {
                    longest_match = Some(PathU8::from(virtual_path));
                }
            }
        }

        if longest_match.is_none() {
            return Err(Error::new(
                ErrorKind::NotFound,
                path.to_str().unwrap().to_owned() + "not found",
            ));
        }

        //longest path first

        let matched = longest_match.unwrap();
        let rel = path.strip_prefix(matched.clone()).unwrap();

        return self.slow_try_in_archive(&matched, &PathU8::from(rel));
    }

    fn grow_under(&mut self, this_root: &PathU8, path: &PathU8) {
        let mut parent = this_root.clone();

        for comp in path.iter() {
            let full_path = parent.join(comp);

            let parent_id = path_to_id(&parent);

            let entry = self.dir_tree.entry(parent_id).or_insert(HashMap::new());

            let utf8 = String::from(comp.to_str().unwrap());
            if entry.contains_key(&utf8) {
                parent = full_path;
                continue;
            }

            let node_id = path_to_id(&full_path);

            entry.insert(utf8, node_id);

            parent = full_path;
        }
    }

    fn set_archive_internal(
        &mut self,
        virtual_path: &PathU8,
        mut ar: ArArchive,
        is_nested: bool,
    ) -> std::io::Result<NodeContents> {
        let mut entries = HashMap::new();

        for f in ar.iter() {
            self.grow_under(virtual_path, &PathU8::from(f.name.clone()));
            entries.insert(f.name.clone(), f);
        }

        let virtual_root_path = &PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id: NodeId = path_to_id(virtual_root_path).clone();

        self.archive_cache
            .put(virtual_root_id, (ar, entries, is_nested));

        let ret = Vec::from_iter(
            self.dir_tree
                .get(&path_to_id(&virtual_path))
                .unwrap()
                .keys(),
        );
        return Ok(NodeContents::Dir(ret));
    }

    pub fn set_archive(
        &mut self,
        virtual_path: &PathU8,
        archive_path: &PathU8,
    ) -> std::io::Result<NodeContents> {
        let node_id = path_to_id(virtual_path);

        assert!(!self.file_cache.contains_key(&node_id));

        if self.dir_tree.contains_key(&node_id) {
            //already read. ignore
            let ret = Vec::from_iter(
                self.dir_tree
                    .get(&path_to_id(&virtual_path))
                    .unwrap()
                    .keys(),
            );
            return Ok(NodeContents::Dir(ret));
        }

        let ar = ArArchive::new(ArStream::from_file(archive_path)?, None)?;

        return self.set_archive_internal(virtual_path, ar, false);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_read() {
        let mut d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        d.push("tests/logtrail-6.6.1-0.1.31.zip");

        let mut tree = ArchiveCache::new(1000, 10);

        assert!(!tree.dir_tree.is_empty());

        let r = tree.set_archive(&PathU8::from(""), &d.clone());

        print!("{}", tree.to_string());
    }

}
