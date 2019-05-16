extern crate simple_logger;
extern crate percent_encoding;

//mod fs;
//
use cache::PathU8;
use crossbeam;
use std::sync;
use percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};

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
    simple_logger::init().unwrap();

    let res = fs::Fs::new(&cache::PathU8::from("."));

    if res.is_err() {
        error!("{:?}", res.err());
        std::process::exit(1);
    }

    let filesystem = Arc::new(res.unwrap());

    let cache = Arc::new(Mutex::new(cache::ArchiveCache::new(100, 100)));

    crossbeam::thread::scope(|s| {
        s.spawn(|_| {
            let mut cache_t1 = cache.clone();

            //filesystem.start_watch(&*cache_t1);
        });
    })
    .unwrap();

    let addr = ([0, 0, 0, 0], 31257).into();

    let new_service = move || {
        let arc = cache.clone();
        let fs = filesystem.clone();

        service_fn_ok(move |req| {
            let raw_path = req.uri().path();

            let path = percent_encoding::percent_decode(raw_path.as_bytes()).decode_utf8().unwrap();

            trace!("access {}", path);

            let rel = PathU8::from(path[1..].to_owned());
            trace!("convert to rel {:?}", rel);

            let mut cursor = std::io::Cursor::new(Vec::new());

            let res = fs.read(&*arc, &cache::PathU8::from(rel), &mut cursor);

            if let Ok(mime) = res {
                let mut resp= Response::new(Body::from(cursor.get_ref().clone()));

                let mut header = resp.headers_mut();

                header.insert("content-type",hyper::header::HeaderValue::from_str(&mime).unwrap());
                header.insert("content-length",hyper::header::HeaderValue::from_str(&cursor.get_ref().len().to_string()).unwrap());

                return resp;
            }

            Response::new(Body::from(format!("nothing")))
        })
    };

    let server = Server::bind(&addr)
        .serve(new_service)
        .map_err(|e| eprintln!("server error: {}", e));

    info!("Listening on http://{}", addr);

    hyper::rt::run(server);
}
