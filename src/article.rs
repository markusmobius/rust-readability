use crate::{
    check::is_space, metadata::Metadata, render, Dom, LocalTimeZone, NodeId, Timestamp,
    TimestampError,
};
use std::{
    fmt,
    io::{self, Write},
};

#[derive(Debug)]
pub enum Error {
    DocumentTooLarge(usize),
    MissingNode,
    CharsetNotDetected,
    TimestampMissing,
    TimestampParse {
        field: &'static str,
        source: TimestampError,
    },
    Html(String),
    ParseInput(Box<Error>),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentTooLarge(count) => {
                write!(formatter, "documents too large: {count} elements")
            }
            Self::MissingNode => formatter.write_str("the Node field is nil"),
            Self::CharsetNotDetected => formatter.write_str("Charset not detected."),
            Self::TimestampMissing => formatter.write_str("timestamp not found in document"),
            Self::TimestampParse { field, source } => {
                write!(formatter, "error parsing {field}: {source}")
            }
            Self::Html(message) => formatter.write_str(message),
            Self::ParseInput(error) => write!(formatter, "failed to parse input: {error}"),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ParseInput(error) => Some(error.as_ref()),
            Self::TimestampParse { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug)]
pub struct Article {
    pub document: Dom,
    pub node: Option<NodeId>,
    pub(crate) metadata: Metadata,
    pub(crate) language: String,
}

impl Article {
    fn field(&self, name: &str) -> &str {
        self.metadata
            .get(name)
            .map(String::as_str)
            .unwrap_or_default()
    }
    pub fn title(&self) -> &str {
        self.field("title")
    }
    pub fn byline(&self) -> &str {
        self.field("byline")
    }
    pub fn site_name(&self) -> &str {
        self.field("siteName")
    }
    pub fn image_url(&self) -> &str {
        self.field("image")
    }
    pub fn favicon(&self) -> &str {
        self.field("favicon")
    }
    pub fn language(&self) -> &str {
        &self.language
    }
    pub fn published_time_raw(&self) -> &str {
        self.field("publishedTime")
    }
    pub fn modified_time_raw(&self) -> &str {
        self.field("modifiedTime")
    }

    pub fn published_time(&self) -> Result<Timestamp, Error> {
        self.timestamp("publishedTime", LocalTimeZone::system())
    }
    pub fn modified_time(&self) -> Result<Timestamp, Error> {
        self.timestamp("modifiedTime", LocalTimeZone::system())
    }
    pub fn published_time_with_local_timezone(
        &self,
        local: &LocalTimeZone,
    ) -> Result<Timestamp, Error> {
        self.timestamp("publishedTime", local)
    }
    pub fn modified_time_with_local_timezone(
        &self,
        local: &LocalTimeZone,
    ) -> Result<Timestamp, Error> {
        self.timestamp("modifiedTime", local)
    }

    fn timestamp(&self, field: &'static str, local: &LocalTimeZone) -> Result<Timestamp, Error> {
        let input = self.field(field);
        if input.is_empty() {
            return Err(Error::TimestampMissing);
        }
        crate::timestamp::parse_with_local_timezone(input, local)
            .map(|(timestamp, _)| timestamp)
            .map_err(|message| Error::TimestampParse {
                field,
                source: TimestampError(message),
            })
    }

    pub fn excerpt(&self) -> String {
        let mut excerpt = self.field("excerpt").to_owned();
        if excerpt.is_empty() {
            if let Some(root) = self.node {
                if let Some(&paragraph) = self.document.tagged(root, "p").first() {
                    excerpt = render::inner_text(&self.document, paragraph);
                }
            }
        }
        excerpt
            .split(is_space)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn html(&self) -> Result<String, Error> {
        let mut output = Vec::new();
        self.render_html(&mut output)?;
        Ok(String::from_utf8(output).unwrap())
    }
    pub fn text(&self) -> Result<String, Error> {
        Ok(render::inner_text(
            &self.document,
            self.node.ok_or(Error::MissingNode)?,
        ))
    }
    pub fn render_html(&self, writer: impl Write) -> Result<(), Error> {
        render::html_dom(&self.document, self.node.ok_or(Error::MissingNode)?, writer)
    }
    pub fn render_text(&self, mut writer: impl Write) -> Result<(), Error> {
        writer.write_all(self.text()?.as_bytes())?;
        Ok(())
    }
}
