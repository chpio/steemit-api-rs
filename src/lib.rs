extern crate futures;
extern crate websocket;

mod message;

use futures::{Future, Sink, Stream};
use futures::unsync::mpsc;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
use websocket::async::Handle;
use websocket::client::builder::Url;

pub struct Api {
    sender: mpsc::Sender<OwnedMessage>,
}

impl Api {
    pub fn new(
        handle: &Handle,
    ) -> Box<
        Future<
            Item = (Box<Future<Item = (), Error = WebSocketError>>, Api),
            Error = WebSocketError,
        >,
    > {
        Self::with_url(&Url::parse("wss://gtg.steem.house:8090").unwrap(), handle)
    }

    pub fn with_url(
        address: &Url,
        handle: &Handle,
    ) -> Box<
        Future<
            Item = (Box<Future<Item = (), Error = WebSocketError>>, Api),
            Error = WebSocketError,
        >,
    > {
        let future = ClientBuilder::from_url(address)
            .async_connect(None, handle)
            .map(|(client, _)| {
                let (sink, stream) = client.split();

                let (sender, receiver) = mpsc::channel(8);

                let runner = stream
                    .filter_map(|message| {
                        println!("Received Message: {:?}", message);
                        match message {
                            OwnedMessage::Close(e) => Some(OwnedMessage::Close(e)),
                            OwnedMessage::Ping(d) => Some(OwnedMessage::Pong(d)),
                            _ => None,
                        }
                    })
                    .select(receiver.map_err(|_| WebSocketError::NoDataAvailable))
                    .map(|m| {
                        println!("sent Message: {:?}", m);
                        m
                    })
                    .forward(sink)
                    .map(|_| ());
                let runner = Box::new(runner) as Box<Future<Item = (), Error = WebSocketError>>;
                (runner, Api { sender: sender })
            });
        Box::new(future)
    }

    pub fn send(self) -> Box<Future<Item = Api, Error = WebSocketError>> {
        let future = self.sender
            .send(OwnedMessage::Text())
            .map_err(|_| WebSocketError::NoDataAvailable)
            .map(|sender| Api { sender: sender });
        Box::new(future)
    }
}
