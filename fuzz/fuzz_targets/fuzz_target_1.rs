#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate archive_cache;
extern crate simple_logger;

fuzz_target!(|data: &[u8]| {

    // fuzzed code goes here
    let decode = std::str::from_utf8(data);

    if decode.is_err() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let root_dir = archive_cache::cache::PathU8::from(manifest_dir).join("../");

    let filesystem = archive_cache::fs::Fs::new(&root_dir).unwrap();
    let cache = std::sync::Mutex::new(archive_cache::cache::ArchiveCache::new(100, 100));
            
    let mut cursor = std::io::Cursor::new(Vec::new());
    let utf8 = decode.unwrap();

    filesystem.read(&cache,&archive_cache::cache::PathU8::from(utf8),&mut cursor);

});
