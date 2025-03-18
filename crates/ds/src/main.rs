use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, path::PathBuf, str::FromStr, sync::mpsc::channel};

use errors::{new_custom_error, GenericError};
use net::{new_client_endpoint, new_server_endpoint};
use thread_pool::ThreadPool;

mod file_sender;
mod file_receiver;
mod messages;
mod logger;

fn main() -> Result<(), GenericError> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(new_custom_error("server or client"));
    }
    let pool = ThreadPool::new(2);

    let (logger_send, logger_receive) = channel();
    let logger_send_clone = logger_send.clone();
    pool.execute(move || -> Result<(), GenericError> {
        let logger_send = logger_send_clone;
        let run: &str = &args[1];
        match run {
            "server" => {
                let (server_end, addr) = new_server_endpoint(None)?;
                println!("{:?}", addr);
                let path = "C:\\Users\\Vasil\\Desktop\\dir_sync\\crates";
                let dir = PathBuf::from_str(&path)?;
                file_sender::send_files(server_end, dir, logger_send)?;
            }
            "client" => {
                if args.len() < 3 {
                    return Err(new_custom_error("port?"));
                }
                let port: &str = &args[2];
                let port: u16 = port.parse().unwrap();

                let ip = Ipv4Addr::new(127, 0, 0, 1);
                let ip = IpAddr::V4(ip);
                let addr = SocketAddr::new(ip, port);
                let client_end = new_client_endpoint(addr)?;
                file_receiver::receive_files(client_end, logger_send)?;
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

    if let Some(err) = err {
        return Err(err);
    }

    println!("Hello, world!");

    Ok(())
}
