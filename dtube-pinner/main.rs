extern crate futures;
extern crate ipfs_api;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate steemit_api;
extern crate tokio_core;
extern crate toml;

use std::io::Read;
use tokio_core::reactor::Core;
use futures::{Future, Stream};
use steemit_api::{message, Api};

#[derive(Debug, Deserialize)]
struct DtubeMetadataVideoInfo<'a> {
    snaphash: &'a str,
    spritehash: &'a str,
}

#[derive(Debug, Deserialize)]
struct DtubeMetadataVideoContent<'a> {
    videohash: &'a str,
    video480hash: &'a str,
}

#[derive(Debug, Deserialize)]
struct DtubeMetadataVideo<'a> {
    #[serde(borrow)]
    info: DtubeMetadataVideoInfo<'a>,
    #[serde(borrow)]
    content: DtubeMetadataVideoContent<'a>,
}

#[derive(Debug, Deserialize)]
struct DtubeMetadata<'a> {
    #[serde(borrow)]
    video: DtubeMetadataVideo<'a>,
}

#[derive(Debug, Deserialize)]
pub struct Config<'a> {
    #[serde(borrow)]
    pub names: Vec<&'a str>,
    pub concurrent_requests: usize,
    pub concurrent_pins: usize,
    pub interval_ms: u64,
    pub pin_snaphash: bool,
    pub pin_spritehash: bool,
    pub pin_videohash: bool,
    pub pin_video480hash: bool,
}

#[derive(Debug)]
pub enum Error {
    SteemIt(steemit_api::Error),
    Ipfs(ipfs_api::response::Error),
    StdIo(std::io::Error),
}

fn main() {
    let mut config = Vec::new();
    std::io::stdin()
        .read_to_end(&mut config)
        .expect("Couldn't load config");
    let config: Config = toml::from_slice(&config).expect("Couldn't parse config");
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let api = Api::new("https://api.steemit.com/".parse().unwrap(), &handle);
    let ipfs = ipfs_api::IpfsClient::default(&handle);
    let runner = tokio_core::reactor::Interval::new_at(
        std::time::Instant::now(),
        std::time::Duration::from_millis(config.interval_ms),
        &handle,
    ).unwrap()
        .map_err(Error::StdIo)
        .map(|_| {
            let name_futures = config.names.iter().map(|name| {
                api.request(
                    [
                        &message::RequestGetDiscussionsByBlog {
                            limit: 100,
                            tag: name,
                        },
                    ].as_ref(),
                ).map_err(Error::SteemIt)
            });
            futures::stream::iter_ok(name_futures)
        })
        .flatten()
        .buffer_unordered(config.concurrent_requests)
        .map(|res| {
            let pin_futures = res.0
                .into_iter()
                .filter_map(|entry| {
                    if entry.category != "dtube" {
                        return None;
                    }
                    let meta: DtubeMetadata = match serde_json::from_str(&entry.json_metadata) {
                        Ok(meta) => meta,
                        Err(err) => {
                            println!(
                                "Couldn't deserialize dtube meta: `{:?}` err: `{}`",
                                entry.json_metadata, err
                            );
                            return None;
                        }
                    };
                    let snaphash = bool_to_option(config.pin_snaphash)
                        .and_then(|_| str_to_option(meta.video.info.snaphash));
                    let spritehash = bool_to_option(config.pin_spritehash)
                        .and_then(|_| str_to_option(meta.video.info.spritehash));
                    let videohash = bool_to_option(config.pin_videohash)
                        .and_then(|_| str_to_option(meta.video.content.videohash));
                    let video480hash = bool_to_option(config.pin_video480hash)
                        .and_then(|_| str_to_option(meta.video.content.video480hash));
                    let pin_futures: Vec<_> = [snaphash, spritehash, videohash, video480hash]
                        .into_iter()
                        .cloned()
                        .filter_map(|hash| hash)
                        .map(|hash| ipfs.pin_add(hash, true).map_err(Error::Ipfs))
                        .collect();
                    Some(pin_futures)
                })
                .flat_map(|e| e);
            futures::stream::iter_ok(pin_futures)
        })
        .flatten()
        .buffer_unordered(config.concurrent_pins)
        .for_each(|_| Ok(()));
    core.run(runner).unwrap();
}

fn str_to_option(s: &str) -> Option<&str> {
    if !s.is_empty() {
        Some(s)
    } else {
        None
    }
}

fn bool_to_option(b: bool) -> Option<()> {
    match b {
        true => Some(()),
        false => None,
    }
}
