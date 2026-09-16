use super::{LocalTimeZone, Timestamp};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Year,
    LongYear,
    Month,
    LongMonth,
    NumMonth(bool),
    Weekday(bool),
    Day(bool, bool),
    YearDay(bool),
    Hour,
    Hour12(bool),
    Minute(bool),
    Second(bool),
    AmPm(bool),
    Zone,
    Offset(bool, usize, bool),
    Fraction(bool, usize),
}

fn chunk(layout: &[u8]) -> (usize, Field, usize) {
    for index in 0..layout.len() {
        let rest = &layout[index..];
        let mut length = 0;
        let mut field = Field::None;
        match rest[0] {
            b'J' if rest.starts_with(b"January") => {
                length = 7;
                field = Field::LongMonth;
            }
            b'J' if rest.starts_with(b"Jan")
                && !rest.get(3).is_some_and(u8::is_ascii_lowercase) =>
            {
                length = 3;
                field = Field::Month;
            }
            b'M' if rest.starts_with(b"Monday") => {
                length = 6;
                field = Field::Weekday(true);
            }
            b'M' if rest.starts_with(b"Mon")
                && !rest.get(3).is_some_and(u8::is_ascii_lowercase) =>
            {
                length = 3;
                field = Field::Weekday(false);
            }
            b'M' if rest.starts_with(b"MST") => {
                length = 3;
                field = Field::Zone;
            }
            b'0' if rest.len() >= 2 && (b'1'..=b'6').contains(&rest[1]) => {
                length = 2;
                field = match rest[1] {
                    b'1' => Field::NumMonth(true),
                    b'2' => Field::Day(true, false),
                    b'3' => Field::Hour12(true),
                    b'4' => Field::Minute(true),
                    b'5' => Field::Second(true),
                    _ => Field::Year,
                };
            }
            b'0' if rest.starts_with(b"002") => {
                length = 3;
                field = Field::YearDay(true);
            }
            b'1' if rest.starts_with(b"15") => {
                length = 2;
                field = Field::Hour;
            }
            b'1' => {
                length = 1;
                field = Field::NumMonth(false);
            }
            b'2' if rest.starts_with(b"2006") => {
                length = 4;
                field = Field::LongYear;
            }
            b'2' => {
                length = 1;
                field = Field::Day(false, false);
            }
            b'_' if rest.starts_with(b"_2006") => return (index + 1, Field::LongYear, index + 5),
            b'_' if rest.starts_with(b"_2") => {
                length = 2;
                field = Field::Day(false, true);
            }
            b'_' if rest.starts_with(b"__2") => {
                length = 3;
                field = Field::YearDay(false);
            }
            b'3' => {
                length = 1;
                field = Field::Hour12(false);
            }
            b'4' => {
                length = 1;
                field = Field::Minute(false);
            }
            b'5' => {
                length = 1;
                field = Field::Second(false);
            }
            b'P' if rest.starts_with(b"PM") => {
                length = 2;
                field = Field::AmPm(false);
            }
            b'p' if rest.starts_with(b"pm") => {
                length = 2;
                field = Field::AmPm(true);
            }
            b'-' | b'Z' => {
                for pattern in [b"07:00:00".as_slice(), b"070000", b"07:00", b"0700", b"07"] {
                    if rest[1..].starts_with(pattern) {
                        length = pattern.len() + 1;
                        field = Field::Offset(rest[0] == b'Z', length, pattern.contains(&b':'));
                        break;
                    }
                }
            }
            b'.' | b',' if rest.get(1).is_some_and(|byte| matches!(byte, b'0' | b'9')) => {
                let digits = rest[1..]
                    .iter()
                    .take_while(|&&byte| byte == rest[1])
                    .count();
                if !rest.get(digits + 1).is_some_and(u8::is_ascii_digit) {
                    length = digits + 1;
                    field = Field::Fraction(rest[1] == b'0', digits);
                }
            }
            _ => {}
        }
        if length != 0 {
            return (index, field, index + length);
        }
    }
    (layout.len(), Field::None, layout.len())
}

