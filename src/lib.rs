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
mod sort;
mod timestamp;
mod url;

#[cfg(test)]
mod upstream_tests;

pub use article::{Article, Error};
pub use check::check_document;
pub use dom::{Attribute, Children, Document, Kind, Node, NodeData, NodeId, Text, Tree as Dom};
pub use html::{parse_dom, parse_html};
pub use options::Options;
pub use parser::{from_document, from_html, from_reader, Parser};
pub use timestamp::{LocalTimeZone, Timestamp, TimestampError};
pub use url::{Url, UrlError};

pub const GO_REFERENCE_MODULE: &str = "github.com/markusmobius/go-readabilityV2";
pub const GO_UPSTREAM_COMMIT: &str = "b18540d99ebf105cd67122585a0a41ec299b70bc";
