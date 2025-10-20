use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};
use reqwest;

struct Server{
    host: String,
    port: u16,
    status: String,
    health_check_url: String,
}

struct HealthyServers {
   healthy_server_indices: Vec<usize>,
}

type AvailableServers = Vec<Server>;

impl Server {
    fn new(host: String, port: u16, status: String, health_check_url: String) -> Self {
        Self {
            host,
            port,
            status,
            health_check_url,
        }
    }
async fn is_healthy(&mut self) -> bool {
        match reqwest::get(&self.health_check_url).await {
            Ok(response) => {
                if response.status().is_success() {
                    self.status = String::from("UP");
                    true
                } else {
                    self.status = String::from("DOWN");
                    false
                }
            }
            Err(_) => {
                self.status = String::from("DOWN");
                false
            }
        }
    }
}

impl HealthyServers {
    fn new() -> Self {
        Self {
            healthy_server_indices: Vec::new(),
        }
    }
    fn add_server(&mut self, index: usize) {
       if !self.healthy_server_indices.contains(&index) {
           self.healthy_server_indices.push(index);
       }
    }
    fn remove_server(&mut self, index: usize) {
        self.healthy_server_indices.retain(|&idx| idx != index)
    }
    async fn periodic_health_check(mut self, available_servers: &mut AvailableServers) {
        if available_servers.len() > 0 {
            error!("No servers available.")
        }
        for (index, server) in available_servers.iter_mut().enumerate() {
            if server.is_healthy().await{
                self.add_server(index);
            } else {
                self.remove_server(index);
            }
        }
    }
}

fn get_available_server(){
    //this will return the first available server

}


async fn handle_incoming(mut stream: TcpStream) {
    info!("New connection from: {}", stream.peer_addr().unwrap());

    let mut buffer = [0; 1024];
    match stream.try_read(&mut buffer) {
        Ok(n) if n == 0 => return,
        Ok(_) => {
            let request = String::from_utf8_lossy(&buffer);
            if request.starts_with("GET") {
                let response = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
                if let Err(e) = stream.write_all(response.as_bytes()).await {
                    error!("Failed to write response: {}", e);
                }
            }
        }
        Err(e) => {
            error!("Failed to read from connection: {}", e);
            return;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    info!("Starting server on: {}", listener.local_addr()?);

    let mut server_list = AvailableServers::new();

    let mut healthy_servers = HealthyServers::new();

    tokio::spawn(async move {
        healthy_servers.periodic_health_check(&mut server_list);
    });

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(async move {
                    handle_incoming(stream).await;
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    // Ensures handle_incoming completes without panicking when given a connected TcpStream.
    #[tokio::test]
    async fn test_handle_incoming_does_not_panic() {
        // Arrange: create a pair of connected TCP streams (client and server sides).
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_fut = TcpStream::connect(addr);
        let server_fut = listener.accept();

        let (client_res, server_res) = tokio::join!(client_fut, server_fut);
        let _client = client_res.expect("client connect");
        let (server_stream, _peer) = server_res.expect("server accept");

        // Act + Assert: handle_incoming should run to completion quickly.
        timeout(Duration::from_secs(1), handle_incoming(server_stream))
            .await
            .expect("handle_incoming should not hang");
    }

    // Spins up a temporary listener (like a tiny server) that accepts one connection and then exits.
    // Verifies that a client can connect to it.
    #[tokio::test]
    async fn test_server_accepts_one_connection() {
        tracing_subscriber::fmt::try_init().ok();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Task that accepts exactly one connection and calls handle_incoming, then returns.
        let server = tokio::spawn(async move {
            let (stream, _addr) = listener.accept().await.unwrap();
            handle_incoming(stream).await;
        });

        // Connect as a client and then drop the connection.
        let _client = TcpStream::connect(addr).await.expect("client connect");

        // The server task should finish quickly after handling the single connection.
        timeout(Duration::from_secs(2), server)
            .await
            .expect("server task should finish after one connection")
            .expect("server task join should succeed");
    }
}