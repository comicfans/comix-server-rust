#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate archive_cache;
extern crate futures;
extern crate simple_logger;
extern crate reqwest;

fuzz_target!(|data: &[u8]| {

     let decode = std::str::from_utf8(data);

    if decode.is_err() {
        return;
    }

    let utf8 = decode.unwrap();


    let addr : std::net::SocketAddr= ([0,0,0,0],0).into() ;
    // fuzzed code goes here
    let tcp_res = std::net::TcpListener::bind(addr);

    if tcp_res.is_err(){
        return;
    }

    let tcp = tcp_res.unwrap();
    let local_addr = tcp.local_addr();

    let (tx, rx) = futures::sync::oneshot::channel::<()>();

    let join = std::thread::spawn(||{
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let root_dir = archive_cache::cache::PathU8::from(manifest_dir).join("../");
        archive_cache::server::run_server_from_listener(tcp,&root_dir,rx);
    });

    //make http response

    let url = std::format!("http://127.0.0.1:{}/{}",local_addr.unwrap().port(), utf8);
    reqwest::get(url.as_str());

    tx.send(()).unwrap();
    join.join();

});
