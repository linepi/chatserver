use tonic::{Request, Response, Status};
use chat::chat_server::Chat;
use std::sync::RwLock;
use std::sync::Arc;
use std::sync::atomic;

pub mod common;

pub mod chat {
    tonic::include_proto!("chat");
}

// client lib

pub struct Client {
    pub channel: chat::chat_client::ChatClient<tonic::transport::Channel>,
    pub addr: String,
    pub username: String,
    pub roomname: String,
    pub msgnum: atomic::AtomicU64,
}

impl Client {
    pub async fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let response_wrapper = self.channel.heartbeat(
            tonic::Request::new(chat::HeartBeatRequest::build(self))).await?;
        let response = response_wrapper.get_ref();
        if !response.extra_info.is_empty() {
            println!("{}", response.extra_info);
        }
        self.msgnum.fetch_add(response.messages.len() as u64, atomic::Ordering::SeqCst);
        for msg in response.messages.iter() {
            println!("{}", msg); 
        } 
        Ok(())
    }

    pub async fn send(&mut self, s: &String) -> Result<(), Box<dyn std::error::Error>> {
        self.channel.send(tonic::Request::new(chat::SendRequest::build(self, &s))).await?;
        self.msgnum.fetch_add(1, atomic::Ordering::SeqCst);
        Ok(())
    }
}


impl chat::HeartBeatRequest {
    pub fn build(cli: &Client) -> Self {
        chat::HeartBeatRequest {
            client: Some(chat::Client {
                user: Some(chat::User {
                    name: Some(cli.username.clone()),
                    gender: Some(1),
                }),
                device: Some(chat::Device::default()),
            }),
            roomname: cli.roomname.clone(),
            msgnum: cli.msgnum.load(atomic::Ordering::SeqCst),
        }
    }
}

impl chat::SendRequest {
    pub fn build(cli: &Client, s: &String) -> Self {
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

// server lib

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

#[derive(Default)]
pub struct MyChatServer {
    state: Arc<RwLock<ServerState>>,
}

#[derive(Default)]
pub struct ServerState {
    // map: HashMap<i32, chat::Client>,
    rooms: Vec<chat::Room>,
}

impl MyChatServer {
    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let serv = MyChatServer::default();
        {
            let mut state = serv.state.write().unwrap();
            // for simplisity, load all roominfos
            let readdir = std::fs::read_dir("data")?;
            for diri in readdir {
                let entry = diri?;
                let path = entry.path();
                let pathstr = path.to_str().unwrap().to_string();
                if pathstr.contains("room") {
                    state.rooms.push(chat::Room::from_file(&pathstr).unwrap());
                }
            }
        }
        Ok(serv)
    }
}

impl Drop for MyChatServer {
    // serialize all rooms
    fn drop(&mut self) {
        let state = self.state.read().unwrap();
        for room in state.rooms.iter() {
            let _ = room.to_file(&format!("data/room_{}", &room.name));
        }
    }
}


#[tonic::async_trait]
impl Chat for MyChatServer {
    async fn heartbeat(
        &self,
        request: Request<chat::HeartBeatRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        println!("Got a heartbeat request from {:?}", request.remote_addr());
        let hb_req = request.into_inner();
        if hb_req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if hb_req.roomname.is_empty() { // create room
            log::error!("roomname is none");
            return Err(Status::invalid_argument("roomname is none"));
        }

        let mut state = self.state.write().unwrap();
        let room = state.rooms.iter_mut().find(|x| x.name == hb_req.clone().roomname);

        let mut response = chat::ServerResponse::default();
        if room.is_none() {
            // no room found, create it
            let now = common::now_milli_seconds();
            state.rooms.push(chat::Room{
                created_time: now,
                history_visible: true,
                manner: hb_req.client.clone(),
                messages: vec![],
                clients: vec![hb_req.client.clone().unwrap()],
                name: hb_req.roomname,
            });
        } else {
            // room found, check if client exists in this room
            let room: &mut chat::Room = room.unwrap();
            let client_exist_in_room = common::client_in_room(hb_req.client.as_ref().unwrap(), room);

            if !client_exist_in_room {
                room.clients.push(hb_req.client.unwrap().clone());
                response.messages = room.messages.clone();
            } else {
                for i in (hb_req.msgnum as usize)..room.messages.len() {
                    response.messages.push(room.messages[i].clone()); 
                }
            }
        }
        Ok(Response::new(response))
    }

    async fn send(
        &self, 
        request: Request<chat::SendRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        println!("Got a send request from {:?}", request.remote_addr());
        let sd_req = request.into_inner();
        if sd_req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if sd_req.message.is_none() {
            log::error!("message is none");
            return Err(Status::invalid_argument("message is none"));
        }
        if sd_req.roomname.is_empty() { // create room
            log::error!("roomname is empty");
            return Err(Status::invalid_argument("roomname is empty"));
        }

        let mut state = self.state.write().unwrap();
        let room = state.rooms.iter_mut().find(|x| x.name == sd_req.clone().roomname);
        if room.is_none() {
            log::error!("room is none");
            return Err(Status::invalid_argument("room is none"));
        }
        let room: &mut chat::Room = room.unwrap();
        let client_exist_in_room = common::client_in_room(sd_req.client.as_ref().unwrap(), room);

        if !client_exist_in_room {
            let msg = format!("client not exist in room {}", sd_req.roomname);
            log::error!("{}", msg);
            return Err(Status::invalid_argument(msg));
        }

        let message = &sd_req.message.unwrap();
        room.messages.push(message.clone());

        Ok(Response::new(chat::ServerResponse::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
