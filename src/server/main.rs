use chatserver::server::slib;
use chatserver::chat::chat_server::ChatServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:15535".parse().unwrap();
    let mychatserver = slib::MyChatServer::init().unwrap();
    chatserver::log_init().unwrap();

    tonic::transport::Server::builder()
        .add_service(ChatServer::new(mychatserver))
        .serve(addr)
        .await?;

    Ok(())
}


