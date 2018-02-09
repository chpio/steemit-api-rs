use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub trait Request: Debug + Serialize {
    type Res: Response;
    const API: &'static str;
    const METHOD: &'static str;
}

pub trait Response: 'static + Debug + for<'de> Deserialize<'de> {}

#[derive(Debug, Serialize)]
pub struct RequestGetDiscussionsByBlog<'a> {
    pub limit: i32,
    pub tag: &'a str,
}

impl<'a> Request for &'a [&'a RequestGetDiscussionsByBlog<'a>] {
    type Res = ResponseGetDiscussionsByBlog;
    const API: &'static str = "database_api";
    const METHOD: &'static str = "get_discussions_by_blog";
}

#[derive(Debug, Deserialize)]
pub struct ResponseGetDiscussionsByBlogEntry {
    id: i32,
    author: String,
    permlink: String,
    body: String,
    json_metadata: String,
}

#[derive(Debug, Deserialize)]
pub struct ResponseGetDiscussionsByBlog(Vec<ResponseGetDiscussionsByBlogEntry>);

impl Response for ResponseGetDiscussionsByBlog {}
