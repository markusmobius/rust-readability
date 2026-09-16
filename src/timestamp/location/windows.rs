use super::{LocalTimeZone, Zone};
use chrono::Datelike;
use std::{collections::BTreeMap, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::SYSTEMTIME,
    System::{
        Registry::RegLoadMUIStringW,
        Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION},
    },
};
use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(
        &value[..value
            .iter()
            .position(|&character| character == 0)
            .unwrap_or(value.len())],
    )
}

fn mui_string(key: &RegKey, name: &str) -> std::io::Result<String> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 1024];
    loop {
        let mut required = 0;
        let status = unsafe {
            RegLoadMUIStringW(
                key.raw_handle().cast(),
                name.as_ptr(),
                buffer.as_mut_ptr(),
                (buffer.len() * 2) as u32,
                &mut required,
                0,
                std::ptr::null(),
            )
        };
        if status == 0 {
            return Ok(utf16(&buffer));
        }
        if status != 234 || required as usize <= buffer.len() * 2 {
            return Err(std::io::Error::from_raw_os_error(status as i32));
        }
        buffer.resize((required as usize).div_ceil(2), 0);
    }
}

fn names(information: &TIME_ZONE_INFORMATION) -> (String, String) {
    static ABBREVIATIONS: OnceLock<BTreeMap<String, [String; 2]>> = OnceLock::new();
    let abbreviations = ABBREVIATIONS.get_or_init(|| {
        serde_json::from_str(include_str!("../windows-abbreviations.json")).unwrap()
    });
    let standard = utf16(&information.StandardName);
    let daylight = utf16(&information.DaylightName);
    if let Some([standard, daylight]) = abbreviations.get(&standard) {
        return (standard.clone(), daylight.clone());
    }
    if let Ok(zones) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Time Zones")
    {
        for name in zones.enum_keys().flatten() {
            let Ok(zone) = zones.open_subkey(&name) else {
                continue;
            };
            let strings = mui_string(&zone, "MUI_Std")
                .and_then(|standard| {
                    mui_string(&zone, "MUI_Dlt").map(|daylight| (standard, daylight))
                })
                .or_else(|_| {
                    zone.get_value::<String, _>("Std").and_then(|standard| {
                        zone.get_value::<String, _>("Dlt")
                            .map(|daylight| (standard, daylight))
                    })
                });
            if let Ok((zone_standard, zone_daylight)) = strings {
                if zone_standard == standard && (zone_daylight == daylight || daylight == standard)
                {
                    if let Some([standard, daylight]) = abbreviations.get(&name) {
                        return (standard.clone(), daylight.clone());
                    }
                    break;
                }
            }
        }
    }
    (
        standard.chars().filter(char::is_ascii_uppercase).collect(),
        daylight.chars().filter(char::is_ascii_uppercase).collect(),
    )
}

fn pseudo_unix(year: i32, rule: &SYSTEMTIME) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(year, rule.wMonth as u32, 1).unwrap();
    let first =
        1 + (rule.wDayOfWeek as i32 - date.weekday().num_days_from_sunday() as i32).rem_euclid(7);
    let mut day = first + (rule.wDay as i32 - 1).min(4) * 7;
    if chrono::NaiveDate::from_ymd_opt(year, rule.wMonth as u32, day as u32).is_none() {
        day -= 7;
    }
    date.and_hms_opt(rule.wHour as u32, rule.wMinute as u32, rule.wSecond as u32)
        .unwrap()
        .and_utc()
        .timestamp()
        + (day - 1) as i64 * 86400
}

pub(super) fn load() -> Option<LocalTimeZone> {
    let mut information = std::mem::MaybeUninit::<TIME_ZONE_INFORMATION>::uninit();
    let status = unsafe { GetTimeZoneInformation(information.as_mut_ptr()) };
    if status == u32::MAX {
        return None;
    }
    let information = unsafe { information.assume_init() };
    let (standard, daylight) = names(&information);
    if information.StandardDate.wMonth == 0 {
        return Some(LocalTimeZone::fixed(standard, -information.Bias * 60));
    }
    let zones = vec![
        Zone {
            name: standard,
            offset: -(information.Bias + information.StandardBias) * 60,
            daylight: false,
        },
        Zone {
            name: daylight,
            offset: -(information.Bias + information.DaylightBias) * 60,
            daylight: true,
        },
    ];
    let (first, first_zone, second, second_zone) =
        if information.StandardDate.wMonth > information.DaylightDate.wMonth {
            (&information.DaylightDate, 1, &information.StandardDate, 0)
        } else {
            (&information.StandardDate, 0, &information.DaylightDate, 1)
        };
    let year = chrono::Utc::now().year();
    let mut transitions = Vec::with_capacity(400);
    for year in year - 100..year + 100 {
        transitions.push((
            pseudo_unix(year, first) - zones[second_zone].offset as i64,
            first_zone,
        ));
        transitions.push((
            pseudo_unix(year, second) - zones[first_zone].offset as i64,
            second_zone,
        ));
    }
    Some(LocalTimeZone {
        zones,
        transitions,
        future: None,
    })
}
