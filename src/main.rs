mod resp_parser;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use resp_parser::to_bulk_string;

use crate::resp_parser::{handle_resp_request, Command, ContentType, RespRequest};

fn handle_request(request: RespRequest, mut stream: TcpStream) {
    let pong = "+PONG\r\n";

    if matches!(request.command, Command::None) {
        let error = format!(
            "-ERR Unknown command '{}'\r\n",
            request.arguments.first().unwrap().content
        );
        stream.write_all(error.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Ping) {
        stream.write_all(pong.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Echo) {
        let count = request.arguments.len();
        let mut message = String::new();
        if count > 1 {
            message = format!("*{count}\r\n");

            for x in request.arguments {
                if matches!(x.content_type, ContentType::BulkString) {
                    message += to_bulk_string(x.content).as_str();
                }
            }
        } else if count == 1 {
            message = to_bulk_string(request.arguments.first().unwrap().content.clone());
        }

        stream.write_all(message.as_bytes()).unwrap();
    }
}

fn handle_client(mut stream: TcpStream) {
    loop {
        let mut buffer = [0; 256];
        let bytes_read = match stream.read(&mut buffer) {
            Ok(0) => {
                // Connection closed by client
                println!("[INFO] : Connection closed by client");
                return;
            }
            Ok(n) => n,
            Err(e) => {
                println!("[ERROR] : {}", e);
                return;
            }
        };

        let shortened_buffer = &buffer[..bytes_read];
        let stream_buf = String::from_utf8_lossy(shortened_buffer).into_owned();

        println!("[INFO] : accepted new request");
        println!(
            "=====REQUEST==========\n{}======================",
            stream_buf
        );

        let resp_request: RespRequest = resp_parser::handle_resp_request(stream_buf);
        handle_request(resp_request, stream.try_clone().unwrap());
    }
}

fn main() {
    println!("[INFO] : Logs will appear here!");
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Spawn a new thread to handle the client
                thread::spawn(move || {
                    handle_client(stream);
                });
            }
            Err(e) => {
                println!("[ERROR] : {}", e);
            }
        }
    }
}
