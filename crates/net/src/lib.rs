use std::{cell::RefCell, io::{Read, Write}, net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream}, rc::Rc};

use errors::{new_custom_error, GenericError};
use serde_json::Value;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum TcpMessagePayload {
    NewConnection
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TcpMessage {
    payload: TcpMessagePayload
}

pub trait TcpEndpoint {
    fn get_connection(&mut self) -> Result<TcpStream, GenericError>;
    fn wait_for_connection(&mut self) -> Result<TcpStream, GenericError>;
}

struct TcpClientEnd(SocketAddr, JSONReader);

impl TcpClientEnd {
    fn new(addr: SocketAddr) -> Result<Self, GenericError> {
        let main_stream = TcpStream::connect(addr)?;
        let reader = JSONReader::new(main_stream);
        Ok(TcpClientEnd(addr, reader))
    }
}

impl TcpEndpoint for TcpClientEnd {
    fn get_connection(&mut self) -> Result<TcpStream, GenericError> {
        let addr = &self.0;
        let stream = TcpStream::connect(addr)?;
        Ok(stream)
    }

    fn wait_for_connection(&mut self) -> Result<TcpStream, GenericError> {
        let reader = &mut self.1;
        let json = reader.read_json()?;

        let message: TcpMessage = serde_json::from_value(json)?;

        match message.payload {
            TcpMessagePayload::NewConnection => {
                let stream = TcpStream::connect(self.0)?;
                Ok(stream)
            }
        }
    }
}

struct TcpServerEnd {
    listener: TcpListener,
    addr: SocketAddr,
    main_stream: Option<TcpStream>
}

impl TcpServerEnd {
    fn new(port: Option<u16>) -> Result<Self, GenericError> {
        let ip = Ipv4Addr::new(0, 0, 0, 0);
        let ip = IpAddr::V4(ip);
        let port = match port {
            Some(p) => p,
            None => 0
        };
        let addr = SocketAddr::new(ip, port);

        let listener = std::net::TcpListener::bind(addr)?;
        let addr = listener.local_addr()?;

        let server_end = TcpServerEnd {
            listener,
            addr,
            main_stream: None
        };
        Ok(server_end)
    }

    fn get_addr(&self) -> SocketAddr {
        self.addr
    }

    fn init_main_stream(&mut self) -> Result<(), GenericError> {
        if let Some(_) = self.main_stream {
            return Ok(());
        }
        let (stream, _) = self.listener.accept()?;
        self.main_stream = Some(stream);

        Ok(())
    }
}

impl TcpEndpoint for TcpServerEnd {
    fn get_connection(&mut self) -> Result<TcpStream, GenericError> {
        self.init_main_stream()?;

        match &mut self.main_stream {
            None => Err(errors::new_custom_error("No main connection!")),
            Some(stream) => {
                let payload = TcpMessagePayload::NewConnection;
                let message = TcpMessage {
                    payload
                };
                let message = serde_json::to_string(&message)?;

                stream.write(message.as_bytes())?;

                let (stream, _) = self.listener.accept()?;
                Ok(stream)
            }
        }
    }

    fn wait_for_connection(&mut self) -> Result<TcpStream, GenericError> {
        self.init_main_stream()?;

        let (stream, _) = self.listener.accept()?;
        Ok(stream)
    }
}

pub struct JSONReader {
    stream: TcpStream,
    read: String,
}

impl JSONReader {
    pub fn new(stream: TcpStream) -> Self {
        JSONReader {
            stream,
            read: "".into()
        }
    }

    pub fn read_json(&mut self) -> Result<Value, GenericError> {
        let mut buff: Vec<u8> = Vec::<u8>::with_capacity(256);
        buff.resize(256, 0);

        let mut parsed: String = "".into();
        let mut brackets: u32 = 0;

        let read = &mut self.read;

        loop {
            if read.is_empty() {
                let buff = &mut buff[..];
                let stream = &mut self.stream;
                let bytes_read = stream.read(buff)?;
                if bytes_read == 0 {
                    return Err(new_custom_error("stream closed"));
                }

                let read_str = &*String::from_utf8_lossy(&buff[..bytes_read]);
                read.push_str(read_str);
            }

            let num_parsed = 'parsed_chars: {
                for (index, c) in read.char_indices() {
                    match c {
                        '{' => {
                            brackets += 1;
                        }
                        '}' => {
                            brackets -= 1;
                        }
                        _ => {}
                    }
                    if brackets == 0 {
                        break 'parsed_chars Some(index + 1);
                    }
                }
                None
            };
            let (prefix, full_json) = match num_parsed {
                Some(n) => (n, true),
                None => (read.len(), false)
            };

            parsed.push_str(&read[..prefix]);
            *read = read[prefix..].into();

            if full_json {
                let json: serde_json::Value = serde_json::from_str(&parsed)?;
                return Ok(json);
            }
        }
    }
}

pub fn new_server_endpoint(port: Option<u16>) -> Result<(impl TcpEndpoint, SocketAddr), GenericError> {
    let server = TcpServerEnd::new(port)?;
    let addr = server.get_addr();

    Ok((server, addr))
}

pub fn new_client_endpoint(addr: SocketAddr) -> Result<impl TcpEndpoint, GenericError> {
    TcpClientEnd::new(addr)
}
