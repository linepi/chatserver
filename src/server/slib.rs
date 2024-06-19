#![allow(unused_variables)]

use tonic::{Request, Response, Status};
use std::sync::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use std::collections::HashSet;
use crate::chat;
use crate::chat::chat_server::Chat;
use crate::common;

#[derive(Default)]
pub struct Config {
    pub addr: String,
    pub datapath: String,
}

impl Config {
    pub fn read_file(&mut self, path: &str) {
        let content = String::from_utf8(std::fs::read(path).unwrap()).unwrap(); 
        let lines: Vec<&str> = content.split_whitespace().collect();
        self.addr = lines[0].to_string();
        self.datapath = lines[1].to_string();
    }
}

#[derive(Default)]
pub struct MyChatServer {
    state: RwLock<ServerState>,
    pub config: Config,
}

#[derive(Default)]
pub struct ServerState {
    rooms: Vec<RwLock<chat::Room>>,
    users: Vec<RwLock<chat::User>>,
    // map roomname to a bunch of online users
    onlinemap: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    // note the latest time of user action at room
    clientuptime: Arc<RwLock<HashMap<String, u64>>>,
    uptime: Arc<RwLock<u64>>,
}

impl MyChatServer {
    pub fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut state = self.state.write().unwrap();
        {
            std::fs::create_dir_all(&self.config.datapath)?;
            // for simplisity, load all roominfos
            let readdir = std::fs::read_dir(&self.config.datapath)?;
            for diri in readdir {
                let entry = diri?;
                let path = entry.path();
                let pathstr = path.to_str().unwrap().to_string();
                if pathstr.contains("room") {
                    state.rooms.push(RwLock::new(chat::Room::from_file(&pathstr).unwrap()));
                    state.onlinemap.write().unwrap().insert(
                        pathstr[self.config.datapath.len()+6..].to_string(), HashSet::new());
                }
                if pathstr.contains("user") {
                    state.users.push(RwLock::new(chat::User::from_file(&pathstr).unwrap()));
                }
            }
        }
        *state.uptime.write().unwrap() = common::now_milli_seconds();
        let uptime = Arc::clone(&state.uptime);
        let onlinemap = Arc::clone(&state.onlinemap);
        let clientuptime = Arc::clone(&state.clientuptime);
        drop(state);

        std::thread::spawn(move || loop {
            let uptimeval = common::now_milli_seconds();
            *uptime.write().unwrap() = uptimeval;

            let mut om_writer = onlinemap.write().unwrap();
            for (_roomname, onlineset) in om_writer.iter_mut() {
                if onlineset.is_empty() {
                    continue;
                }
                let cu_reader = clientuptime.read().unwrap(); 
                for (username, t) in cu_reader.iter() {
                    if uptimeval > *t && uptimeval - *t > 5000 {
                        // println!("{:?} remove {}", onlineset, username);
                        onlineset.remove(username);
                    }
                }
            }
            drop(om_writer);
            std::thread::sleep(std::time::Duration::from_millis(1000));    
        });
        Ok(())
    }
}

impl Drop for MyChatServer {
    // serialize all rooms
    fn drop(&mut self) {
        self.serialize();
    }
}

impl MyChatServer {
    fn serialize(&self) {
        let datapath = &self.config.datapath;
        for room in self.state.read().unwrap().rooms.iter() {
            let room_reader = room.read().unwrap();
            let _ = room_reader.to_file(&format!("{}/room_{}", datapath, &room_reader.name));
        }
        for user in self.state.read().unwrap().users.iter() {
            let user_reader = user.read().unwrap();
            let _ = user_reader.to_file(&format!("{}/user_{}", datapath, user_reader.name));
        }
    }
}

