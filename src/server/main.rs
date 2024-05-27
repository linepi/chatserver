use chatserver::server::slib;
use chatserver::chat::chat_server::ChatServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mychatserver = slib::MyChatServer::default();
    mychatserver.config.read_file("src/server/config");
    mychatserver.init()?;
    let addr = mychatserver.config.addr.parse().unwrap();
    chatserver::log_init().unwrap();

    tonic::transport::Server::builder()
        .add_service(ChatServer::new(mychatserver))
        .serve(addr)
        .await?;

    Ok(())
}



