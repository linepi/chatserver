use std::sync::RwLock;
use std::sync::Arc;
use crate::chat;
use crate::common;
use colored::Colorize;

#[derive(Clone)]
pub struct Client {
    pub state: Arc<RwLock<ClientState>>,
    pub username: String,
    pub req: ClientReq,
}

#[derive(Clone, Default)]
pub struct ClientReq {
    pub roomname: Option<String>,
    pub room_password: Option<String>,
    pub history_visible: Option<bool>,
    pub send_str: Option<String>,
}

pub struct ClientState {
    pub channel: chat::chat_client::ChatClient<tonic::transport::Channel>,
    pub lastupdate_time: u64,
    pub cur_roomname: Option<String>,
    pub msgnum: u32,
}

impl Client {
    pub async fn update(&self) -> Result<(), Box<dyn std::error::Error>> {
        let request = self.hb_req();
        let mut state = self.state.write().unwrap();
        let lastupdate_time = common::now_milli_seconds();

        let response_wrapper = state.channel.heartbeat(tonic::Request::new(request)).await?;
        let mut printlines = Vec::<String>::new();

        let response = response_wrapper.get_ref();
        if !response.extra_info.is_empty() {
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
            print!("{}: ", self.username.yellow());
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }

        state.lastupdate_time = lastupdate_time;
        state.msgnum += response.messages.len() as u32;
        state.cur_roomname = self.req.roomname.clone();
        Ok(())
    }

    pub async fn send(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().unwrap();
        state.channel.send(tonic::Request::new(self.sd_req())).await?;
        Ok(())
    }

    pub async fn createroom(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().unwrap();
        state.channel.createroom(tonic::Request::new(self.cr_req())).await?;
        Ok(())
    }

    pub async fn exitroom(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().unwrap();
        let cur_roomname = state.cur_roomname.clone().unwrap();
        state.channel.exitroom(tonic::Request::new(self.er_req(cur_roomname))).await?;
        state.lastupdate_time = 0;
        state.cur_roomname = None;
        state.msgnum = 0;
        Ok(())
    }

    pub async fn listrooms(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().unwrap();
        let response_wrapper = state.channel.getrooms(tonic::Request::new(self.gr_req())).await?;
        let response = response_wrapper.get_ref();
        for roominfo in response.roominfos.iter() {
            print!("\t{} ({}, online: [", &roominfo.name, &roominfo.manner.as_ref().unwrap().username().bold());
            for i in 0..roominfo.online_users.len() {
                print!("{}", roominfo.online_users[i]);
                if i != roominfo.online_users.len() - 1 {
                    print!(",");
                } 
            }
            println!("])");
        }
        Ok(())
    }

    fn hb_req(&self) -> chat::HeartBeatRequest {
        chat::HeartBeatRequest {
            client: Some(chat::Client {
                user: Some(chat::User {
                    name: Some(self.username.clone()),
                    gender: Some(1),
                }),
                device: Some(chat::Device::default()),
            }),
            roomname: self.req.roomname.clone().unwrap(),
            room_password: self.req.room_password.clone(), 
            lasttime: self.state.read().unwrap().lastupdate_time,
            msgnum: self.state.read().unwrap().msgnum,
        }
    }

    fn gr_req(&self) -> chat::GetRoomsRequest {
        chat::GetRoomsRequest {
            client: Some(chat::Client {
                user: Some(chat::User {
                    name: Some(self.username.clone()),
                    gender: Some(1),
                }),
                device: Some(chat::Device::default()),
            }),
        }
    }

    fn cr_req(&self) -> chat::CreateRoomRequest {
        chat::CreateRoomRequest {
            client: Some(chat::Client {
                user: Some(chat::User {
                    name: Some(self.username.clone()),
                    gender: Some(1),
                }),
                device: Some(chat::Device::default()),
            }),
            roomname: self.req.roomname.clone().unwrap(),
            password: self.req.room_password.clone(), 
            history_visible: self.req.history_visible.clone().unwrap(),
        }
    }

    fn er_req(&self, cur_rn: String) -> chat::ExitRoomRequest {
        chat::ExitRoomRequest {
            client: Some(chat::Client {
                user: Some(chat::User {
                    name: Some(self.username.clone()),
                    gender: Some(1),
                }),
                device: Some(chat::Device::default()),
            }),
            roomname: cur_rn,
        }
    }

    fn sd_req(&self) -> chat::SendRequest {
        let c = Some(chat::Client {
            user: Some(chat::User {
                name: Some(self.username.clone()),
                gender: Some(1),
            }),
            device: Some(chat::Device::default()),
        });
        chat::SendRequest {
            client: c.clone(),
            roomname: self.req.roomname.clone().unwrap(),
            message: Some(chat::Message{
                client: c.clone(),
                bytes: self.req.send_str.clone().unwrap().as_bytes().to_vec(),
                time: common::now_milli_seconds(),
                msg_type: chat::MessageType::Text as i32,
            }),
            room_password: self.req.room_password.clone(), 
        }
    }
}
