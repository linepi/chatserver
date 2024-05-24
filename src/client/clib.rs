use std::sync::RwLock;
use std::sync::Arc;
use crate::chat;
use crate::common;

#[derive(Clone)]
pub struct Client {
    pub state: Arc<RwLock<ClientState>>,
    pub addr: String,
    pub username: String,
    pub roomname: String,
}

pub struct ClientState {
    pub channel: chat::chat_client::ChatClient<tonic::transport::Channel>,
    pub lastupdate_time: u64,
}

impl Client {
    pub async fn update(&self) -> Result<(), Box<dyn std::error::Error>> {
        let request = chat::HeartBeatRequest::build(self);
        let mut state = self.state.write().unwrap();

        let response_wrapper = state.channel.heartbeat(tonic::Request::new(request)).await?;

        let mut printlines = Vec::<String>::new();
        let response = response_wrapper.get_ref();
        assert!(response.extra_info.is_empty());
        if response.extra_info.is_empty() {
            printlines.push(response.extra_info.clone());
        }
        for msg in response.messages.iter() {
            let msg_username = msg.client.as_ref().unwrap().user.as_ref().unwrap().name.as_ref().unwrap();
            if *msg_username != self.username {
                printlines.push(format!("{}", msg).clone());
            }
        } 
        
        if printlines.len() > 0 {
            print!("\r");
            for line in printlines {
                println!("{line}");
            }
            print!("{}: ", self.username);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        state.lastupdate_time = common::now_milli_seconds();
        Ok(())
    }

    pub async fn send(&self, s: &String) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().unwrap();
        state.channel.send(tonic::Request::new(chat::SendRequest::build(self, &s))).await?;
        Ok(())
    }
}
