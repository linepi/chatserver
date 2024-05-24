use chatserver::chat;
use chatserver::Client;
use chatserver::ClientState;
use tokio::sync::Mutex;
use std::sync::mpsc;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let addr = prompt("chat server address: ").unwrap();
    let addr = "localhost:15535".to_string();
    let username = prompt("give your username: ").unwrap();
    // let roomname = prompt("give roomname to create(or enter): ").unwrap();
    let roomname = "aroom".to_string();

    let tmp = chat::chat_client::ChatClient::connect(format!("http://{addr}")).await?;

    let clientstate = std::sync::Arc::new(std::sync::RwLock::new(ClientState{
        channel: tmp,
        lastupdate_time: 0,
    }));

    let client = Client {
        state: clientstate,
        addr,
        username: username.clone(),
        roomname,
    };

    client.update().await?;

    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        loop {
            let inputmsg = prompt(format!("{}: ", &username).as_str()).unwrap();
            if inputmsg.is_empty() {
                continue;
            }
            sender.send(inputmsg).unwrap();
        }
    });

    loop {
        let tosd = receiver.try_recv();
        if tosd.is_ok() {
            client.send(&tosd.unwrap()).await?;
        } else {
            client.update().await?;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}


