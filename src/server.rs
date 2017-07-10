
use std::net::SocketAddr;
use std::rc::Rc;
use std::io;
use std::io::{Error, ErrorKind};
use mio::{Event, Events, Token};
use mio::net::{TcpListener, TcpStream};
use slab;
use connection;
use poll;

//must be less than this
const SERVER: Token = Token(1000);

pub struct Server {
    _token: Token,
    _listener: TcpListener,
    _conns: slab::Slab<connection::Connection, Token>,
    _events: Events,
}

impl Server {
    pub fn new(addr: &str, max_clients: usize) -> Option<Server> {
        if max_clients >= SERVER.into() {
            println!("too many clients, should less than 1_000");
            return None;
        }

        let address = addr.parse::<SocketAddr>();
        if let Ok(r) = address {
            let listener = TcpListener::bind(&r);
            println!("binded {:?}", r);
            if let Ok(l) = listener {
                return Some(Server {
                    _token: SERVER,
                    _listener: l,
                    _events: Events::with_capacity(1024),
                    _conns: slab::Slab::with_capacity(max_clients),
                });
            } else {
                println!("bind failed,  port used?");
            }
        }

        println!("address format [ip:port]");
        None
    }

    pub fn run(&mut self, poll: &mut poll::Poller) -> io::Result<()> {
        poll.register_read(&self._listener, self._token)?;
        //println!("run {:?} {:?}", self._token, self._listener);
        loop {
            //println!("start to poll");
            let size = poll.poll_once(&mut self._events)?;
            //println!("poll size{}", size);
            for i in 0..size {
                let event_op = self._events.get(i);
                if let Some(event) = event_op {
                    self.on_event(poll, &event);
                } else {
                    println!("errror event");
                    break;
                }
            }
        }
    }

    fn on_event(&mut self, poll: &mut poll::Poller, event: &Event) {
        let ready = event.readiness();
        let token = event.token();
        if ready.is_error() {
            self.remove_client(token);
            println!("error event recv");
            return;
        }

        let mut vec: Vec<Token> = vec![];

        if ready.is_readable() {
            if token == self._token {
                println!("new client connected");
                self.on_accept(poll);
            } else {
                println!("forward read, token={:?}", token);
                match self.forward_readable(token) {
                    Err(_) => {
                        vec.push(token);
                    }
                    Ok(_) => {}
                }
            }
        } //end

        if ready.is_writable() {
            let client = self.get_client_mut(token);
            if let Some(mut c) = client {
                println!("client write event, token={:?}", token);
                c.on_write().unwrap_or_else(
                    |_| { vec.push(c.get_token()); },
                );
            }
        }

        for token in vec {
            self.remove_client(token);
        }
    }

    fn on_accept(&mut self, poll: &mut poll::Poller) {
        loop {
            let accept_result = self._listener.accept();
            //println!("get one client");
            let client = match accept_result {
                Ok((c, _)) => c,
                Err(e) => {
                    if e.kind() != ErrorKind::WouldBlock {
                        println!("accept error");
                    }
                    //println!("accept wuld block, {:?}", e);
                    return;
                }
            };
            let token = self.available_token(client);
            if let Some(t) = token {
                println!("client added:......");
                if let Some(c) = self.get_client(t) {
                    c.register(poll).expect("register client failed");
                }
            } else {
                println!("no available token found");
            }
        }
    }

    fn available_token(&mut self, client: TcpStream) -> Option<Token> {
        let token_op = self._conns.vacant_entry();
        let token = match token_op {
            Some(e) => {
                let connection = connection::Connection::new(client, e.index());
                e.insert(connection).index()
            }
            None => {
                println!("no empty entry for new clients");
                return None;
            }
        };

        Some(token)
    }

    fn forward_readable(&mut self, token: Token) -> io::Result<()> {
        let client_op = self.get_client_mut(token);
        let client = match client_op {
            Some(expr) => expr,
            None => {
                println!("no client got:{:?}", token);
                return Err(Error::new(ErrorKind::InvalidData, "invlid token"));
            }
        };

        loop {
            let read_result = client.on_read();
            if let Ok(read_op) = read_result {
                if let Some(message) = read_op {
                    let rc_message = Rc::new(message);
                    println!("client send message start..");
                    // Queue up a write for all connected clients.
                    client.send_message(rc_message.clone());
                } else {
                    println!("forward read: no message got");
                    break;
                }
            } else {
                println!("forward read: read failed");
                return Err(Error::new(ErrorKind::InvalidData, "read failed"));
            }
        }
        Ok(())
    }

    fn get_client(&self, token: Token) -> Option<&connection::Connection> {
        self._conns.get(token)
    }

    fn get_client_mut(&mut self, token: Token) -> Option<&mut connection::Connection> {
        self._conns.get_mut(token)
    }

    fn remove_client(&mut self, token: Token) {
        println!("client removed, {:?}", token);
        self._conns.remove(token);
    }
}
