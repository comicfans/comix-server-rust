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

pub const VIRTUAL_ROOT_PATH: &str = "*\\/virtual_root\\/*";

#[cfg(not(debug_assertions))]
type NodeId = u64;

#[cfg(debug_assertions)]
type NodeId = PathU8;

pub enum FileOrMem<'a> {
    Path(&'a PathU8),
    Mem(&'a [u8]),
}

pub fn join_may_empty(lhs: &PathU8, rhs: &PathU8) -> PathU8 {
    if lhs.to_str().unwrap().is_empty() {
        return rhs.clone();
    }

    if rhs.to_str().unwrap().is_empty() {
        return lhs.clone();
    }

    return lhs.join(rhs);
}

pub fn is_archive(name: &PathU8, file_or_mem: FileOrMem) -> bool {
    let archive_exts = vec!["zip", "cbz", "rar", "cbr", "tar", "7z"];

    let ext = name.extension();

    if ext.is_some() {
        //not archive ext, and no left path
        return archive_exts.contains(&ext.unwrap().to_str().unwrap());
    }

    //no ext, try to detect by mime
    let mime;
    match file_or_mem {
        FileOrMem::Path(path) => {
            mime = tree_magic::from_filepath(path);
        }
        FileOrMem::Mem(mem) => {
            mime = tree_magic::from_u8(mem);
        }
    }
    for test in archive_exts {
        if mime.ends_with(test) {
            return true;
        }
    }

    return false;
}

fn path_to_id(path: &PathU8) -> NodeId {
    #[cfg(debug_assertions)]
    return path.clone();

    #[cfg(not(debug_assertions))]
    {
        let mut s = std::collections::hash_map::DefaultHasher::new();
        s.write(path.to_str().unwrap().as_bytes());
        s.finish()
    }
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
        debug_assert!(self.lru.get(key).is_none(), "{:?} already exists!", key);

        trace!("set file cache {:?}", key);

        self.size += binary.len();

        self.lru.put(key.clone(), binary);

        if self.size > self.limit {
            self.recycle(key);
        }

        debug_assert!(self.lru.contains(key));

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
                debug_assert!(self.size >= v.len());
                self.size -= v.len();
            }
        }
    }

    fn recycle(&mut self, preserved_key: &NodeId) {
        debug_assert!(self.size > self.limit);

        let mut keys: Vec<NodeId> = Vec::new();

        //try to remove most unused ones
        for (id, bin) in self.lru.iter().rev() {
            if id == preserved_key {
                continue;
            }

            keys.push(id.clone());

            debug_assert!(self.size >= bin.len());

            self.size -= bin.len();

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
        #[cfg(debug_assertions)]
        {
            for (key, value) in self.dir_tree.iter() {
                write!(f, "{}:\n", key.display())?;

                for name in value.keys() {
                    write!(f, "-{}\n", to_display_name(name))?;
                }
            }
            write!(f, "\n")?;
        }

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
        debug_assert!(walked_path.len() >= 1);

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
            if from_sibling {
                let _ = write!(f, "-");
            } else {
                let _ = write!(f, "/");
            }
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

    pub fn quick_try(&mut self, full_path: &PathU8) -> Option<NodeContents> {
        trace!("lookup in cache by {:?}", full_path);
        let node_id = path_to_id(full_path);

        //dir cache take higher than file
        //because we may save archive
        if let Some(children) = self.dir_tree.get(&node_id) {
            trace!("cache hit dir {:?}", full_path);
            let ret = Vec::from_iter(children.keys());
            return Some(NodeContents::Dir(ret));
        }

        if let Some(binary) = self.file_cache.get(&node_id) {
            trace!("cache hit file {:?}", full_path);
            return Some(NodeContents::File(binary));
        }

        trace!("no cache for {:?}", full_path);
        return None;
    }

    fn recursive_try(&mut self, virtual_path: &PathU8, rel: &PathU8) -> std::io::Result<bool> {
        trace!("slow try archive_root {:?} , rel {:?}", virtual_path, rel);

        debug_assert!(
            !self
                .dir_tree
                .contains_key(&path_to_id(&join_may_empty(virtual_path, rel))),
            "slow try codepath should not be called if {:?} already in dir cache",
            join_may_empty(&virtual_path, &rel)
        );
        debug_assert!(
            !self
                .file_cache
                .contains_key(&path_to_id(&join_may_empty(virtual_path, rel))),
            "slow try codepath should not be called if {:?} already in file cache",
            join_may_empty(&virtual_path, &rel)
        );

        let ar_and_entries = self
            .archive_cache
            .get_mut(&path_to_id(virtual_path))
            .unwrap();

        let mut partical_try = rel.clone();

        let mut left_path = PathU8::from("");

        for i in 0..rel.iter().count() + 1 {
            if i != 0 {
                let comp = PathU8::from(partical_try.iter().last().unwrap());
                partical_try.pop();
                left_path = join_may_empty(&comp.to_owned(), &left_path);
            }

            let full_virtual = join_may_empty(virtual_path, &partical_try);
            if self.dir_tree.contains_key(&path_to_id(&full_virtual)) {
                {
                    //parent already is a dir,so we don't need to try more
                    //this can't be a nested archive (already a dir)

                    Error::new(
                        ErrorKind::NotFound,
                        format!("{:?} is already dir, can't be nested archive", full_virtual),
                    );
                }
            }

            debug!(
                "under {:?} partical_try {:?}, left_path {:?}",
                virtual_path, partical_try, left_path
            );

            let entry = ar_and_entries
                .1
                .entry(partical_try.to_str().unwrap().to_string());
            let mut binary;
            match entry {
                std::collections::hash_map::Entry::Vacant(_) => {
                    continue;
                }
                std::collections::hash_map::Entry::Occupied(kv) => {
                    let ar_entry = kv.get();

                    let mut reader = ar_and_entries.0.reader_for(ar_entry)?;

                    //allocate memory
                    let layout: std::alloc::Layout =
                        std::alloc::Layout::from_size_align(ar_entry.size(), 1).unwrap();

                    binary = Vec::with_capacity(ar_entry.size());

                    if let Err(e) = reader.read_to_end(&mut binary) {
                        return Err(e);
                    }
                }
            }

            //hit a entry in existing archive
            let is_archive = is_archive(&partical_try, FileOrMem::Mem(&binary));

            if !is_archive {
                if left_path.to_str().unwrap().is_empty() {
                    //normal file ,load into cache
                    self.file_cache.put(
                        &path_to_id(&join_may_empty(virtual_path, &partical_try)),
                        binary,
                    );

                    return Ok(true);
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

            let nested_path = join_may_empty(virtual_path, &partical_try);
            trace!(
                "set nested archive from {:?} as {:?} as archive",
                virtual_path,
                nested_path
            );

            // have left path, but partical_path is not archive
            self.set_archive_internal(&nested_path, ar.unwrap(), true);

            if left_path.to_str().unwrap().is_empty() {
                return Ok(true);
            }

            if self.quick_try(&join_may_empty(virtual_path, rel)).is_some() {
                return Ok(true);
            }

            return self.recursive_try(&nested_path, &left_path);
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

    pub fn slow_try(&mut self, path: &PathU8) -> std::io::Result<NodeContents> {
        trace!("slow try {:?}", path);

        {
            //should quick_try() first, and expose this function only
            //but rust has bug that early return still extends lifetime
            //to function scope so we expose quick_try too
            debug_assert!(
                !self.dir_tree.contains_key(&path_to_id(path))
                    && !self.file_cache.contains_key(&path_to_id(path)),
                "{:?} already in cache before get, caller should check quick_try result first",
                path
            );
        }

        //try to find in archive
        //first we need to find matched archive root
        //due to we support nested archive, we should longest first

        let virtual_root_id = path_to_id(&PathU8::from(VIRTUAL_ROOT_PATH));

        let mut longest_match: Option<PathU8> = None;

        let workaround_rust_nll = self.dir_tree.clone();

        for virtual_path in workaround_rust_nll.get(&virtual_root_id).unwrap().keys() {
            trace!("key is {:?}", virtual_path);
            if !path.starts_with(virtual_path) {
                continue;
            }

            if longest_match.is_none() {
                longest_match = Some(PathU8::from(virtual_path));
                continue;
            }

            if longest_match.as_ref().unwrap().to_str().unwrap().len() < virtual_path.len() {
                longest_match = Some(PathU8::from(virtual_path));
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
        trace!("longest_match {:?}", matched);
        let rel = path.strip_prefix(matched.clone()).unwrap();

        self.recursive_try(&matched, &PathU8::from(rel))?;

        match self.quick_try(path) {
            Some(result) => {
                return Ok(result);
            }
            None => {
                return Err(Error::new(ErrorKind::NotFound, ""));
            }
        }
    }

    fn grow_under(&mut self, this_root: &PathU8, path: &PathU8) {
        trace!("grow under {:?} {:?}", this_root, path);

        let mut parent = this_root.clone();

        for comp in path.iter() {
            let full_path = parent.join(comp);

            let parent_id = path_to_id(&parent);

            let entry = self.dir_tree.entry(parent_id).or_insert(HashMap::new());

            let utf8 = String::from(comp.to_str().unwrap());
            parent = full_path;

            if entry.contains_key(&utf8) {
                continue;
            }

            let node_id = path_to_id(&parent);

            entry.insert(utf8, node_id);
        }
    }

    fn set_archive_internal(
        &mut self,
        virtual_path: &PathU8,
        ar: ArArchive,
        is_nested: bool,
    ) -> NodeContents {
        let virtual_root_path = &PathU8::from(VIRTUAL_ROOT_PATH);
        let virtual_root_id: NodeId = path_to_id(virtual_root_path);

        debug_assert!(
            virtual_path != virtual_root_path,
            "archive virtual path can not be same to virtual root {}",
            VIRTUAL_ROOT_PATH
        );

        let vroot_children = self.dir_tree.get_mut(&virtual_root_id).unwrap();

        debug_assert!(!vroot_children.contains_key(virtual_path.to_str().unwrap()));

        vroot_children.insert(
            virtual_path.to_str().unwrap().to_owned(),
            path_to_id(virtual_path),
        );

        let mut entries = HashMap::new();

        for f in ar.iter() {
            self.grow_under(virtual_path, &PathU8::from(f.name().clone()));
            entries.insert(f.name().to_owned(), f);
        }

        self.archive_cache
            .put(path_to_id(virtual_path), (ar, entries, is_nested));

        self.dir_tree.get_mut(&virtual_root_id).unwrap().insert(
            virtual_path.clone().to_str().unwrap().to_owned(),
            path_to_id(&virtual_path),
        );

        let ret = Vec::from_iter(
            self.dir_tree
                .get(&path_to_id(&virtual_path))
                .unwrap()
                .keys(),
        );
        return NodeContents::Dir(ret);
    }

    pub fn set_archive(
        &mut self,
        virtual_path: &PathU8,
        archive_path: &PathU8,
    ) -> std::io::Result<NodeContents> {
        let node_id = path_to_id(virtual_path);

        debug_assert!(!self.file_cache.contains_key(&node_id));

        //all virtual path must be relative to 'virtual root'
        debug_assert!(virtual_path.is_relative());

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

        debug!("try to open {:?} as archive", archive_path);

        let ar = ArArchive::new(ArStream::from_file(archive_path)?, None)?;

        trace!("added archive {:?} as {:?}", archive_path, virtual_path);
        return Ok(self.set_archive_internal(virtual_path, ar, false));
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_only_archive_under_root() {
        simple_logger::init();

        let mut d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        d.push("tests/nested.zip");

        let mut ac = ArchiveCache::new(1000, 10);

        assert!(!ac.dir_tree.is_empty());

        let virtual_path = "test/test.zip";

        assert!(ac
            .set_archive(&PathU8::from(virtual_path), &d.clone())
            .is_ok());

        let vroot_id = path_to_id(&PathU8::from(VIRTUAL_ROOT_PATH));

        let children = ac.dir_tree.get(&vroot_id).unwrap();

        assert_eq!(children.len(), 1);
    }

    #[test]
    fn test_nested() {
        simple_logger::init();

        let mut d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        d.push("tests/nested.zip");

        let mut ac = ArchiveCache::new(1000, 10);

        assert!(!ac.dir_tree.is_empty());

        let virtual_path = "tek";

        match ac
            .set_archive(&PathU8::from(virtual_path), &d.clone())
            .unwrap()
        {
            NodeContents::File(_) => {
                assert!(false);
            }
            NodeContents::Dir(dir) => {
                assert!(dir.contains(&&("test.zip".to_owned())));
            }
        }

        ac.recursive_try(&PathU8::from(virtual_path), &PathU8::from("test.zip"))
            .unwrap();

        ac.recursive_try(
            &PathU8::from(virtual_path).join("test.zip"),
            &PathU8::from("under_root"),
        )
        .unwrap();
        assert!(ac
            .quick_try(&PathU8::from(virtual_path).join(PathU8::from("test.zip")))
            .is_some());
        assert!(ac
            .quick_try(&PathU8::from(virtual_path).join(PathU8::from("test.zip/under_root")))
            .is_some());
    }

    #[test]
    fn test_lru() {
        let mut cache = LruCache::new(100);

        cache.put("key0", 0);
        cache.put("key1", 100);
        cache.put("key2", 200);
        cache.put("key3", 300);
        cache.put("key4", 400);

        for (i, v) in cache.iter() {
            println!("iter key {}, v {}", i, v);
        }
    }

    #[test]
    fn test_read() {
        simple_logger::init();

        let mut d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        d.push("tests/test.zip");

        let vec = vec!["ak", "test/test-6.6.1-0.1.31.zip", ""];

        for p in vec {
            let mut tree = ArchiveCache::new(0, 10);

            assert!(!tree.dir_tree.is_empty());

            match tree.set_archive(&PathU8::from(p), &d.clone()).unwrap() {
                NodeContents::File(_) => {
                    assert!(false);
                }
                NodeContents::Dir(dir) => {
                    assert!(dir.contains(&&("dir".to_owned())));
                    assert!(dir.contains(&&("under_root".to_owned())));
                }
            }

            match tree.quick_try(&PathU8::from(p)).unwrap() {
                NodeContents::File(_) => {
                    assert!(false);
                }
                NodeContents::Dir(dir) => {
                    assert!(dir.contains(&&("dir".to_owned())));
                    assert!(dir.contains(&&("under_root".to_owned())));
                }
            }

            tree.recursive_try(&PathU8::from(p), &PathU8::from("under_root"))
                .unwrap();

            tree.recursive_try(&PathU8::from(p), &PathU8::from("dir/under_dir"))
                .unwrap();

            assert!(tree
                .recursive_try(&PathU8::from(p), &PathU8::from("not_exists"))
                .is_err());
            assert!(tree
                .recursive_try(&PathU8::from(p), &PathU8::from("dir/not_exists"))
                .is_err());
            assert!(tree
                .recursive_try(&PathU8::from(p), &PathU8::from("no_exists_dir/not_exists"))
                .is_err());
        }
    }

}
