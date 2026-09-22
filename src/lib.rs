#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod article;
mod check;
mod cleanup;
mod dom;
mod encoding;
mod entities;
mod extract;
mod html;
mod json;
mod metadata;
mod options;
mod parser;
mod postprocess;
mod prepare;
pub mod render;
mod score;
mod shared;
mod sort;
mod timestamp;
mod url;

#[cfg(test)]
mod upstream_tests;

pub use article::{Article, Error};
pub use check::check_document;
pub use dom::{Attribute, Children, Document, Kind, Node, NodeData, NodeId, Text, Tree as Dom};
pub use html::{
    parse_dom, parse_html, parse_html_direct, parse_html_into, HtmlTreeSink, HtmlTreeStore,
};
pub use options::Options;
pub use parser::{from_document, from_html, from_reader, Parser};
pub use shared::DomSource;
pub use timestamp::{LocalTimeZone, Timestamp, TimestampError};
pub use url::{Url, UrlError};

pub fn decode_bytes(source: &[u8]) -> Result<String, Error> {
    encoding::decode(source).map_err(|error| Error::ParseInput(Box::new(error)))
}

pub fn parse_bytes(source: &[u8]) -> Result<Dom, Error> {
    let decoded = decode_bytes(source)?;
    Ok(parse_dom(&decoded))
}

pub const GO_REFERENCE_MODULE: &str = "github.com/markusmobius/go-readabilityV2";
pub const GO_UPSTREAM_COMMIT: &str = "b18540d99ebf105cd67122585a0a41ec299b70bc";
