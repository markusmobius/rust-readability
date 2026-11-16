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
        if name.is_empty() || name == "UTC" {
            return Ok(Self::utc());
        }
        Self::from_tzif(tzinfo_from_zip(include_bytes!("zoneinfo.zip"), name)?)
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

fn get2(data: &[u8], offset: usize) -> usize {
    data.get(offset..offset + 2)
        .map_or(0, |bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
}

fn get4(data: &[u8], offset: usize) -> usize {
    data.get(offset..offset + 4).map_or(0, |bytes| {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
    })
}

/// Port of Go's `time.loadTzinfoFromZip`: reads one stored (uncompressed)
/// entry from the central directory of Go's bundled `zoneinfo.zip`.
fn tzinfo_from_zip<'a>(zip: &'a [u8], name: &str) -> Result<&'a [u8], TimestampError> {
    const END_HEADER: usize = 0x0605_4b50;
    const CENTRAL_HEADER: usize = 0x0201_4b50;
    const LOCAL_HEADER: usize = 0x0403_4b50;
    const END_SIZE: usize = 22;
    const LOCAL_SIZE: usize = 30;
    let corrupt = || TimestampError("corrupt zip file zoneinfo.zip".into());

    let end = zip
        .len()
        .checked_sub(END_SIZE)
        .map(|start| &zip[start..])
        .filter(|end| get4(end, 0) == END_HEADER)
        .ok_or_else(corrupt)?;
    let count = get2(end, 10);
    let size = get4(end, 12);
    let offset = get4(end, 16);
    let mut directory = zip
        .get(offset..offset.saturating_add(size))
        .ok_or_else(corrupt)?;

    for _ in 0..count {
        if get4(directory, 0) != CENTRAL_HEADER {
            break;
        }
        let method = get2(directory, 10);
        let size = get4(directory, 24);
        let name_length = get2(directory, 28);
        let extra_length = get2(directory, 30);
        let comment_length = get2(directory, 32);
        let offset = get4(directory, 42);
        let entry_name = directory.get(46..46 + name_length).ok_or_else(corrupt)?;
        directory = directory
            .get(46 + name_length + extra_length + comment_length..)
            .ok_or_else(corrupt)?;
        if entry_name != name.as_bytes() {
            continue;
        }
        if method != 0 {
            return Err(TimestampError(format!(
                "unsupported compression for {name} in zoneinfo.zip"
            )));
        }

        let header = zip
            .get(offset..offset.saturating_add(LOCAL_SIZE + name_length))
            .filter(|header| {
                get4(header, 0) == LOCAL_HEADER
                    && get2(header, 8) == method
                    && get2(header, 26) == name_length
                    && &header[LOCAL_SIZE..] == name.as_bytes()
            })
            .ok_or_else(corrupt)?;
        let start = offset + LOCAL_SIZE + name_length + get2(header, 28);
        return zip
            .get(start..start.saturating_add(size))
            .ok_or_else(corrupt);
    }
    Err(TimestampError(format!("unknown time zone {name}")))
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

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_zone_lookup() {
        assert!(super::LocalTimeZone::from_name("America/New_York").is_ok());
        for missing in ["America", "America/", "Nowhere/City", "america/new_york"] {
            assert_eq!(
                super::LocalTimeZone::from_name(missing).unwrap_err().0,
                format!("unknown time zone {missing}")
            );
        }
    }
}
