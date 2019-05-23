#[macro_use]
extern crate log;

extern crate simple_logger;

mod cache;
mod fs;
mod server;

fn main() {
    let _ = simple_logger::init();

    let _ = server::run_server(([0, 0, 0, 0], 31257).into(), &cache::PathU8::from("."));
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_server() {

        /*
        let thrd=std::thread::spawn(||{
            server::run_server(([0, 0, 0, 0], 31257).into(), &cache::PathU8::from(env!("CARGO_MANIFEST_DIR")));
        });

        thrd.join();
        */
    }
}