fn quote(value: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::from("\"");
    for &byte in value {
        if !(b' '..128).contains(&byte) {
            write!(output, "\\x{byte:02x}").unwrap();
        } else {
            if matches!(byte, b'"' | b'\\') {
                output.push('\\');
            }
            output.push(byte as char);
        }
    }
    output.push('"');
    output
}

fn error(layout: &[u8], input: &[u8], field: &[u8], rest: &[u8], message: &str) -> String {
    if message.is_empty() {
        format!(
            "parsing time {} as {}: cannot parse {} as {}",
            quote(input),
            quote(layout),
            quote(rest),
            quote(field)
        )
    } else {
        format!("parsing time {}{message}", quote(input))
    }
}

fn number(value: &mut &[u8], width: usize, fixed: bool) -> Result<i32, ()> {
    let count = value
        .iter()
        .take(width)
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if count == 0 || fixed && count != width {
        return Err(());
    }
    let result = value[..count]
        .iter()
        .fold(0, |total, byte| total * 10 + (byte - b'0') as i32);
    *value = &value[count..];
    Ok(result)
}

fn integer(value: &[u8]) -> Result<i32, ()> {
    let text = std::str::from_utf8(value).map_err(|_| ())?;
    text.parse().map_err(|_| ())
}

fn lookup(value: &mut &[u8], names: &[&str], short: bool) -> Result<i32, ()> {
    for (index, name) in names.iter().enumerate() {
        let name = if short {
            &name.as_bytes()[..3]
        } else {
            name.as_bytes()
        };
        if value.len() >= name.len() && value[..name.len()].eq_ignore_ascii_case(name) {
            *value = &value[name.len()..];
            return Ok(index as i32);
        }
    }
    Err(())
}

fn skip<'a>(mut input: &'a [u8], mut prefix: &[u8]) -> Result<&'a [u8], &'a [u8]> {
    while !prefix.is_empty() {
        if prefix[0] == b' ' {
            if !input.is_empty() && input[0] != b' ' {
                return Err(input);
            }
            prefix = &prefix[prefix.iter().take_while(|&&byte| byte == b' ').count()..];
            input = &input[input.iter().take_while(|&&byte| byte == b' ').count()..];
        } else {
            if input.first() != prefix.first() {
                return Err(input);
            }
            prefix = &prefix[1..];
            input = &input[1..];
        }
    }
    Ok(input)
}

