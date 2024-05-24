use tonic::{Request, Response, Status};
use std::sync::RwLock;
use crate::chat;
use crate::chat::chat_server::Chat;
use crate::common;

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
        log::info!("Got a heartbeat request from {:?}", request.remote_addr());
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
        log::info!("Got a send request from {:?}", request.remote_addr());
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
        log::info!("add message[{}] to room[{}]", 
            String::from_utf8(message.bytes.clone()).unwrap(), 
            &sd_req.roomname);
        room_writer.messages.push(message.clone());

        Ok(Response::new(chat::ServerResponse::default()))
    }
}
