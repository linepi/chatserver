pub mod common;
pub mod client;
pub mod server;

pub mod chat {
    tonic::include_proto!("chat");
}

impl chat::HeartBeatRequest {
    pub fn build(cli: &client::clib::Client) -> Self {
        chat::HeartBeatRequest {
            client: Some(chat::Client {
                user: Some(chat::User {
                    name: Some(cli.username.clone()),
                    gender: Some(1),
                }),
                device: Some(chat::Device::default()),
            }),
            roomname: cli.roomname.clone(),
            lasttime: cli.state.read().unwrap().lastupdate_time,
        }
    }
}

impl chat::SendRequest {
    pub fn build(cli: &client::clib::Client, s: &String) -> Self {
        let c = Some(chat::Client {
            user: Some(chat::User {
                name: Some(cli.username.clone()),
                gender: Some(1),
            }),
            device: Some(chat::Device::default()),
        });
        chat::SendRequest {
            client: c.clone(),
            roomname: cli.roomname.clone(),
            message: Some(chat::Message{
                client: c.clone(),
                bytes: s.as_bytes().to_vec(),
                time: common::now_milli_seconds(),
                msg_type: chat::MessageType::Text as i32,
            }),
        }
    }
}

impl std::fmt::Display for chat::Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let username = self.client.as_ref().unwrap().user.as_ref().unwrap().name.as_ref().unwrap();
        // let milli =  self.time;
        let msg = String::from_utf8(self.bytes.clone()).unwrap();
        // write!(f, "[{}] {}: {}", common::human_milli_seconds(milli), username, msg)?;
        write!(f, "{}: {}", username, msg)?;
        Ok(())
    } 
}

impl chat::Room {
    pub fn from_file(filepath: &String) -> Result<Self, Box<dyn std::error::Error>> {
        let buf = std::fs::read(filepath)?;
        Ok(prost::Message::decode(&buf[..]).unwrap())
    }

    pub fn to_file(&self, filepath: &String) -> Result<(), Box<dyn std::error::Error>> {
        use prost::Message;
        let mut buf = vec![];
        self.encode(&mut buf)?;
        std::fs::write(filepath, buf)?;
        Ok(())
    }
}

