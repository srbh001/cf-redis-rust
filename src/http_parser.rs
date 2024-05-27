#![allow(dead_code)]
use std::usize;

#[derive(Debug, Clone)]
pub enum Method {
    POST,
    GET,
}

#[derive(Clone)]
pub struct HttpRequest {
    method: Method,
    path: String,
    path_param: String,
    content_length: usize,
    content_type: String,
    user_agent: String,
    data: String,
    encoding: String,
}

impl HttpRequest {
    pub fn print_request(http_request: HttpRequest) {
        println!(
            "[INFO] HTTP REQUEST HANDLER 
    ===============================
    [INFO] Method : {:#?}
    [INFO] Path: {} 
    [INFO] Path Param: {}
    [INFO] Content-Encoding : {}
    [INFO] Content-Length: {}
    [INFO] User-Agent: {} 
    [INFO] Content-Type: {} 
    [INFO] Data: {} 
    [INFO] END OF SH RESPONSE
    ==============================
        ",
            http_request.method,
            http_request.path,
            http_request.path_param,
            http_request.encoding,
            http_request.content_length,
            http_request.user_agent,
            http_request.content_type,
            http_request.data
        );
    }
}

pub fn handle_stream(stream_buf: String) -> HttpRequest {
    let accepted_encodings = [
        "gzip",
        "deflate",
        "compress",
        "br",
        "zstd",
        "exi",
        "identity",
        "pack200-gzip",
    ];

    let blank_string = String::from("");
    let mut line_iterator = stream_buf.lines();
    let mut http_request: HttpRequest = HttpRequest {
        method: Method::GET,
        path: String::from("/"),
        path_param: String::from(""),
        content_length: 0,
        content_type: blank_string.clone(),
        user_agent: blank_string.clone(),
        data: blank_string.clone(),
        encoding: blank_string.clone(),
    };

    while let Some(line_content) = line_iterator.next() {
        if line_content.starts_with("GET /") {
            http_request.method = Method::GET; // Assuming some type of HTTP request only.
            let mut line_content_iter = line_content.split_whitespace();
            line_content_iter.next(); // skip the "GET"
            match line_content_iter.next() {
                Some(full_path) => {
                    let mut path_iter = full_path.split("/");
                    path_iter.next();
                    http_request.path = String::from("/") + path_iter.next().unwrap_or("");
                    http_request.path_param = String::from(path_iter.next().unwrap_or(""));
                }
                None => {
                    http_request.path = String::from("/");
                    http_request.path_param = String::from("");
                }
            }
        } else if line_content.starts_with("POST /") {
            http_request.method = Method::POST; // Assuming some type of HTTP request only.
            let mut line_content_iter = line_content.split_whitespace();
            line_content_iter.next(); // skip the "GET"
            match line_content_iter.next() {
                Some(full_path) => {
                    full_path.replace(" ", "");
                    let mut path_iter = full_path.split("/");
                    path_iter.next(); // Skip the blank char
                    http_request.path = String::from("/") + path_iter.next().unwrap_or("");
                    http_request.path_param = String::from(path_iter.next().unwrap_or(""));
                }
                None => {
                    http_request.path = String::from("/");
                    http_request.path_param = String::from("");
                }
            }
        }
        if line_content.starts_with("Content-Type") {
            let mut line_content_iter = line_content.split_whitespace();
            line_content_iter.next(); // skip to content type
            match line_content_iter.next() {
                Some(content_type) => {
                    http_request.content_type = String::from(content_type);
                }
                None => {
                    http_request.content_type = String::from("");
                }
            }
        }
        if line_content.starts_with("Content-Length") {
            let mut line_content_iter = line_content.split_whitespace();
            line_content_iter.next();
            match line_content_iter.next() {
                Some(content_length) => {
                    http_request.content_length = content_length.parse::<usize>().unwrap_or(0);
                }
                None => http_request.content_length = 0,
            }
        }
        if line_content.starts_with("User-Agent") {
            let user_agent = line_content.replace("User-Agent: ", "");

            http_request.user_agent = user_agent;
        }
        if line_content.starts_with("Accept-Encoding: ") {
            http_request.encoding = String::from("");
            let encodings = line_content
                .replace("Accept-Encoding: ", "")
                .replace(" ", "");
            let mut encoding_iter = encodings.split(",");

            while let Some(_encoding) = encoding_iter.next() {
                let encoding = String::from(_encoding);
                if accepted_encodings.contains(&encoding.as_str()) {
                    http_request.encoding = encoding;
                    break;
                }
            }
        }
        if line_content.is_empty() {
            let mut line_iterator_cloned = line_iterator.clone();

            while let Some(body_content) = line_iterator_cloned.next() {
                http_request.data += body_content;
            }
            break;
        }
    }
    http_request
}
