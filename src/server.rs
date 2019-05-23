extern crate futures;
extern crate image;
extern crate log;
extern crate pretty_bytes;
extern crate simple_logger;

use pretty_bytes::converter::convert;

use super::cache;
use super::fs;
use futures::sync::oneshot::Receiver;
use hyper::rt::Future;
use hyper::service::service_fn_ok;
use hyper::{Body, Response, Server, StatusCode};
use image::GenericImageView;
use std::sync::{Arc, Mutex};

fn convert_if_needed(bin: &[u8], mime: String) -> Option<std::io::Cursor<Vec<u8>>> {
    if !mime.starts_with("image") {
        return None;
    }

    if bin.len() < 300 * 1024 {
        return None;
    }

    if !mime.ends_with("png") && !mime.ends_with("jpeg") {
        return None;
    }

    //png size reduce is huge, but jpg is not. we need to lower q very much
    //for jpg
    let q = if mime.ends_with("jpeg") { 60 } else { 80 };

    let decode = image::load_from_memory(bin);
    if decode.is_err() {
        return None;
    }

    let mut reduced = std::io::Cursor::new(Vec::new());

    let mut enc = image::jpeg::JPEGEncoder::new_with_quality(&mut reduced, q);

    let img = decode.unwrap();

    let res = enc.encode(&img.raw_pixels(), img.width(), img.height(), img.color());

    if res.is_err() {
        trace!("encode error {:?}", res);
        return None;
    }

    trace!(
        "reduced from {} to {}",
        convert(bin.len() as f64),
        convert(reduced.get_ref().len() as f64)
    );

    Some(reduced)
}

pub fn run_server_from_listener(
    listener: std::net::TcpListener,
    fs_root: &cache::PathU8,
    rx: Receiver<()>,
) {
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

            let res = fs.read(&*arc, &rel, &mut cursor);

            if let Ok(mut mime) = res {
                let converted = convert_if_needed(cursor.get_ref(), mime.clone());

                if let Some(newbin) = converted {
                    cursor = newbin;
                    mime = tree_magic::from_u8(cursor.get_ref());
                }

                let mut resp = Response::new(Body::from(cursor.get_ref().clone()));

                *resp.status_mut() = StatusCode::OK;

                let header = resp.headers_mut();

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

            let mut not_found = Response::new(Body::from("nothing"));
            *not_found.status_mut() = StatusCode::NOT_FOUND;

            not_found
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

pub fn run_server(addr: std::net::SocketAddr, fs_root: &cache::PathU8) -> std::io::Result<bool> {
    let listener = std::net::TcpListener::bind(addr)?;

    let (_tx, rx) = futures::sync::oneshot::channel::<()>();

    run_server_from_listener(listener, fs_root, rx);

    Ok(true)
}
