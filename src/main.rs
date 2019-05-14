//mod fs;
//
use crossbeam;
use std::sync;

#[macro_use]
extern crate log;

extern crate futures;
extern crate hyper;

use futures::future;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn_ok;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::sync::{Arc, Mutex};

mod cache;
mod fs;
type BoxFut = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

fn main() {
    let res = fs::Fs::new(
        &cache::PathU8::from("."),
        fs::DEFAULT_MEM_LIMIT,
        fs::DEFAULT_ARCHIVE_LIMIT,
    );

    if res.is_err() {
        println!("{:?}", res.err());
        std::process::exit(1);
    }

    let filesystem = Arc::new(res.unwrap());

    let mut cache = Arc::new(Mutex::new(cache::ArchiveCache::new(100, 100)));

    crossbeam::thread::scope(|s| {
        s.spawn(|_| {
            let mut cache_t1 = cache.clone();

            filesystem.start_watch(&*cache_t1);
        });
    })
    .unwrap();

    let addr = ([127, 0, 0, 1], 1234).into();

    let new_service = move || {
        let arc = cache.clone();
        let fs = filesystem.clone();

        service_fn_ok(move |req| {

            let path = req.uri().path();

            let mut cursor = std::io::Cursor::new(Vec::new());

            let res = fs.read(&*arc, &cache::PathU8::from(path), &mut cursor);

            if let Ok(_) = res {
                return Response::new(Body::from(cursor.get_ref().clone()));
            }

            Response::new(Body::from(format!("nothing")))
        })
    };

    let server = Server::bind(&addr)
        .serve(new_service)
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    hyper::rt::run(server);
}
