use std::{error::Error, io::Write};

use chat_core::{
    constants::{HOST, PORT},
    protocol::{Message, MessageType},
};
use tokio::{net::TcpStream, sync::mpsc};

const TRACING_LEVEL: tracing::Level = tracing::Level::DEBUG;

#[derive(Debug)]
pub struct Application {}

impl Application {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        use tracing_subscriber::fmt::format::FmtSpan;
        tracing_subscriber::fmt()
            .with_max_level(TRACING_LEVEL)
            .compact()
            .with_span_events(FmtSpan::FULL)
            .init();

        Ok(Application::default())
    }

    fn get_user_data() -> (String, String) {
        let mut username = String::new();
        let mut password = String::new();

        print!("Enter username: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut username).unwrap();

        print!("Enter password: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut password).unwrap();

        (username, password)
    }

    fn get_message_data() -> (String, String) {
        let mut recipient = String::new();
        let mut message = String::new();

        print!("Enter recipient: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut recipient).unwrap();

        print!("Enter message: ");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut message).unwrap();

        (recipient, message)
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        tracing::debug!("Starting application");
        tracing::debug!("Connecting to server");

        let stream = TcpStream::connect((HOST, PORT)).await?;
        let stream_addr = stream.peer_addr()?;

        let (reader, writer) = stream.into_split();
        let (tx, rx) = mpsc::unbounded_channel::<Message>();

        let (hdc_tx, hdc_rx) = mpsc::channel::<bool>(1);
        let (sdc_tx, sdc_rx) = mpsc::channel::<bool>(1);

        let send_h = tokio::spawn(Self::handle_send(writer, rx, sdc_tx, hdc_rx));
        let recv_h = tokio::spawn(Self::handle_receive(reader, tx.clone(), hdc_tx, sdc_rx));

        tracing::debug!("Connected to server {}", stream_addr);

        loop {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();

            let message = match input.trim() {
                "new" => {
                    let (username, password) = Self::get_user_data();
                    Message::auth_create(username.trim(), password.trim())
                }
                "auth" => {
                    let (username, password) = Self::get_user_data();
                    Message::auth(username.trim(), password.trim())
                }
                "msg" => {
                    let (recipient, message) = Self::get_message_data();
                    Message::direct_message_send(recipient.trim(), message.trim())
                }
                "log" => Message::SERVER_DEBUG_LOG,
                "dc" => Message::DISCONNECT,
                "shutdown" => Message::server_shutdown(5),
                _ => Message::heartbeat(),
            };

            if tx.is_closed() {
                break;
            }

            let msg_type = message.message_type();
            if let Err(e) = tx.send(message) {
                tracing::error!("Error sending message: {}", e);
                break;
            }
            if msg_type == MessageType::Disconnect {
                break;
            }
        }

        send_h.await?;
        recv_h.await?;

        tracing::debug!("Closing connection");

        Ok(())
    }

    async fn handle_send(
        mut writer: tokio::net::tcp::OwnedWriteHalf,
        mut rx: mpsc::UnboundedReceiver<Message>,
        dc_tx: mpsc::Sender<bool>,
        mut dc_rx: mpsc::Receiver<bool>,
    ) {
        loop {
            if dc_rx.try_recv().is_ok() {
                break;
            }

            if let Some(message) = rx.recv().await {
                if message.is(MessageType::Break) {
                    // dc_tx.try_send(true).unwrap();
                    break;
                }
                tracing::debug!("Sending message: {:?}", message.message_type());
                message.send(&mut writer).await.unwrap();
                if message.is(MessageType::Disconnect) {
                    dc_tx.try_send(true).unwrap();
                    break;
                }
            }
        }
    }

    async fn handle_receive(
        mut reader: tokio::net::tcp::OwnedReadHalf,
        tx: mpsc::UnboundedSender<Message>,
        dc_tx: mpsc::Sender<bool>,
        mut dc_rx: mpsc::Receiver<bool>,
    ) {
        loop {
            if dc_rx.try_recv().is_ok() {
                break;
            }

            if !Message::has_header_start(&mut reader).await {
                continue;
            }

            let message = Message::receive(&mut reader).await;
            match message {
                Ok(message) => {
                    tracing::debug!("Received message: {:?}", message.message_type());
                    match message.message_type() {
                        MessageType::Disconnect => {
                            if let Err(e) = dc_tx.try_send(true) {
                                tracing::warn!("Error sending disconnect message: {}", e);
                            }
                            tx.send(Message::BREAK).unwrap();
                            break;
                        }
                        MessageType::Heartbeat => {
                            tx.send(Message::heartbeat()).unwrap();
                        }
                        MessageType::ServerShutdownWarning => {
                            let data = message.payload().get_data();
                            let timeout = u64::from_be_bytes(data[0].clone().try_into().unwrap());
                            tracing::warn!("Server shutting down in {} seconds", timeout);
                        }
                        MessageType::MessageError => {
                            let data = message.payload().get_data();
                            let error = std::str::from_utf8(&data[0]).unwrap();
                            tracing::error!("Could not send message | Error: {}", error);
                        }
                        MessageType::DirectMessageReceive => {
                            let data = message.payload().get_data();
                            let sender = std::str::from_utf8(&data[0]).unwrap();
                            let message = std::str::from_utf8(&data[1]).unwrap();
                            tracing::info!("Message from {}: {}", sender, message);
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    tracing::error!("Error receiving message: {}", e);
                    break;
                }
            }
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        Self {}
    }
}
