use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    debug: bool,
    bind_address: String,
    commands_1: Vec<String>,
    commands_2: Vec<String>,
    commands_3: Vec<String>,
    commands_4: Vec<String>,
}

#[derive(Debug, Copy, Clone)]
struct Message {
    command: u8,
    value: u8,
}

fn parse_message(buf: &[u8; 8]) -> Message {
    let mut out = Message { command: 0, value: 0 };
    if buf[..3] == [0, 0, 0] && buf[5..] == [255, 255, 255] { 
        out.command = buf[3];
        out.value = buf[4];
    }
    out
}

fn run_command(commands: &Vec<String>, index: usize, debug: bool) {
    match commands.get(index) {
        Some(value) => { 
            let mut child_proc = Command::new("sh");
            child_proc.arg("-c")
                .arg(&value);
            match child_proc.output() {
                Ok(v) => { if debug { println!("running '{}': {:?}", value, v) } }
                Err(e) => { if debug { println!("error '{}': {:?}", value, e) } }
            }
        }
        None => {}
    }
}

fn main() -> std::io::Result<()> {
    let config_raw = fs::read_to_string("./config.yaml").expect("Unable to open configuration file");
    let config: Config = serde_yaml::from_str(&config_raw).expect("Unable to parse configuration file");
    let debug = config.debug;
    let bind_address = config.bind_address.clone();

    let (channel_sender, channel_receiver): (Sender<Message>, Receiver<Message>) = channel();

    let control_thread = thread::spawn(move || {
        loop {
            match channel_receiver.recv() {
                Ok(v) => {
                    match v.command {
                        0 => {}
                        1 => { run_command(&config.commands_1, v.value as usize, debug) }
                        2 => { run_command(&config.commands_2, v.value as usize, debug) }
                        3 => { run_command(&config.commands_3, v.value as usize, debug) }
                        4 => { run_command(&config.commands_4, v.value as usize, debug) }
                        255 => { break }
                        _ => {}
                    }
                }
                Err(e) => { if debug { println!("error on Channel.recv(): {:?}", e) } }
            }
        }
    });
    
    let listener = TcpListener::bind(&bind_address)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let channel_sender_clone = channel_sender.clone();
                let stream_thread = thread::spawn(move || {
                    s.set_read_timeout(Some(Duration::from_millis(5000))).expect("failed to set_read_timeout()");
                    let mut buf = [0u8; 8];
                    let mut return_value = false;
                    match s.read(&mut buf) {
                        Ok(bytes_read) => {
                            if bytes_read > 0 {
                                let message = parse_message(&buf);
                                match channel_sender_clone.send(message) {
                                    Ok(_) => {}
                                    Err(e) => { if debug { println!("error on Channel.send(): {:?}", e); }}
                                }
                                if message.command == 255 { return_value = true; }
                            }
                        }
                        Err(e) => {
                            if debug { println!("error on TcpListener.read(): {:?}", e); }
                        }
                    }
                    
                    return_value
                });
                
                match stream_thread.join() {
                    Ok(v) => { 
                        if debug { println!("thread.join()") }
                        if v == true { 
                            break;
                        }
                    }
                    Err(e) => { if debug { println!("thread panic: {:?}", e) } }
                }
            }
            Err(e) => {
                if debug { println!("error on TcpListener.incoming(): {}", e); }
            }
        }
    }

    control_thread.join().expect("unable to join control_thread");
    if debug { println!("all threads ended"); }

    Ok(())
}
