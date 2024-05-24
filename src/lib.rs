use tonic::{Request, Response, Status};
use chat::chat_server::Chat;
use std::sync::RwLock;
use std::sync::Arc;

pub mod common;

pub mod chat {
    tonic::include_proto!("chat");
}

// client lib

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
        // if  {
        //     printlines.push(response.extra_info.clone());
        // }
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
            lasttime: cli.state.read().unwrap().lastupdate_time,
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
    state: RwLock<ServerState>,
}

#[derive(Default)]
pub struct ServerState {
    // map: HashMap<i32, chat::Client>,
    rooms: Vec<RwLock<chat::Room>>,
}

impl MyChatServer {
    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let serv = MyChatServer::default();
        {
            // for simplisity, load all roominfos
            let readdir = std::fs::read_dir("data")?;
            for diri in readdir {
                let entry = diri?;
                let path = entry.path();
                let pathstr = path.to_str().unwrap().to_string();
                if pathstr.contains("room") {
                    serv.state.write().unwrap().rooms.push(RwLock::new(chat::Room::from_file(&pathstr).unwrap()));
                }
            }
        }
        Ok(serv)
    }
}

impl Drop for MyChatServer {
    // serialize all rooms
    fn drop(&mut self) {
        for room in self.state.read().unwrap().rooms.iter() {
            let room_reader = room.read().unwrap();
            let _ = room_reader.to_file(&format!("data/room_{}", &room_reader.name));
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

        let state = self.state.read().unwrap();
        let room = state.rooms.iter().find(|x| {
            let room_reader = x.read().unwrap();
            room_reader.name == hb_req.clone().roomname
        });

        let mut response = chat::ServerResponse::default();
        if room.is_none() {
            // no room found, create it
            let now = common::now_milli_seconds();
            // unlock the read lock to create write lock 
            drop(state);
            let mut state_writer = self.state.write().unwrap();
            state_writer.rooms.push(RwLock::new(chat::Room{
                created_time: now,
                history_visible: true,
                manner: hb_req.client.clone(),
                messages: vec![],
                clients: vec![hb_req.client.clone().unwrap()],
                name: hb_req.roomname,
            }));
        } else {
            // room found, check if client exists in this room
            let room_reader = room.unwrap().read().unwrap();
            let client_exist_in_room = common::client_in_room(hb_req.client.as_ref().unwrap(), &room_reader);

            if !client_exist_in_room {
                drop(room_reader);
                let mut room_writer = room.unwrap().write().unwrap();
                room_writer.clients.push(hb_req.client.unwrap().clone());
                response.messages = room_writer.messages.clone();
            } else {
                for i in 0..room_reader.messages.len() {
                    if room_reader.messages[i].time > hb_req.lasttime {
                        response.messages.push(room_reader.messages[i].clone()); 
                    }
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

        let state = self.state.read().unwrap();
        let room = state.rooms.iter().find(|x| {
            let room_reader = x.read().unwrap();
            room_reader.name == sd_req.clone().roomname
        });
        if room.is_none() {
            log::error!("room is none");
            return Err(Status::invalid_argument("room is none"));
        }

        let mut room_writer = room.unwrap().write().unwrap();
        let client_exist_in_room = common::client_in_room_w(sd_req.client.as_ref().unwrap(), &room_writer);

        if !client_exist_in_room {
            let msg = format!("client not exist in room {}", sd_req.roomname);
            log::error!("{}", msg);
            return Err(Status::invalid_argument(msg));
        }

        let message = &sd_req.message.unwrap();

        room_writer.messages.push(message.clone());

        Ok(Response::new(chat::ServerResponse::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
