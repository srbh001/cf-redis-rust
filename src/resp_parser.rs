use std::{str::Lines, string, usize};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Command {
    Get,
    Set,
    Ping,
    Echo,
    None,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ContentType {
    Error,
    Integer,
    String,
    BulkString,
    Array,
    Double,
    None,
}

#[derive(Debug, Clone)]
pub struct Content {
    pub content: String,
    pub content_type: ContentType,
}

#[derive(Debug, Clone)]
pub struct RespRequest {
    pub command: Command,
    pub arguments: Vec<Content>,
    pub single_content_type: ContentType,
}

pub const ACCEPTED_TYPES: [&str; 14] = [
    "+", // Simple Strings
    "-", // Simple Errors
    ":", // Integers
    "$", // Bulk Strings (Aggregate)
    "*", // Arrays (Aggregate)
    "_", // Nulls
    "#", // Booleans
    ",", // Doubles
    "(", // Big Numbers
    "!", // Bulk Errors
    "=", // Verbatim Strings
    "%", // Maps
    "~", // Sets
    ">", // Pushes
];

impl RespRequest {
    pub fn print_struct(resp_struct: RespRequest) {
        let mut args = String::from("");
        for x in resp_struct.arguments {
            args += x.content.as_str();
            args += " ";
        }
        println!(
            "RESP REQUEST PARSED\n===========================\nCommand             : {:#?}\nArguments           : {}\nSingle Content Type : {:#?}\n\n===========================",
            resp_struct.command, args, resp_struct.single_content_type
        )
    }

    pub fn parse_command(mut resp_struct: RespRequest) -> RespRequest {
        if let Some(first_arg) = resp_struct.arguments.first() {
            if matches!(first_arg.content_type, ContentType::String)
                || matches!(first_arg.content_type, ContentType::BulkString)
            {
                if first_arg.content.to_ascii_uppercase() == "ECHO" {
                    resp_struct.command = Command::Echo;
                    resp_struct.arguments.remove(0);
                } else if first_arg.content.to_ascii_uppercase() == "GET" {
                    resp_struct.command = Command::Get;
                    resp_struct.arguments.remove(0);
                } else if first_arg.content.to_ascii_uppercase() == "SET" {
                    println!("[INFO] Arguments LEN {}", resp_struct.arguments.len());
                    resp_struct.command = Command::Set;
                    resp_struct.arguments.remove(0);
                    let mut expiry = String::from("MAX_VALUE");
                    if resp_struct
                        .arguments
                        .get(2)
                        .unwrap()
                        .content
                        .to_ascii_lowercase()
                        == "px"
                    {
                        if let Ok(int_expiry) =
                            resp_struct.arguments.get(3).unwrap().content.parse::<i64>()
                        {
                            expiry = int_expiry.to_string();
                        }
                    }

                    resp_struct.arguments.remove(3);

                    resp_struct.arguments.insert(
                        3,
                        Content {
                            content: expiry,
                            content_type: ContentType::BulkString,
                        },
                    );
                } else if first_arg.content.to_ascii_uppercase() == "PING" {
                    resp_struct.arguments.remove(0);
                    resp_struct.command = Command::Ping;
                }
            }
        }

        resp_struct
    }
}

pub fn handle_resp_request(resp_request: String) -> RespRequest {
    let mut request_lines = resp_request.lines();
    let current_line = request_lines.next().unwrap().to_string();
    let mut is_simple = false;
    if !current_line.starts_with('*') {
        is_simple = true;
    }

    let mut resp_request_struct = RespRequest {
        command: Command::None,
        arguments: Vec::new(),
        single_content_type: ContentType::None,
    };
    let _: Lines;

    (resp_request_struct, _) = parse_by_iter(
        request_lines.clone(),
        current_line,
        is_simple,
        resp_request_struct,
    );

    RespRequest::print_struct(resp_request_struct.clone());

    // finally parse if there is some command is present
    let resp_parse = RespRequest::parse_command(resp_request_struct.clone());

    RespRequest::print_struct(resp_parse.clone());
    resp_parse
}

pub fn parse_by_iter(
    request_line_iter: Lines,
    current_line: String,
    is_simple: bool,
    mut resp_request: RespRequest,
) -> (RespRequest, Lines) {
    let mut line_iter = request_line_iter;
    if let Some(_first_byte) = current_line.chars().next() {
        let first_byte = _first_byte.to_string().as_str().to_owned();

        if ACCEPTED_TYPES.contains(&first_byte.as_str()) {
            match first_byte.as_str() {
                "+" => {
                    let content_str = current_line[1..].to_string();
                    let content_struct: Content = Content {
                        content: content_str,
                        content_type: ContentType::String,
                    };
                    resp_request.arguments.push(content_struct);
                    if is_simple {
                        resp_request.single_content_type = ContentType::String;
                    }
                }
                "-" => {
                    let content_str = current_line[1..].to_string();

                    resp_request.arguments.push(Content {
                        content: content_str,
                        content_type: ContentType::Error,
                    });
                    if is_simple {
                        resp_request.single_content_type = ContentType::Error;
                    }
                }
                ":" => {
                    // Not quite correct but it works for now :rolling_eyes:
                    let content = current_line[1..].to_string();
                    resp_request.arguments.push(Content {
                        content,
                        content_type: ContentType::Integer,
                    });
                    if is_simple {
                        resp_request.single_content_type = ContentType::Integer;
                    }
                }
                "_" => {
                    if is_simple {
                        resp_request.single_content_type = ContentType::None;
                    }
                }
                "#" => {
                    let content = current_line[1..].to_string();
                    resp_request.arguments.push(Content {
                        content,
                        content_type: ContentType::Double,
                    });
                    if is_simple {
                        resp_request.single_content_type = ContentType::Double;
                    }
                }

                "$" => {
                    let string_length: usize = current_line[1..].parse::<usize>().unwrap_or(0);

                    let next_line = line_iter.next().unwrap();
                    let mut content = next_line.to_string();
                    content.truncate(string_length);
                    resp_request.arguments.push(Content {
                        content,
                        content_type: ContentType::BulkString,
                    });
                    if is_simple {
                        resp_request.single_content_type = ContentType::BulkString;
                    }
                }
                "*" => {
                    let mut item_count: usize = current_line[1..].parse::<usize>().unwrap_or(0);

                    item_count += 1;
                    while item_count != 0 {
                        let resp_request_cloned: RespRequest = resp_request.clone();
                        if let Some(next_line) = line_iter.next() {
                            (resp_request, line_iter) = parse_by_iter(
                                line_iter.clone(),
                                next_line.to_string(),
                                is_simple,
                                resp_request_cloned,
                            );
                        }

                        item_count -= 1;
                    }
                }
                others => {
                    println!("Parsing Not Yet Implemented for {}", others);
                    todo!();
                }
            }
        } else {
            println!("[PARSE] Invalid Command {}", first_byte);
        }
    }

    (resp_request, line_iter)
}

// ENCODING FUNCTIONS

pub fn to_bulk_string(content: String) -> String {
    format!("${}\r\n{}\r\n", content.len(), content)
}

pub fn string_to_simple_resp(content: &str, prefix: char) -> String {
    format!("{}{}\r\n", prefix, content)
}
