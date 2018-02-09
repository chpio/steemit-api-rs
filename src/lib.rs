extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;

pub mod message;

use futures::{Future, Stream};
use tokio_core::reactor::Handle;
use message::Request;
use std::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::ser::SerializeSeq;

#[derive(Debug)]
struct RpcVersion;

impl Serialize for RpcVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for RpcVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RpcVersionVisitor;
        impl<'de> serde::de::Visitor<'de> for RpcVersionVisitor {
            type Value = RpcVersion;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a version string")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<RpcVersion, E> {
                match value {
                    "2.0" => Ok(RpcVersion),
                    _ => Err(E::invalid_value(
                        serde::de::Unexpected::Str(value),
                        &"value 2.0",
                    )),
                }
            }
        }
        deserializer.deserialize_str(RpcVersionVisitor)
    }
}

#[derive(Debug)]
struct RpcMethod;

impl Serialize for RpcMethod {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("call")
    }
}

#[derive(Debug)]
struct RequestParams<Req>(Req);

impl<Req: Request> Serialize for RequestParams<Req> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(3))?;
        seq.serialize_element(Req::API)?;
        seq.serialize_element(Req::METHOD)?;
        seq.serialize_element(&self.0)?;
        seq.end()
    }
}

#[derive(Debug, Serialize)]
struct RpcRequest<Req: Request> {
    jsonrpc: RpcVersion,
    id: serde_json::Value,
    method: RpcMethod,
    params: RequestParams<Req>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcResponse<Res> {
    // jsonrpc: RpcVersion, // broken steemit rpc server
    id: serde_json::Value,
    result: Option<Res>,
    error: Option<RpcError>,
}

#[derive(Debug)]
pub enum Error {
    NotAJsonResponse,
    Hyper(hyper::Error),
    Rpc(RpcError),
    Json(serde_json::Error),
}

pub struct Api {
    endpoint: hyper::Uri,
    client: hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>, hyper::Body>,
}

impl Api {
    pub fn new(endpoint: hyper::Uri, handle: &Handle) -> Api {
        let client = hyper::Client::configure()
            .connector(hyper_tls::HttpsConnector::new(1, handle).unwrap())
            .build(handle);

        Api {
            endpoint: endpoint,
            client: client,
        }
    }

    pub fn request<Req: Request>(&self, req: Req) -> Box<Future<Item = Req::Res, Error = Error>> {
        use hyper::header::{qitem, Accept, ContentLength, ContentType};

        let req = RpcRequest {
            jsonrpc: RpcVersion,
            id: 0.into(),
            method: RpcMethod,
            params: RequestParams(req),
        };
        let req = match serde_json::to_vec(&req) {
            Ok(vec) => {
                let mut req = hyper::Request::new(hyper::Method::Post, self.endpoint.clone());
                {
                    let mut h = req.headers_mut();
                    h.set(ContentLength(vec.len() as u64));
                    h.set(ContentType::json());
                    h.set(Accept(vec![qitem(hyper::mime::APPLICATION_JSON)]));
                }
                req.set_body(vec);
                req
            }
            Err(err) => {
                let f = futures::future::err(Error::Json(err));
                return Box::new(f);
            }
        };
        let f = self.client
            .request(req)
            .map_err(Error::Hyper)
            .and_then(|res| {
                if let Some(is_json) = res.headers()
                    .get::<ContentType>()
                    .map(|ct| ct == &ContentType::json())
                {
                    if is_json {
                        return Ok(res);
                    }
                }
                Err(Error::NotAJsonResponse)
            })
            .and_then(|res| res.body().concat2().map_err(Error::Hyper))
            .and_then(|body| {
                let res: RpcResponse<Req::Res> =
                    serde_json::from_slice(body.as_ref()).map_err(Error::Json)?;
                let err = res.error;
                let res = res.result;
                res.ok_or_else(|| Error::Rpc(err.expect("no `result` and `error`")))
            });
        Box::new(f)
    }
}
