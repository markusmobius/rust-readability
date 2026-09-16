mod dateparse;
mod layout;
mod location;

pub use location::LocalTimeZone;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanoseconds: u32,
    pub zone: String,
    pub offset: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimestampError(pub(crate) String);

impl std::fmt::Display for TimestampError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TimestampError {}

pub(crate) use dateparse::parse_with_local_timezone;
pub(crate) use layout::parse as parse_layout;
