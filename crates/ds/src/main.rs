use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, path::PathBuf, str::FromStr};

use errors::GenericError;
use net::{new_client_endpoint, new_server_endpoint};

mod file_sender;
mod file_receiver;
mod messages;

fn main() -> Result<(), GenericError> {
    let args: Vec<String> = std::env::args().collect();
    let run: &str = &args[1];
    match run {
        "server" => {
            let (server_end, addr) = new_server_endpoint(None)?;
            println!("{:?}", addr);
            let path = "C:\\Users\\Vasil\\Desktop\\dir_sync\\crates";
            let dir = PathBuf::from_str(&path)?;
            file_sender::send_files(server_end, dir)?;
        }
        "client" => {
            let port: &str = &args[2];
            let port: u16 = port.parse().unwrap();
            let ip = Ipv4Addr::new(127, 0, 0, 1);
            let ip = IpAddr::V4(ip);
            let addr = SocketAddr::new(ip, port);
            let client_end = new_client_endpoint(addr)?;
            file_receiver::receive_files(client_end)?;
        }
        _ => Err(errors::new_custom_error("CLI Error"))?
    }

    let report = thread_pool::get_report_channel();
    report.send(None)?;

    println!("Hello, world!");

    let report_channel = thread_pool::get_report_receiver();
    let err = {
        let channel = &*report_channel.lock().unwrap();
        channel.recv()?
    };

    if let Some(err) = err {
        return Err(err);
    }

    Ok(())
}
