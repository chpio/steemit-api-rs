use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde::de::{Deserialize, Deserializer, Error, Unexpected, Visitor};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Version;

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct VersionVisitor;
        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = Version;

            fn expecting(&self, formatter: &mut Formatter) -> FmtResult {
                formatter.write_str("a version string")
            }

            fn visit_str<E: Error>(self, value: &str) -> Result<Version, E> {
                match value {
                    "2.0" => Ok(Version),
                    _ => Err(E::invalid_value(Unexpected::Str(value), &"value 2.0")),
                }
            }
        }
        deserializer.deserialize_str(VersionVisitor)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    pub id: Value,
}

/// A response to an RPC.
///
/// It is created by the methods on [Request](struct.Request.html).
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    jsonrpc: Version,
    pub result: Result<Value, RpcError>,
    pub id: Value,
}

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sub = serializer.serialize_struct("Response", 3)?;
        sub.serialize_field("jsonrpc", &self.jsonrpc)?;
        match self.result {
            Ok(ref value) => sub.serialize_field("result", value),
            Err(ref err) => sub.serialize_field("error", err),
        }?;
        sub.serialize_field("id", &self.id)?;
        sub.end()
    }
}

/// Deserializer for `Option<Value>` that produces `Some(Value::Null)`.
///
/// The usual one produces None in that case. But we need to know the difference between
/// `{x: null}` and `{}`.
fn some_value<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Deserialize::deserialize(deserializer).map(Some)
}

/// A helper trick for deserialization.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResponse {
    // It is actually used to eat and sanity check the deserialized text
    #[allow(dead_code)]
    jsonrpc: Version,
    // Make sure we accept null as Some(Value::Null), instead of going to None
    #[serde(default, deserialize_with = "some_value")]
    result: Option<Value>,
    error: Option<RpcError>,
    id: Value,
}

// Implementing deserialize is hard. We sidestep the difficulty by deserializing a similar
// structure that directly corresponds to whatever is on the wire and then convert it to our more
// convenient representation.
impl<'de> Deserialize<'de> for Response {
    #[allow(unreachable_code)] // For that unreachable below
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wr: WireResponse = Deserialize::deserialize(deserializer)?;
        let result = match (wr.result, wr.error) {
            (Some(res), None) => Ok(res),
            (None, Some(err)) => Err(err),
            _ => {
                let err = D::Error::custom("Either 'error' or 'result' is expected, but not both");
                return Err(err);
            }
        };
        Ok(Response {
            jsonrpc: Version,
            result: result,
            id: wr.id,
        })
    }
}
