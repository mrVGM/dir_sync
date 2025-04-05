use std::{io::{stdin, stdout, Write}, net::SocketAddr, path::PathBuf, str::FromStr, sync::mpsc::channel};

use crossterm::{cursor, execute};
use errors::{new_custom_error, GenericError};
use net::{new_client_endpoint, new_server_endpoint};
use network_interface::NetworkInterfaceConfig;
use thread_pool::ThreadPool;

mod file_sender;
mod file_receiver;
mod messages;
mod logger;

fn get_local_params() -> Result<(String, String), GenericError> {
    let cur_dir = std::env::current_dir()?;
    let cur_dir = cur_dir.to_str()
        .ok_or(new_custom_error("cur dir error"))?;
    Ok((cur_dir.into(), cur_dir.into()))
}

enum Transfer {
    SendFiles,
    ReceiveFiles,
}
fn ask_for_transfer_type() -> Result<Transfer, GenericError> {
    print!("(S)end or (R)eceive files? ");
    stdout().flush().unwrap();
    let stdin = stdin();
    let mut buf = String::new();
    stdin.read_line(&mut buf)?;
    let buf = buf.trim();
    if buf.eq_ignore_ascii_case("S") {
        return Ok(Transfer::SendFiles);
    }
    if buf.eq_ignore_ascii_case("R") {
        return Ok(Transfer::ReceiveFiles);
    }

    Err(new_custom_error("transfer type not parsed"))
}

fn main() -> Result<(), GenericError> {
    ctrlc::set_handler(|| {
        let report_channel = thread_pool::get_report_channel();
        report_channel.send(Some(new_custom_error("terminated by user"))).unwrap();
    }).map_err(|e| {
        let err = format!("{}", e);
        new_custom_error(&err)
    })?;

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(new_custom_error("server or client"));
    }
    let transfer = loop {
        let transfer = ask_for_transfer_type();
        match transfer {
            Ok(x) => {
                break x;
            }
            Err(_) => {}
        }
    };

    let pool = ThreadPool::new(2);

    let (logger_send, logger_receive) = channel();
    let logger_send_clone = logger_send.clone();
    pool.execute(move || -> Result<(), GenericError> {
        let logger_send = logger_send_clone;
        let run: &str = &args[1];
        match run {
            "server" => {
                let (server_end, addr) = new_server_endpoint(None)?;
                let port = addr.port();
                println!("{:?}", addr);

                let net_interfaces = network_interface::NetworkInterface::show();
                if let Ok(net_interfaces) = net_interfaces {
                    for n in net_interfaces.iter() {
                        let net_name = &n.name;
                        let mut addr = n.addr.iter()
                            .map(|a| {
                                let ip = a.ip();
                                match ip {
                                    std::net::IpAddr::V4(ip) => Some(ip),
                                    _ => None
                                }
                            })
                            .filter(|x| {
                                match x {
                                    Some(_) => true,
                                    None => false
                                }
                            });

                        let addr = addr.next();
                        if let Some(Some(addr)) = addr {
                            println!("{}:{} - {}", addr, port, net_name);
                        }
                    }
                }

                let path = get_local_params()?.0;
                let dir = PathBuf::from_str(&path)?;
                match transfer {
                    Transfer::SendFiles => {
                        file_sender::send_files(server_end, dir, logger_send)?;
                    }
                    Transfer::ReceiveFiles => {
                        file_receiver::receive_files(server_end, dir, logger_send)?;
                    }
                }
            }
            "client" => {
                if args.len() < 3 {
                    return Err(new_custom_error("address?"));
                }
                let addr = SocketAddr::from_str(&args[2])?;
                let client_end = new_client_endpoint(addr)?;
                let path = get_local_params()?.1;
                let dir = PathBuf::from_str(&path)?;
                match transfer {
                    Transfer::SendFiles => {
                        file_sender::send_files(client_end, dir, logger_send)?;
                    }
                    Transfer::ReceiveFiles => {
                        file_receiver::receive_files(client_end, dir, logger_send)?;
                    }
                }
            }
            _ => Err(errors::new_custom_error("CLI Error"))?
        }
        let report = thread_pool::get_report_channel();
        report.send(None)?;

        Ok(())
    });

    pool.execute(move || -> Result<(), GenericError> {
        logger::log_progress(logger_receive)
    });

    let report_channel = thread_pool::get_report_receiver();
    let err = {
        let channel = &*report_channel.lock().unwrap();
        channel.recv()?
    };

    let mut stdout = stdout();
    execute!(stdout, cursor::Show)?;

    match err {
        Some(err) => Err(err),
        None => Ok(())
    }
}