fn signed_offset(value: &[u8]) -> usize {
    if !value
        .first()
        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
    {
        return 0;
    }
    let count = value[1..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if integer(&value[1..1 + count]).is_ok_and(|value| value <= 23) {
        1 + count
    } else {
        0
    }
}

fn timezone(value: &[u8]) -> Result<usize, ()> {
    if value.len() < 3 {
        return Err(());
    }
    if value.starts_with(b"ChST") || value.starts_with(b"MeST") {
        return Ok(4);
    }
    if value.starts_with(b"GMT") {
        return Ok(3 + signed_offset(&value[3..]));
    }
    if matches!(value[0], b'+' | b'-') {
        return match signed_offset(value) {
            0 => Err(()),
            count => Ok(count),
        };
    }
    let count = value
        .iter()
        .take(6)
        .take_while(|byte| byte.is_ascii_uppercase())
        .count();
    match count {
        3 => Ok(3),
        4 if value[3] == b'T' || value.starts_with(b"WITA") => Ok(4),
        5 if value[4] == b'T' => Ok(5),
        _ => Err(()),
    }
}

fn nanoseconds(value: &[u8], count: usize) -> Result<u32, ()> {
    if !matches!(value[0], b'.' | b',') {
        return Err(());
    }
    let count = count.min(10);
    let fraction = integer(&value[1..count])?;
    if fraction < 0 {
        return Err(());
    }
    Ok(fraction as u32 * 10u32.pow((10 - count) as u32))
}

pub(crate) fn parse(
    layout: &str,
    source: &str,
    local: &LocalTimeZone,
) -> Result<Timestamp, String> {
    let original = layout.as_bytes();
    let input = source.as_bytes();
    let mut layout = original;
    let mut value = input;
    let (mut year, mut month, mut day, mut year_day) = (0, -1, -1, -1);
    let (mut hour, mut minute, mut second, mut nanos) = (0, 0, 0, 0);
    let (mut am_set, mut pm_set, mut utc) = (false, false, false);
    let mut offset = -1;
    let mut zone = String::new();
    loop {
        let (prefix_len, field, end) = chunk(layout);
        let field_text = &layout[prefix_len..end];
        value = skip(value, &layout[..prefix_len])
            .map_err(|rest| error(original, input, &layout[..prefix_len], rest, ""))?;
        if field == Field::None {
            if !value.is_empty() {
                return Err(error(
                    original,
                    input,
                    b"",
                    value,
                    &format!(": extra text: {}", quote(value)),
                ));
            }
            break;
        }
        layout = &layout[end..];
        let hold = value;
        let mut range_error = "";
        let parsed: Result<(), ()> = (|| {
            match field {
                Field::Year | Field::LongYear => {
                    let width = if field == Field::Year { 2 } else { 4 };
                    if value.len() < width || field == Field::LongYear && !value[0].is_ascii_digit()
                    {
                        return Err(());
                    }
                    let digits = &value[..width];
                    value = &value[width..];
                    year = integer(digits)?;
                    if field == Field::Year {
                        year += if year >= 69 { 1900 } else { 2000 };
                    }
                }
                Field::Month | Field::LongMonth => {
                    month = lookup(&mut value, &MONTHS, field == Field::Month)? + 1
                }
                Field::NumMonth(fixed) => {
                    month = number(&mut value, 2, fixed)?;
                    if !(1..=12).contains(&month) {
                        range_error = "month";
                    }
                }
                Field::Weekday(long) => {
                    lookup(&mut value, &DAYS, !long)?;
                }
                Field::Day(fixed, under) => {
                    if under && value.first() == Some(&b' ') {
                        value = &value[1..];
                    }
                    day = number(&mut value, 2, fixed)?;
                }
                Field::YearDay(fixed) => {
                    if !fixed {
                        for _ in 0..2 {
                            if value.first() == Some(&b' ') {
                                value = &value[1..];
                            }
                        }
                    }
                    year_day = number(&mut value, 3, fixed)?;
                }
                Field::Hour | Field::Hour12(_) => {
                    hour = number(&mut value, 2, field == Field::Hour12(true))?;
                    if hour >= if field == Field::Hour { 24 } else { 13 } {
                        range_error = "hour";
                    }
                }
                Field::Minute(fixed) => {
                    minute = number(&mut value, 2, fixed)?;
                    if minute >= 60 {
                        range_error = "minute";
                    }
                }
                Field::Second(fixed) => {
                    second = number(&mut value, 2, fixed)?;
                    if second >= 60 {
                        range_error = "second";
                    } else if value.len() >= 2
                        && matches!(value[0], b'.' | b',')
                        && value[1].is_ascii_digit()
                        && !matches!(chunk(layout).1, Field::Fraction(_, _))
                    {
                        let count = 1 + value[1..]
                            .iter()
                            .take_while(|byte| byte.is_ascii_digit())
                            .count();
                        nanos = nanoseconds(value, count)?;
                        value = &value[count..];
                    }
                }
                Field::AmPm(lower) => {
                    if value.len() < 2 {
                        return Err(());
                    }
                    let marker = &value[..2];
                    value = &value[2..];
                    if marker == if lower { b"pm" } else { b"PM" } {
                        pm_set = true;
                    } else if marker == if lower { b"am" } else { b"AM" } {
                        am_set = true;
                    } else {
                        return Err(());
                    }
                }
                Field::Offset(iso, length, colon) => {
                    if iso && value.first() == Some(&b'Z') {
                        utc = true;
                        value = &value[1..];
                    } else {
                        if value.len() < length
                            || colon && (value[3] != b':' || length == 9 && value[6] != b':')
                        {
                            return Err(());
                        }
                        let digits = &value[..length];
                        value = &value[length..];
                        let hours = number(&mut &digits[1..3], 2, true)?;
                        let minutes = if length == 3 {
                            0
                        } else {
                            let start = if colon { 4 } else { 3 };
                            number(&mut &digits[start..start + 2], 2, true)?
                        };
                        let seconds = if length < 7 {
                            0
                        } else {
                            let start = if colon { 7 } else { 5 };
                            number(&mut &digits[start..start + 2], 2, true)?
                        };
                        if hours > 24 {
                            range_error = "time zone offset hour";
                        }
                        if minutes > 60 {
                            range_error = "time zone offset minute";
                        }
                        if seconds > 60 {
                            range_error = "time zone offset second";
                        }
                        offset = (hours * 60 + minutes) * 60 + seconds;
                        match digits[0] {
                            b'+' => {}
                            b'-' => offset = -offset,
                            _ => return Err(()),
                        }
                    }
                }
                Field::Zone => {
                    if value.starts_with(b"UTC") {
                        utc = true;
                        value = &value[3..];
                    } else {
                        let count = timezone(value)?;
                        zone = String::from_utf8_lossy(&value[..count]).into_owned();
                        value = &value[count..];
                    }
                }
                Field::Fraction(fixed, digits) => {
                    if fixed {
                        if value.len() < digits + 1 {
                            return Err(());
                        }
                        nanos = nanoseconds(value, digits + 1)?;
                        value = &value[digits + 1..];
                    } else if value.len() >= 2
                        && matches!(value[0], b'.' | b',')
                        && value[1].is_ascii_digit()
                    {
                        let count = 1 + value[1..]
                            .iter()
                            .take_while(|byte| byte.is_ascii_digit())
                            .count();
                        nanos = nanoseconds(value, count)?;
                        value = &value[count..];
                    }
                }
                Field::None => unreachable!(),
            }
            Ok(())
        })();
        if !range_error.is_empty() {
            return Err(error(
                original,
                input,
                field_text,
                value,
                &format!(": {range_error} out of range"),
            ));
        }
        if parsed.is_err() {
            return Err(error(original, input, field_text, hold, ""));
        }
    }
    if pm_set && hour < 12 {
        hour += 12;
    } else if am_set && hour == 12 {
        hour = 0;
    }
    if year_day >= 0 {
        use chrono::Datelike;
        let date = chrono::NaiveDate::from_yo_opt(year, year_day as u32)
            .ok_or_else(|| error(original, input, b"", value, ": day-of-year out of range"))?;
        if month >= 0 && month != date.month() as i32 {
            return Err(error(
                original,
                input,
                b"",
                value,
                ": day-of-year does not match month",
            ));
        }
        if day >= 0 && day != date.day() as i32 {
            return Err(error(
                original,
                input,
                b"",
                value,
                ": day-of-year does not match day",
            ));
        }
        month = date.month() as i32;
        day = date.day() as i32;
    } else {
        if month < 0 {
            month = 1;
        }
        if day < 0 {
            day = 1;
        }
    }
    let date = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32)
        .ok_or_else(|| error(original, input, b"", value, ": day out of range"))?;
    let mut seconds = date
        .and_hms_opt(hour as u32, minute as u32, second as u32)
        .unwrap()
        .and_utc()
        .timestamp();
    if utc {
        offset = 0;
        zone = "UTC".to_owned();
    } else if offset != -1 {
        seconds -= offset as i64;
        let actual = local.lookup(seconds);
        if actual.offset == offset && (zone.is_empty() || actual.name == zone) {
            zone = actual.name;
        }
    } else if !zone.is_empty() {
        if let Some(actual) = local.lookup_name(&zone, seconds) {
            seconds -= actual as i64;
            let actual = local.lookup(seconds);
            offset = actual.offset;
            zone = actual.name;
        } else {
            offset = if zone.starts_with("GMT") && zone.len() > 3 {
                integer(&zone.as_bytes()[3..]).unwrap_or_default() * 3600
            } else {
                0
            };
        }
    } else {
        offset = 0;
        zone = "UTC".to_owned();
    }
    Ok(Timestamp {
        seconds,
        nanoseconds: nanos,
        zone,
        offset,
    })
}