#[tonic::async_trait]
impl Chat for MyChatServer {
    // now signup and login is conbined together
    async fn signup(
        &self,
        request: Request<chat::UserSignupRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        let req = request.into_inner();
        let username = &req.client.as_ref().unwrap().user.as_ref().unwrap().name;
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if req.password.is_empty() { // create room
            return Err(Status::invalid_argument("password is empty"));
        }
        log::info!("try signup: username: {}, password: {}", username, &req.password);

        let state = self.state.read().unwrap();
        let user = state.users.iter().find(|u| {
            let user_reader = u.read().unwrap();
            user_reader.name == *username
        });

        let mut response = chat::ServerResponse::default();
        // user not exist, signup
        if user.is_none() {
            drop(state);
            let mut state_writer = self.state.write().unwrap();
            state_writer.users.push(RwLock::new(chat::User{
                name: username.clone(),
                gender: Some(1),
                password: req.password,
            }));
        } else {
            // sign in
            // check password
            if user.unwrap().read().unwrap().password != req.password {
                response.code = chat::ResponseCode::PasswordWrong as i32;
            }
        }

        // TODO more efficient serialize
        self.serialize();
        
        Ok(Response::new(response))
    }

    async fn join(
        &self,
        request: Request<chat::JoinRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        let req = request.into_inner();
        let username = &req.client.as_ref().unwrap().user.as_ref().unwrap().name;
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if req.roomname.is_empty() { // create room
            log::error!("roomname is none");
            return Err(Status::invalid_argument("roomname is none"));
        }

        let roomname = req.roomname.clone();
        let state = self.state.read().unwrap();
        let room = state.rooms.iter().find(|x| {
            let room_reader = x.read().unwrap();
            room_reader.name == roomname
        });

        let mut response = chat::ServerResponse::default();
        if room.is_none() {
            return Err(Status::invalid_argument("join a non exist room"));
        } 

        let mut room_writer = room.unwrap().write().unwrap();
        if !room_writer.clients.contains(&(req.client.clone().unwrap())) {
            room_writer.clients.push(req.client.clone().unwrap());
        }
        response.messages = room_writer.messages.clone();

        let mut map = state.onlinemap.write().unwrap();
        map.get_mut(&roomname).unwrap().insert(username.clone());

        Ok(Response::new(response))

    }

    async fn heartbeat(
        &self,
        request: Request<chat::HeartBeatRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        // log::info!("Got a heartbeat request from {:?}", request.remote_addr());
        let req = request.into_inner();
        let username = &req.client.as_ref().unwrap().user.as_ref().unwrap().name;
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if req.roomname.is_empty() { // create room
            log::error!("roomname is none");
            return Err(Status::invalid_argument("roomname is none"));
        }

        let roomname = req.roomname.clone();
        let state = self.state.read().unwrap();
        let room = state.rooms.iter().find(|x| {
            let room_reader = x.read().unwrap();
            room_reader.name == roomname
        });

        let mut response = chat::ServerResponse::default();
        if room.is_none() {
            return Err(Status::invalid_argument("heartbeat a non exist room"));
        }

        // room found, check if client exists in this room
        let room_reader = room.unwrap().read().unwrap();
        assert!(common::client_in_room(req.client.as_ref().unwrap(), &room_reader));
        for i in (req.msgnum as usize)..room_reader.messages.len() {
            response.messages.push(room_reader.messages[i].clone()); 
            log::info!("client [{}] recv new msg", username);
        }

        // update clientuptime
        let mut clientuptime = state.clientuptime.write().unwrap();
        clientuptime.insert(username.clone(), common::now_milli_seconds());
        Ok(Response::new(response))
    }

