extern crate futures;
extern crate steemit_api;
extern crate tokio_core;

use tokio_core::reactor::Core;
use futures::Future;
use steemit_api::{message, Api};

fn main() {
    let mut core = Core::new().unwrap();
    let api = Api::new("https://api.steemit.com/".parse().unwrap(), &core.handle());
    let f = api.request(
        [
            &message::RequestGetDiscussionsByBlog {
                limit: 1,
                tag: "phc",
            },
        ].as_ref(),
    ).map(|res| println!("test: {:?}", res));
    core.run(f).unwrap();
}
