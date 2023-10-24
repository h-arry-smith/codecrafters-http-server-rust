// Uncomment this block to pass the first stage
use std::io::Write;
use std::net::{TcpListener, TcpStream};

fn handle_connection(mut stream: TcpStream) {
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_connection(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
