use std::{io::Write, net::{IpAddr, Ipv4Addr, SocketAddr}, path::{self, PathBuf, StripPrefixError}, str::FromStr};

use errors::{new_custom_error, GenericError};
use net::{new_client_endpoint, new_server_endpoint, JSONReader, TcpEndpoint};

mod file_sender;
mod file_receiver;
mod messages;

fn main1() -> Result<(), GenericError> {
    let path = "C:\\Users\\Vasil\\Desktop\\dir_sync\\crates";
    let path = std::path::PathBuf::from_str(path)?;

    let l = files::path_to_list(&path);
    dbg!(&l);

    let p = files::list_to_path(&l);
    dbg!(&p);

    let path = path.canonicalize()?;
    // let path = path.to_str().ok_or(new_custom_error("fsdfsd"))?;
    // let path = PathBuf::from_str(path);

    let files = files::get_files_in_dir(&path)?;
    let files: Vec<String> = files.iter().map(|x| {
        let val = serde_json::to_value(x).unwrap();
        val.to_string()
    }).collect();

    // println!("{}", files[0]);
    // println!("{:?}", files);

    Ok(())
}

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
