use chatserver::chat;
use chatserver::Client;
use tokio::sync::Mutex;

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
    let mut client = Mutex::new(Client {
        channel: tmp,
        addr,
        username: username.clone(),
        roomname,
        msgnum: 0.into(),
    });

    client.get_mut().update().await?;

    // std::thread::spawn(|| {
    //     loop {
    //         client.lock().await;
    //         client.get_mut().update();
    //         std::thread::sleep(std::time::Duration::from_millis(100));
    //     }
    // });

    loop {
        let inputmsg = prompt(format!("{}: ", &username).as_str())?;
        // let _ = client.lock().await;
        client.get_mut().send(&inputmsg).await?;
        client.get_mut().update().await?;
    }
}


