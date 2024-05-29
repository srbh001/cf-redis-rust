mod resp_parser;
mod storage;
use crate::resp_parser::{handle_resp_request, Command, ContentType, RespRequest};
use crate::storage::TimeKeyValueStorage;

use std::net::SocketAddr;
use std::vec;
use std::{
    cmp::Ordering,
    env::args,
    fmt, i64,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use resp_parser::{string_to_simple_resp, to_bulk_string};

#[derive(Debug, Clone)]
enum Role {
    Master,
    Slave,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Master => write!(f, "master"),
            Role::Slave => write!(f, "slave"),
        }
    }
}

#[derive(Debug, Clone)]
struct RedisReplicationState {
    role: Role,
    connected_slaves: usize,
    master_replid: String,
    master_repl_offset: usize,
    second_repl_offset: i32,
    repl_acklog_active: usize,
    repl_backlog_size: i32,
    repl_backlog_first_byte_offset: usize,
    repl_backlog_histlen: i32,
}

impl RedisReplicationState {
    fn new() -> Self {
        Self {
            role: Role::Master,
            connected_slaves: 0,
            master_replid: String::from("8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb"),
            master_repl_offset: 0,
            second_repl_offset: 0,
            repl_acklog_active: 0,
            repl_backlog_size: 0,
            repl_backlog_first_byte_offset: 0,
            repl_backlog_histlen: 0,
        }
    }
    fn to_string_vec(self) -> Vec<String> {
        let mut string_vec = vec![];

        string_vec.push("#Replication".to_string());
        string_vec.push(format!("role:{:#?}", self.role));
        string_vec.push(format!("connected_slaves:{}", self.connected_slaves));
        string_vec.push(format!("master_replid:{}", self.master_replid));
        string_vec.push(format!("second_repl_offset:{}", self.second_repl_offset));
        string_vec.push(format!("repl_backlog_size:{}", self.repl_backlog_size));
        string_vec.push(format!(
            "repl_backlog_first_byte_offset:{}",
            self.repl_backlog_first_byte_offset,
        ));
        string_vec.push(format!(
            "repl_backlog_histlen:{}",
            self.repl_backlog_histlen
        ));

        string_vec
    }
}

fn handle_request(
    request: RespRequest,
    mut stream: TcpStream,
    storage: Arc<Mutex<TimeKeyValueStorage<String, String>>>,
    state: Arc<Mutex<RedisReplicationState>>,
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
        let mut message: String;

        match count.cmp(&1) {
            Ordering::Greater => {
                message = format!("*{}\r\n", count);

                for arg in &request.arguments {
                    if matches!(arg.content_type, ContentType::BulkString) {
                        message += &to_bulk_string(arg.content.clone());
                    }
                }
            }
            Ordering::Equal => {
                if let Some(first_arg) = request.arguments.first() {
                    message = to_bulk_string(first_arg.content.clone());
                } else {
                    message = string_to_simple_resp("ERR no arguments found", '-');
                }
            }
            Ordering::Less => {
                message = string_to_simple_resp("ERR no arguments provided", '-');
            }
        }

        stream.write_all(message.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Set) {
        // Block thread only when the parsed command requires storage access
        let mut storage_hash = storage.lock().unwrap();
        let count = request.arguments.len();

        let message: String = if count >= 2 {
            let expiry_ms_string = request.arguments.get(2).unwrap().content.clone();

            let expiry_ms: i64 = match expiry_ms_string.as_str() {
                "MAX_VALUE" => i64::MAX,
                _ => expiry_ms_string.parse::<i64>().unwrap(),
            };

            storage_hash.insert(
                request.arguments.get(0).unwrap().content.clone(),
                request.arguments.get(1).unwrap().content.clone(),
                expiry_ms,
            );
            string_to_simple_resp("OK", '+')
        } else {
            string_to_simple_resp("ERR wrong number of arguments for 'get' command", '-')
        };
        stream.write_all(message.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Get) {
        let storage_hash = storage.lock().unwrap();
        let key = request.arguments.get(0).unwrap().content.clone();
        let value = storage_hash.get(&key);

        let message = match value {
            Some(val) => to_bulk_string(val.to_string()),

            None => string_to_simple_resp("-1", '$'),
        };
        stream.write_all(message.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Info) {
        let mut state_locked = state.lock().unwrap();
        state_locked.connected_slaves += 1;

        let mut message = String::new();

        if let Some(argument) = request.arguments.get(0) {
            if argument.content == "replication" {
                // let string_vec = state_locked.clone().to_string_vec();
                // message = format!("*{}\r\n", string_vec.len());
                // for stri in string_vec {
                //     message += to_bulk_string(stri).as_str();
                // }
                let mut content = format!("role:{}\n", state_locked.role);
                content += format!("master_replid:{}\n", state_locked.master_replid).as_str();
                content +=
                    format!("master_repl_offset:{}\n", state_locked.master_repl_offset).as_str();
                message = to_bulk_string(content);
            } else {
                message = String::from("$-1\r\n");
            }
        }
        stream.write_all(message.as_bytes()).unwrap();
    } else if matches!(request.command, Command::Replconf) {
        stream.write_all("+OK\r\n".as_bytes()).unwrap();
    }
}

fn handle_client(
    mut stream: TcpStream,
    storage: Arc<Mutex<TimeKeyValueStorage<String, String>>>,
    state: Arc<Mutex<RedisReplicationState>>,
) {
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

        handle_request(
            resp_request,
            stream.try_clone().unwrap(),
            storage.clone(),
            state.clone(),
        );
    }
}

fn read_parse_stream(mut stream: TcpStream) -> RespRequest {
    let mut buffer = [0; 256];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(0) => {
            // Connection closed by client
            println!("[INFO] : Connection closed by client");
            return RespRequest::new();
        }
        Ok(n) => n,
        Err(e) => {
            println!("[ERROR] : {}", e);
            return RespRequest::new();
        }
    };
    let shortened_buffer = &buffer[..bytes_read];
    let stream_buf = String::from_utf8_lossy(shortened_buffer).into_owned();
    let resp_response: RespRequest = resp_parser::handle_resp_request(stream_buf);
    resp_response
}

