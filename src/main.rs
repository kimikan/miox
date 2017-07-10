
extern crate mio;
extern crate byteorder;
extern crate slab;

mod server;
mod connection;
mod poll;

fn main() {
    let s = server::Server::new("0.0.0.0:7777", 127);

    if let Some(mut svr) = s {
        let p = poll::Poller::new();

        if let Ok(mut poll) = p {
            println!("ok, server is running!");
            svr.run(&mut poll).expect("server run failed");
        }
    }
    println!("app exit!");
}
