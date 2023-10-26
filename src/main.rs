use anyhow::Context;
use anyhow::Result;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
struct Request {
    verb: Verb,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Request {
    fn new(request: &str) -> Result<Request> {
        let verb = match request.split_whitespace().next() {
            Some("GET") => Verb::Get,
            Some("POST") => Verb::Post,
            _ => return Err(anyhow::anyhow!("Unknown verb")),
        };
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

        let body = request.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        Ok(Request {
            verb,
            path: path.to_string(),
            headers,
            body,
        })
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verb {
    Get,
    Post,
}

#[derive(Debug)]
struct Route {
    path: String,
    verb: Verb,
}

impl Route {
    fn new(path: &str, verb: Verb) -> Self {
        Self {
            path: path.to_string(),
            verb,
        }
    }

    fn does_match(&self, req: &Request) -> bool {
        self.verb == req.verb && req.path.starts_with(&self.path)
    }
}

type Handler = Box<dyn Fn(&Request) -> Response + Send + Sync>;
struct Server {
    tcp_listener: TcpListener,
    root_handler: Option<Handler>,
    routes: Vec<(Route, Handler)>,
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

    fn register_route(&mut self, route: Route, handler: Handler) {
        self.routes.push((route, handler));
    }

    fn set_root_handler(&mut self, handler: Handler) {
        self.root_handler = Some(handler);
    }

    async fn listen(self: Arc<Self>) -> Result<()> {
        loop {
            let (mut stream, _) = self
                .tcp_listener
                .accept()
                .await
                .context("Error accepting")?;

            tokio::spawn({
                let me = Arc::clone(&self);
                async move {
                    let _ = me.handle_connection(&mut stream).await;
                }
            });
        }
    }

    async fn handle_connection(&self, tcp_stream: &mut TcpStream) -> Result<()> {
        let mut buf = [0; 4096];
        let bytes_read = tcp_stream
            .read(&mut buf)
            .await
            .context("problem reading into buffer")?;

        let req = Request::new(&String::from_utf8_lossy(&buf[0..bytes_read]));

        let req = req.context("problem parsing request")?;

        if req.path == "/" {
            if let Some(root_handler) = &self.root_handler {
                root_handler(&req).send(tcp_stream).await;
                return Ok(());
            } else {
                let response = Response::new_404();
                response.send(tcp_stream).await;
                return Ok(());
            }
        }

        if let Some((_, handler)) = self.routes.iter().find(|(route, _)| route.does_match(&req)) {
            let response = handler(&req);

            response.send(tcp_stream).await;
        } else {
            let response = Response::new_404();
            response.send(tcp_stream).await;
            return Ok(());
        }

        Ok(())
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

fn handle_files_request(req: &Request, files: &[PathBuf]) -> Response {
    let given_file_name = req.path.strip_prefix("/files/").unwrap_or("");

    if let Some(file) = files
        .iter()
        .find(|file| file.file_name().unwrap_or_default() == given_file_name)
    {
        let mut response = Response::new();
        let file_contents = std::fs::read_to_string(file).unwrap_or_default();

        response.set_header("Content-Type", "application/octet-stream");
        response.set_header("Content-Length", &file_contents.len().to_string());
        response.set_body(&file_contents);

        response
    } else {
        Response::new_404()
    }
}

fn handle_post_file(req: &Request, directory: &Path) -> Response {
    let file_name = req.path.strip_prefix("/files/").unwrap_or("");
    let body_bytes = req.body.as_bytes();

    let mut file = std::fs::File::create(directory.join(file_name)).unwrap();
    file.write_all(body_bytes).unwrap();

    // FIXME: Create a response with a given status code
    let mut response = Response::new();
    response.set_status_code(201);
    response
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut dir = std::env::current_dir()?;
    if args.len() == 3 && args[1] == "--directory" {
        dir = PathBuf::from(&args[2]);
        let dir_contents = std::fs::read_dir(&args[2])?;

        for entry in dir_contents {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
    }

    let mut server = Server::new("127.0.0.1:4221").await;

    server.set_root_handler(Box::new(handle_root));
    server.register_route(
        Route::new("/echo", Verb::Get),
        Box::new(handle_echo_request),
    );
    server.register_route(
        Route::new("/user-agent", Verb::Get),
        Box::new(handle_user_agent_request),
    );

    server.register_route(
        Route::new("/files", Verb::Get),
        Box::new(move |req| handle_files_request(req, &files)),
    );

    server.register_route(
        Route::new("/files", Verb::Post),
        Box::new(move |req| handle_post_file(req, &dir)),
    );

    let arc_server = Arc::new(server);
    Server::listen(arc_server).await?;

    Ok(())
}