fn main() {
    println!("[INFO] : Logs will appear here!");

    let storage_struct = Arc::new(Mutex::new(TimeKeyValueStorage::<String, String>::new()));
    let mut replication_state = RedisReplicationState::new();
    let arguments: Vec<String> = args().collect();

    let mut address = String::from("127.0.0.1:");
    let mut port = String::from("6379");

    if arguments.len() >= 2 && arguments[1] == "--port" {
        if let Ok(_port_number) = arguments[2].parse::<u16>() {
            port = arguments[2].clone();
        }
    }
    if let Some(replicaof_index) = arguments.iter().position(|arg| arg == "--replicaof") {
        if replicaof_index + 2 <= arguments.len() {
            let mut host_port = arguments[replicaof_index + 1]
                .split_whitespace()
                .into_iter();
            let host = host_port.next().expect("[Error] Could not read host");
            let port = host_port.next().expect("[Error] Could not read port");

            println!("Master Host: {} {}", host, port);
            replication_state.role = Role::Slave;

            let mut stream = TcpStream::connect(format!("{}:{}", host, port))
                .expect("[ERROR] Could not connect to master");

            // --------- HANDSHAKE BEGINS---------------------
            let ping = format!("*1\r\n{}", to_bulk_string("PING".to_string()));

            stream
                .write_all(ping.as_bytes())
                .expect("[ERROR] Could not ping master");
            let parsed_response_ping = read_parse_stream(
                stream
                    .try_clone()
                    .expect("[ERROR] Could not read the master"),
            );
            println!("{:#?}", parsed_response_ping.arguments);
            if parsed_response_ping
                .arguments
                .get(0)
                .unwrap()
                .content
                .to_uppercase()
                == "PONG"
            {
                println!("[INFO] Ping Successful");
                let message = format!(
                    "*3\r\n$8\r\nREPLCONF\r\n$14\r\nlistening-port\r\n$4\r\n{}\r\n",
                    port
                );
                stream
                    .write_all(message.as_bytes())
                    .expect("Could not write REPLCONF to master");
                let second_response = read_parse_stream(stream.try_clone().unwrap());
                if second_response.arguments.get(0).unwrap().content == "OK" {
                    println!("[INFO] REPLCONF 1 Successful");

                    stream
                        .write_all(
                            "*3\r\n$8\r\nREPLCONF\r\n$4\r\ncapa\r\n$6\r\npsync2\r\n".as_bytes(),
                        )
                        .unwrap();
                    if let Some(third_response) = read_parse_stream(stream).arguments.get(0) {
                        if third_response.content == "OK" {
                            println!("[INFO] REPLCONF 2 Successful");
                        }
                    }
                }
            }
        }
    }
    address += port.as_str();

    let replication_state_arc = Arc::new(Mutex::new(replication_state));
    let listener = TcpListener::bind(address).unwrap();
    for stream in listener.incoming() {
        let storage = Arc::clone(&storage_struct);
        let mut state = Arc::clone(&replication_state_arc);
        match stream {
            Ok(stream) => {
                // Spawn a new thread to handle the client
                thread::spawn(move || {
                    handle_client(stream, storage, state);
                });
            }
            Err(e) => {
                println!("[ERROR] : {}", e);
            }
        }
    }
}
