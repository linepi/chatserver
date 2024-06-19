#![allow(dead_code)]

use chatserver::client::clib;
use colored::Colorize;
use chatserver::chat::chat_client::ChatClient;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    #[arg(long)]
    address: String,
}

fn prompt(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    print!("{prompt}");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => {},
        Err(e) => { 
            log::error!("prompt: {e}");
            return Err(Box::new(e));
        }
    }
    Ok(input.trim().to_string())
}

fn random_name() -> String {
    use rand::{Rng, thread_rng};
    let mut rng = thread_rng();
    let res: Vec<u8> = (0..5).map(|_| {
        loop {
            let char = rng.gen::<u8>();
            if char.is_ascii_digit() {
                return char;
            }
        }
    }).collect();
    String::from_utf8(res).unwrap()
}

fn dump_usage() {
    println!("{}", "Usage: ".green());
    println!("\tcreate <roomname> [password] [history_visible(y/n)]");
    println!("\texit -- let it go");
    println!("\tlistr -- list rooms");
    println!("\tlistu -- list users");
    println!("\tjoin <roomname> [password]");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = args.address;

    let clientstate = std::sync::Arc::new(std::sync::RwLock::new(clib::ClientState{
        channel: ChatClient::connect(format!("http://{addr}")).await?,
        lastupdate_time: 0,
        cur_roomname: None,
        msgnum: 0,
    }));
    println!("Connected to {}!", addr);
    println!();

    let username = prompt("give your username: ").unwrap();
    let password = prompt("give your password: ").unwrap();
    let mut client = clib::Client {
        req: clib::ClientReq::default(),
        state: clientstate,
        username: username.clone(),
        password: password.clone(),
    };

    loop {
        let result = client.signup().await;
        match result {
            Ok(_) => { break; }
            Err(e) => { println!("{}", e); }
        }
        client.password = prompt("give your password: ").unwrap();
    }

    println!("{}, {}", username, "Welcome to chat room!".cyan().bold());
    dump_usage();

    loop {
        let input = prompt("> ").unwrap();
        let args: Vec<String> = input.split_whitespace().map(|s| s.to_string()).collect();

        if args.len() == 0 {
            continue;
        }

        if args[0] == "create" {
            client.req = clib::ClientReq {
                roomname: Some(args[1].clone()),
                room_password: args.get(2).cloned(),
                history_visible: match args.get(3) {
                    Some(v) => {
                        if v == "n" {
                            Some(false)
                        } else {
                            Some(true)
                        }
                    },
                    None => Some(false),
                },
                send_str: None,
            };
            client.createroom().await?;
        } else if args[0] == "join" {
            client.req = clib::ClientReq {
                roomname: Some(args[1].clone()),
                room_password: args.get(2).cloned(),
                history_visible: None,
                send_str: None,
            };

            client.join().await?;

            let (sender, receiver) = std::sync::mpsc::channel();
            let username = client.username.clone();
            let handle = std::thread::spawn(move || {
                loop {
                    let inputmsg = prompt(format!("{}: ", username.yellow()).as_str()).unwrap();
                    if inputmsg.is_empty() {
                        continue;
                    }
                    sender.send(inputmsg.clone()).unwrap();
                    if inputmsg == "exit()" {
                        break;
                    }
                }
            });

            loop {
                let tosd = receiver.try_recv();
                if tosd.is_ok() {
                    if tosd.as_ref().unwrap() == "exit()" {
                        client.exitroom().await?;
                        break;
                    }
                    client.req.send_str = Some(tosd.unwrap());
                    client.send().await?;
                } else {
                    client.update().await?;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            handle.join().unwrap();
        } else if args[0] == "exit" {
            break;
        } else if args[0] == "listr" {
            client.listrooms().await?;            
        } else if args[0] == "listu" {
            client.listusers().await?;            
        } else {
            println!("Unknown command");
            dump_usage();
        }
    }

    Ok(())
}


