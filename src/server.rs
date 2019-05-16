extern crate log;
extern crate futures;
extern crate simple_logger;

use super::fs;
use super::cache;
use hyper::rt::Future;
use hyper::service::service_fn_ok;
use hyper::{Body, Response, Server, StatusCode};
use std::sync::{Arc, Mutex};
use futures::sync::oneshot::Receiver;

pub fn run_server_from_listener(listener : std::net::TcpListener, fs_root: &cache::PathU8, rx: Receiver<()>) {

    let res = fs::Fs::new(fs_root);

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


    let new_service = move || {
        let arc = cache.clone();
        let fs = filesystem.clone();

        service_fn_ok(move |req| {

            let raw_path = req.uri().path();

            let path = percent_encoding::percent_decode(raw_path.as_bytes())
                .decode_utf8()
                .unwrap();

            trace!("access {}", path);

            let rel = cache::PathU8::from(path[1..].to_owned());
            trace!("convert to rel {:?}", rel);

            let mut cursor = std::io::Cursor::new(Vec::new());

            let res = fs.read(&*arc, &cache::PathU8::from(rel), &mut cursor);

            if let Ok(mime) = res {
                let mut resp = Response::new(Body::from(cursor.get_ref().clone()));

                let mut header = resp.headers_mut();

                header.insert(
                    "content-type",
                    hyper::header::HeaderValue::from_str(&mime).unwrap(),
                );
                header.insert(
                    "content-length",
                    hyper::header::HeaderValue::from_str(&cursor.get_ref().len().to_string())
                        .unwrap(),
                );

                return resp;
            }

            Response::new(Body::from(format!("nothing")))
        })
    };

    let server = Server::from_tcp(listener).unwrap().serve(new_service);

    let local = server.local_addr();

    info!("Listening on http://{}", local);

    let fut = server.with_graceful_shutdown(rx).map_err(|e| {
            error!("server error: {}", e);
            panic!();
        });



    hyper::rt::run(fut);
}

pub fn run_server(addr: std::net::SocketAddr, fs_root: &cache::PathU8)->std::io::Result<bool>{

    let listener = std::net::TcpListener::bind(addr)?;

    let (_tx, rx) = futures::sync::oneshot::channel::<()>();

    run_server_from_listener(listener,fs_root,rx);

    return Ok(true);

}
