// Uncomment this block to pass the first stage
use std::fmt::Display;
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

struct Response {
    status_code: u32,
    status_text: String,
    body: String,
    headers: Vec<(String, String)>,
}

impl Response {
    fn new() -> Response {
        Response {
            status_code: 200,
            status_text: String::new(),
            body: String::new(),
            headers: Vec::new(),
        }
    }

    fn set_body(&mut self, body: &str) {
        self.body = body.to_string();
    }

    fn set_header(&mut self, key: &str, value: &str) {
        self.headers.push((key.to_string(), value.to_string()));
    }

    fn set_status_code(&mut self, status_code: u32) {
        self.status_code = status_code;
    }

    fn set_status_text(&mut self, status_text: &str) {
        self.status_text = status_text.to_string();
    }

    fn send(&self, stream: &mut TcpStream) {
        let response = format!("{}", self);
        stream.write_all(response.as_bytes()).unwrap();
    }

    fn new_404() -> Self {
        let mut response = Self::new();
        response.set_status_code(404);
        response.set_status_text("Not Found");
        response.set_body("Not Found");
        response
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut response = format!("HTTP/1.1 {}\r\n", self.status_code);

        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }

        response.push_str("\r\n");
        response.push_str(&self.body);

        write!(f, "{}", response)
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buf = [0; 4096];
    stream.read(&mut buf).unwrap();

    let req = Request::new(&String::from_utf8_lossy(&buf[..]));

    if req.path == "/" {
        let response = Response::new();
        response.send(&mut stream);
    } else if req.path.starts_with("/echo") {
        handle_echo_request(&mut stream, &req);
    } else {
        let response = Response::new_404();
        response.send(&mut stream);
    }
}

fn handle_echo_request(stream: &mut TcpStream, req: &Request) {
    let mut response = Response::new();

    let echo_string = req.path.strip_prefix("/echo/").unwrap_or("");
    response.set_header("Content-Type", "text/plain");
    response.set_header("Content-Length", &echo_string.len().to_string());
    response.set_body(echo_string);

    response.send(stream);
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
