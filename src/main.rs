mod resp_parser;
mod storage;
use std::{
    i64,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Context;
use resp_parser::{string_to_simple_resp, to_bulk_string};

use crate::resp_parser::{Command, ContentType, RespRequest};
use crate::storage::TimeKeyValueStorage;

fn handle_request(
    request: RespRequest,
    mut stream: TcpStream,
    storage: Arc<Mutex<TimeKeyValueStorage<String, String>>>,
) {
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
    } else if matches!(request.command, Command::Set) {
        let mut storage_hash = storage.lock().unwrap(); // Block thread only when the parsed
                                                        // command requires storage access
        let count = request.arguments.len(); //TODO: fix these conditions in the parser itself
                                             // and sort the aruments - for GET as well

        let mut message = String::new();

        if count >= 2 {
            let expiry_ms: i64;
            let expiry_ms_string = request.arguments.get(2).unwrap().content.clone();
            match expiry_ms_string.as_str() {
                "MAX_VALUE" => {
                    expiry_ms = i64::MAX;
                }
                _ => {
                    expiry_ms = expiry_ms_string.parse::<i64>().unwrap();
                }
            }

            storage_hash.insert(
                request.arguments.get(0).unwrap().content.clone(),
                request.arguments.get(1).unwrap().content.clone(),
                expiry_ms,
            );
            message = string_to_simple_resp("OK", '+');
        } else {
            message = string_to_simple_resp("ERR wrong number of arguments for 'get' command", '-');
        }
        stream.write_all(message.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Get) {
        let storage_hash = storage.lock().unwrap();
        let mut message = String::new();
        let key = request.arguments.get(0).unwrap().content.clone();
        let value = storage_hash.get(&key);

        match value {
            Some(val) => {
                message = to_bulk_string(val.to_string());
            }
            None => {
                message = string_to_simple_resp("-1", '$');
            }
        }
        stream.write_all(message.as_bytes()).unwrap();
    }
}

fn handle_client(mut stream: TcpStream, storage: Arc<Mutex<TimeKeyValueStorage<String, String>>>) {
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

        handle_request(resp_request, stream.try_clone().unwrap(), storage.clone());
    }
}

fn main() {
    println!("[INFO] : Logs will appear here!");
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();
    let mut storage_struct = Arc::new(Mutex::new(TimeKeyValueStorage::<String, String>::new()));
    //let mut storage_arc = Arc::new(Mutex::new(HashMap::<String, String>::new()));
    for stream in listener.incoming() {
        let mut storage = Arc::clone(&mut storage_struct);
        match stream {
            Ok(stream) => {
                // Spawn a new thread to handle the client
                thread::spawn(move || {
                    handle_client(stream, storage);
                });
            }
            Err(e) => {
                println!("[ERROR] : {}", e);
            }
        }
    }
}
