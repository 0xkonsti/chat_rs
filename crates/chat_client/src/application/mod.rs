use std::error::Error;

use chat_core::{
    constants::{HOST, PORT},
    protocol::{Message, MessageType},
};
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::mpsc};

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

        std::io::stdin().read_line(&mut String::new()).unwrap();

        tx.send(Message::DISCONNECT).unwrap();

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
                            break;
                        }
                        MessageType::Heartbeat => {
                            tx.send(Message::heartbeat()).unwrap();
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
