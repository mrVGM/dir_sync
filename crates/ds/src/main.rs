use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, path::PathBuf, str::FromStr, sync::mpsc::channel};

use errors::{new_custom_error, GenericError};
use net::{new_client_endpoint, new_server_endpoint};
use thread_pool::ThreadPool;

mod file_sender;
mod file_receiver;
mod messages;
mod logger;

fn get_local_params() -> Result<(String, String), GenericError> {
    let cur_dir = std::env::current_dir()?;
    let file = "settings.json";
    let file_name = cur_dir.join(file);

    if !file_name.exists() {
        let cur_dir = cur_dir.to_str()
            .ok_or(new_custom_error("cur dir error"))?;
        return Ok((cur_dir.into(), cur_dir.into()));
    }

    let json = std::fs::read_to_string(file_name)?;
    let json: serde_json::Value = serde_json::from_str(&json)?;

    let json_object = json.as_object().ok_or(new_custom_error("not valid"))?;
    let path = json_object.get("path")
        .ok_or(new_custom_error("path not found"))?
        .as_str()
        .ok_or(new_custom_error("path not found"))?;
    let outpath = json_object.get("outpath")
        .ok_or(new_custom_error("path not found"))?
        .as_str()
        .ok_or(new_custom_error("path not found"))?;

    Ok((path.to_string(), outpath.to_string()))
}

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

                let path = get_local_params()?.0;
                let dir = PathBuf::from_str(&path)?;
                dbg!(&path);
                file_sender::send_files(server_end, dir, logger_send)?;
            }
            "client" => {
                if args.len() < 3 {
                    return Err(new_custom_error("address?"));
                }
                let addr = SocketAddr::from_str(&args[2])?;
                let client_end = new_client_endpoint(addr)?;
                let path = get_local_params()?.1;
                let dir = PathBuf::from_str(&path)?;
                file_receiver::receive_files(client_end, dir, logger_send)?;
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

    Ok(())
}
