use super::TimestampError;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[derive(Clone, Debug)]
pub(super) struct Zone {
    pub name: String,
    pub offset: i32,
    pub daylight: bool,
}

#[derive(Clone, Debug)]
pub struct LocalTimeZone {
    pub(super) zones: Vec<Zone>,
    pub(super) transitions: Vec<(i64, usize)>,
    pub(super) future: Option<tz::TimeZone>,
}

impl LocalTimeZone {
    pub fn system() -> &'static Self {
        static LOCAL: std::sync::OnceLock<LocalTimeZone> = std::sync::OnceLock::new();
        LOCAL.get_or_init(system)
    }

    pub fn utc() -> Self {
        Self::fixed("UTC", 0)
    }

    pub fn fixed(name: impl Into<String>, offset: i32) -> Self {
        Self {
            zones: vec![Zone {
                name: name.into(),
                offset,
                daylight: false,
            }],
            transitions: vec![],
            future: None,
        }
    }

    pub fn from_name(name: &str) -> Result<Self, TimestampError> {
        use std::io::Read;
        if name.is_empty() || name == "UTC" {
            return Ok(Self::utc());
        }
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(include_bytes!("zoneinfo.zip")))
                .map_err(|error| TimestampError(error.to_string()))?;
        let mut entry = archive
            .by_name(name)
            .map_err(|error| TimestampError(error.to_string()))?;
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| TimestampError(error.to_string()))?;
        Self::from_tzif(&data)
    }

    pub fn from_tzif(data: &[u8]) -> Result<Self, TimestampError> {
        let parsed =
            tz::TimeZone::from_tz_data(data).map_err(|error| TimestampError(error.to_string()))?;
        let parsed = parsed.as_ref();
        let zones = parsed
            .local_time_types()
            .iter()
            .map(|zone| Zone {
                name: zone.time_zone_designation().to_owned(),
                offset: zone.ut_offset(),
                daylight: zone.is_dst(),
            })
            .collect();
        let transitions = parsed
            .transitions()
            .iter()
            .map(|transition| {
                (
                    transition.unix_leap_time(),
                    transition.local_time_type_index(),
                )
            })
            .collect();
        let future = tz::TimeZone::new(
            parsed.transitions().to_vec(),
            parsed.local_time_types().to_vec(),
            vec![],
            *parsed.extra_rule(),
        )
        .map_err(|error| TimestampError(error.to_string()))?;
        Ok(Self {
            zones,
            transitions,
            future: Some(future),
        })
    }

    pub(super) fn lookup(&self, seconds: i64) -> Zone {
        let count = self
            .transitions
            .partition_point(|&(transition, _)| transition <= seconds);
        if count == 0 {
            let first = if !self.transitions.iter().any(|&(_, index)| index == 0) {
                0
            } else {
                self.transitions
                    .first()
                    .filter(|&&(_, index)| self.zones[index].daylight)
                    .and_then(|&(_, index)| {
                        (0..index).rev().find(|&index| !self.zones[index].daylight)
                    })
                    .or_else(|| self.zones.iter().position(|zone| !zone.daylight))
                    .unwrap_or_default()
            };
            return self.zones[first].clone();
        }
        if count == self.transitions.len() {
            if let Some(future) = &self.future {
                if let Ok(zone) = future.find_local_time_type(seconds) {
                    return Zone {
                        name: zone.time_zone_designation().to_owned(),
                        offset: zone.ut_offset(),
                        daylight: zone.is_dst(),
                    };
                }
            }
        }
        self.zones[self.transitions[count - 1].1].clone()
    }

    pub(super) fn lookup_name(&self, name: &str, seconds: i64) -> Option<i32> {
        for zone in self.zones.iter().filter(|zone| zone.name == name) {
            let actual = self.lookup(seconds - zone.offset as i64);
            if actual.name == name {
                return Some(actual.offset);
            }
        }
        self.zones
            .iter()
            .find(|zone| zone.name == name)
            .map(|zone| zone.offset)
    }
}

#[cfg(windows)]
fn system() -> LocalTimeZone {
    windows::load().unwrap_or_else(LocalTimeZone::utc)
}

#[cfg(unix)]
fn system() -> LocalTimeZone {
    let mut fallback = None;
    let paths = match std::env::var_os("TZ") {
        None => vec![std::path::PathBuf::from("/etc/localtime")],
        Some(value) => {
            let value = value.to_string_lossy();
            let name = value.strip_prefix(':').unwrap_or(&value);
            if name.is_empty() || name == "UTC" {
                return LocalTimeZone::utc();
            }
            if name.starts_with('/') {
                vec![std::path::PathBuf::from(name)]
            } else {
                fallback = Some(name.to_owned());
                [
                    "/usr/share/zoneinfo",
                    "/usr/share/lib/zoneinfo",
                    "/usr/lib/locale/TZ",
                    "/etc/zoneinfo",
                ]
                .iter()
                .map(|directory| std::path::Path::new(directory).join(name))
                .collect()
            }
        }
    };
    paths
        .iter()
        .find_map(|path| {
            std::fs::read(path)
                .ok()
                .and_then(|data| LocalTimeZone::from_tzif(&data).ok())
        })
        .or_else(|| fallback.and_then(|name| LocalTimeZone::from_name(&name).ok()))
        .unwrap_or_else(LocalTimeZone::utc)
}

#[cfg(not(any(windows, unix)))]
fn system() -> LocalTimeZone {
    LocalTimeZone::utc()
}
