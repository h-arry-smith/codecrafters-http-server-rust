// Uncomment this block to pass the first stage
use std::fmt::Display;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
struct Request {
    path: String,
    headers: Vec<(String, String)>,
}

impl Request {
    fn new(request: &str) -> Request {
        let path = request.split_whitespace().nth(1).unwrap_or("/");

        let headers = request
            .lines()
            .skip(1)
            .map(|line| {
                let mut parts = line.splitn(2, ": ");
                let key = parts.next().unwrap_or("").to_lowercase();
                let value = parts.next().unwrap_or("").to_string();
                (key, value)
            })
            .collect();

        Request {
            path: path.to_string(),
            headers,
        }
    }

    fn get_header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == &key.to_lowercase())
            .map(|(_, v)| v.as_str())
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

    async fn send(&self, stream: &mut TcpStream) {
        let response = format!("{}", self);
        stream.write_all(response.as_bytes()).await.unwrap();
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
        let mut response = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status_text);

        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }

        response.push_str("\r\n");
        response.push_str(&self.body);

        write!(f, "{}", response)
    }
}

type Handler = fn(&Request) -> Response;
struct Server {
    tcp_listener: TcpListener,
    root_handler: Option<Handler>,
    routes: Vec<(String, Handler)>,
}

impl Server {
    async fn new(addr: &str) -> Self {
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let routes = Vec::new();

        Self {
            tcp_listener,
            root_handler: None,
            routes,
        }
    }

    fn register_route(&mut self, path: &str, handler: Handler) {
        self.routes.push((path.to_string(), handler));
    }

    fn set_root_handler(&mut self, handler: Handler) {
        self.root_handler = Some(handler);
    }

    async fn listen(self: Arc<Self>) {
        loop {
            let (mut stream, _) = self.tcp_listener.accept().await.unwrap();

            tokio::spawn({
                let me = Arc::clone(&self);
                async move {
                    me.handle_connection(&mut stream).await;
                }
            });
        }
    }

    async fn handle_connection(&self, tcp_stream: &mut TcpStream) {
        let mut buf = [0; 4096];
        tcp_stream.read(&mut buf).await.unwrap();

        let req = Request::new(&String::from_utf8_lossy(&buf[..]));

        if req.path == "/" {
            if let Some(root_handler) = self.root_handler {
                root_handler(&req).send(tcp_stream).await;
                return;
            } else {
                let response = Response::new_404();
                response.send(tcp_stream).await;
            }
        }

        if let Some((_, handler)) = self
            .routes
            .iter()
            .find(|(path, _)| req.path.starts_with(path))
            .cloned()
        {
            handler(&req).send(tcp_stream).await;
        } else {
            let response = Response::new_404();
            response.send(tcp_stream).await;
        }
    }
}

fn handle_root(_: &Request) -> Response {
    Response::new()
}

fn handle_echo_request(req: &Request) -> Response {
    let mut response = Response::new();

    let echo_string = req.path.strip_prefix("/echo/").unwrap_or("");
    response.set_header("Content-Type", "text/plain");
    response.set_header("Content-Length", &echo_string.len().to_string());
    response.set_body(echo_string);

    response
}

fn handle_user_agent_request(req: &Request) -> Response {
    let mut response = Response::new();

    let user_agent = req.get_header("User-Agent").unwrap_or("Unknown");
    response.set_header("Content-Type", "text/plain");
    response.set_header("Content-Length", &user_agent.len().to_string());
    response.set_body(user_agent);

    response
}

#[tokio::main]
async fn main() {
    let mut server = Server::new("127.0.0.1:4221").await;

    server.set_root_handler(handle_root);
    server.register_route("/echo", handle_echo_request);
    server.register_route("/user-agent", handle_user_agent_request);

    let arc_server = Arc::new(server);
    Server::listen(arc_server).await;
}
