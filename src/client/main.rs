use chatserver::client::clib;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let addr = prompt("chat server address: ").unwrap();
    let addr = "localhost:15535".to_string();
    // let username = prompt("give your username: ").unwrap();
    let username = random_name();
    let roomname = prompt("give roomname to create(or enter): ").unwrap();
    // let roomname = "aroom".to_string();

    let tmp = chatserver::chat::chat_client::ChatClient::connect(format!("http://{addr}")).await?;

    let clientstate = std::sync::Arc::new(std::sync::RwLock::new(clib::ClientState{
        channel: tmp,
        lastupdate_time: 0,
    }));

    let client = clib::Client {
        state: clientstate,
        addr,
        username: username.clone(),
        roomname,
    };

    client.update().await?;

    let (sender, receiver) = std::sync::mpsc::channel();

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
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}


