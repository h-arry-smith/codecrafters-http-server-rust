// Uncomment this block to pass the first stage
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

#[derive(Debug)]
struct Request {
    path: String,
}

impl Request {
    fn new(request: &str) -> Request {
        let path = request.split_whitespace().nth(1).unwrap_or("/");

        Request {
            path: path.to_string(),
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buf = [0; 4096];
    stream.read(&mut buf).unwrap();

    eprintln!("Request: {}", String::from_utf8_lossy(&buf[..]));

    let req = Request::new(&String::from_utf8_lossy(&buf[..]));

    if req.path == "/" {
        let response = "HTTP/1.1 200 OK\r\n\r\n";
        stream.write_all(response.as_bytes()).unwrap();
    } else {
        let response = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
        stream.write_all(response.as_bytes()).unwrap();
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    eprintln!("listening on port 4221");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                eprintln!("new connection: {}", stream.peer_addr().unwrap());
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