    async fn send(
        &self, 
        request: Request<chat::SendRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        log::info!("Got a send request from {:?}", request.remote_addr());
        let req = request.into_inner();
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if req.message.is_none() {
            log::error!("message is none");
            return Err(Status::invalid_argument("message is none"));
        }
        if req.roomname.is_empty() { // create room
            log::error!("roomname is empty");
            return Err(Status::invalid_argument("roomname is empty"));
        }

        let state = self.state.read().unwrap();
        let room = state.rooms.iter().find(|x| {
            let room_reader = x.read().unwrap();
            room_reader.name == req.clone().roomname
        });
        if room.is_none() {
            log::error!("room is none");
            return Err(Status::invalid_argument("room is none"));
        }

        let mut room_writer = room.unwrap().write().unwrap();
        let client_exist_in_room = common::client_in_room_w(req.client.as_ref().unwrap(), &room_writer);

        if !client_exist_in_room {
            let msg = format!("client not exist in room {}", req.roomname);
            log::error!("{}", msg);
            return Err(Status::invalid_argument(msg));
        }

        let message = &req.message.unwrap();
        log::info!("add message[{}] to room[{}]", 
            String::from_utf8(message.bytes.clone()).unwrap(), 
            &req.roomname);
        room_writer.messages.push(message.clone());

        drop(room_writer);
        // TODO more efficient serialize
        self.serialize();

        Ok(Response::new(chat::ServerResponse::default()))
    }

    async fn getrooms(
        &self, 
        request: Request<chat::GetRoomsRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        let req = request.into_inner();
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }

        let state = self.state.read().unwrap();
        let map = state.onlinemap.write().unwrap();
        let mut response = chat::ServerResponse::default();
        state.rooms.iter().for_each(|x| {
            let roomname = &x.read().unwrap().name;
            response.roominfos.push(chat::RoomInfo{
                name: roomname.to_string(),
                manner: x.read().unwrap().manner.clone(),
                online_users: match map.get(roomname) {
                    Some(v) => { 
                        v.iter().map(|s_ref| s_ref.clone()).collect()
                    },
                    None => vec![],
                },
                password: x.read().unwrap().password.clone(),
            });
        });
        Ok(Response::new(response))
    }

    async fn getusers(
        &self, 
        request: Request<chat::GetUsersRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        let req = request.into_inner();
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }

        let state = self.state.read().unwrap();
        let mut response = chat::ServerResponse::default();
        state.users.iter().for_each(|x| {
            let user = x.read().unwrap();
            response.users.push(user.clone());
        });
        Ok(Response::new(response))
    }

    async fn createroom(
        &self, 
        request: Request<chat::CreateRoomRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        log::info!("create room");
        let req = request.into_inner();
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if req.roomname.is_empty() { // create room
            log::error!("roomname is none");
            return Err(Status::invalid_argument("roomname is none"));
        }

        let state = self.state.read().unwrap();
        let room = state.rooms.iter().find(|x| {
            let room_reader = x.read().unwrap();
            room_reader.name == req.roomname.clone()
        });

        if !room.is_none() {
            log::error!("create existed room");
            return Err(Status::invalid_argument("create existed room"));
        }

        let response = chat::ServerResponse::default();
        // unlock the read lock to create write lock 
        drop(state);
        let mut state_writer = self.state.write().unwrap();
        state_writer.rooms.push(RwLock::new(chat::Room{
            created_time: common::now_milli_seconds(),
            history_visible: req.history_visible,
            manner: req.client.clone(),
            messages: vec![],
            clients: vec![req.client.clone().unwrap()],
            name: req.roomname.clone(),
            password: req.password,
        }));
        state_writer.onlinemap.write().unwrap().insert(req.roomname.clone(), HashSet::new());

        // TODO more efficient serialize
        drop(state_writer);
        self.serialize();

        Ok(Response::new(response))

    }

    async fn exitroom(
        &self, 
        request: Request<chat::ExitRoomRequest>
    ) -> Result<Response<chat::ServerResponse>, Status> {
        let req = request.into_inner();
        if req.client.is_none() {
            log::error!("client is none");
            return Err(Status::invalid_argument("client is none"));
        }
        if req.roomname.is_empty() { // create room
            log::error!("roomname is none");
            return Err(Status::invalid_argument("roomname is none"));
        }

        let state_reader = self.state.read().unwrap();
        let mut map = state_reader.onlinemap.write().unwrap();
        map.get_mut(&req.roomname).unwrap().remove(&req.client.as_ref().unwrap().username());
        Ok(Response::new(chat::ServerResponse::default()))
    }
}
