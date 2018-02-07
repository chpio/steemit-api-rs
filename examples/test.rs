extern crate futures;
extern crate steemit_api;
extern crate tokio_core;

use tokio_core::reactor::Core;
use futures::Future;

fn main() {
    let mut core = Core::new().unwrap();
    let runner =
        steemit_api::Api::new(&core.handle()).and_then(|(runner, api)| api.send().join(runner));
    core.run(runner).unwrap();
}
