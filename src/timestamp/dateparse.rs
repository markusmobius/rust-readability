use super::{parse_layout, LocalTimeZone, Timestamp};
use regex::Regex;
use std::sync::{LazyLock, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum DateState {
    #[default]
    Start,
    Digit,
    DigitSt,
    YearDash,
    YearDashAlpha,
    YearDashDash,
    YearDashDashWs,
    YearDashDashT,
    YearDashDashOffset,
    DigitDash,
    DigitDashAlpha,
    DigitDashAlphaDash,
    DigitDashDigit,
    DigitDashDigitDash,
    DigitDot,
    DigitDotDot,
    DigitDotDotWs,
    DigitDotDotT,
    DigitDotDotOffset,
    DigitSlash,
    DigitYearSlash,
    DigitSlashAlpha,
    DigitSlashAlphaSlash,
    DigitColon,
    DigitChineseYear,
    DigitChineseYearWs,
    DigitWs,
    DigitWsMoYear,
    Alpha,
    AlphaWs,
    AlphaWsDigit,
    AlphaWsDigitMore,
    AlphaWsDigitMoreWs,
    AlphaWsDigitMoreWsYear,
    AlphaWsDigitYearMaybe,
    VariousDaySuffix,
    AlphaFullMonthWs,
    AlphaFullMonthWsDayWs,
    AlphaWsAlpha,
    AlphaPeriodWsDigit,
    AlphaSlash,
    AlphaSlashDigit,
    AlphaSlashDigitSlash,
    YearWs,
    YearWsMonthWs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeState {
    Start,
    Ws,
    WsAlpha,
    WsAlphaRParen,
    WsAlphaWs,
    WsAlphaWsYear,
    WsAlphaZoneOffset,
    WsAlphaZoneOffsetWs,
    WsAlphaZoneOffsetWsYear,
    WsOffsetWsTZDescInParen,
    WsAMPMMaybe,
    WsAMPM,
    WsOffset,
    WsOffsetWs,
    WsOffsetWsYear,
    WsOffsetWsAlphaZone,
    WsOffsetWsAlphaZoneWs,
    WsYear,
    Period,
    PeriodAMPM,
    Z,
}

#[derive(Default)]
struct Scanner {
    date: DateState,
    has_time: bool,
    force_utc: bool,
    format: Vec<u8>,
    format_set_len: usize,
    source: Vec<u8>,
    original: Vec<u8>,
    full_month: Vec<u8>,
    parsed_ampm: bool,
    skip: usize,
    link: usize,
    extra: usize,
    part1_len: usize,
    yeari: usize,
    yearlen: usize,
    moi: usize,
    molen: usize,
    dayi: usize,
    daylen: usize,
    houri: usize,
    hourlen: usize,
    mini: usize,
    minlen: usize,
    seci: usize,
    seclen: usize,
    msi: usize,
    mslen: usize,
    offseti: usize,
    tzi: usize,
    tzlen: usize,
}

fn rune(bytes: &[u8]) -> (char, usize) {
    if bytes[0] < 128 {
        return (bytes[0] as char, 1);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            let character = text.chars().next().unwrap();
            (character, character.len_utf8())
        }
        Err(error) if error.valid_up_to() > 0 => rune(&bytes[..error.valid_up_to()]),
        Err(_) => ('\u{fffd}', 1),
    }
}

fn letter(character: char) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    character.is_ascii_alphabetic()
        || !character.is_ascii()
            && PATTERN
                .get_or_init(|| Regex::new(r"^\p{L}$").unwrap())
                .is_match(character.encode_utf8(&mut [0; 4]))
}

fn digit(character: char) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    character.is_ascii_digit()
        || !character.is_ascii()
            && PATTERN
                .get_or_init(|| Regex::new(r"^\p{Nd}$").unwrap())
                .is_match(character.encode_utf8(&mut [0; 4]))
}

fn quote(mut value: &[u8]) -> String {
    use std::fmt::Write;
    static PRINTABLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[\p{L}\p{M}\p{N}\p{P}\p{S}]$").unwrap());
    let mut result = String::from("\"");
    while !value.is_empty() {
        let (character, width) = rune(value);
        if width == 1 && value[0] >= 128 {
            write!(result, "\\x{:02x}", value[0]).unwrap();
        } else {
            match character {
                '\x07' => result.push_str("\\a"),
                '\x08' => result.push_str("\\b"),
                '\x0c' => result.push_str("\\f"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                '\x0b' => result.push_str("\\v"),
                '"' => result.push_str("\\\""),
                '\\' => result.push_str("\\\\"),
                character
                    if (' '..='~').contains(&character)
                        || PRINTABLE.is_match(character.encode_utf8(&mut [0; 4])) =>
                {
                    result.push(character)
                }
                character if character < '\u{100}' => {
                    write!(result, "\\x{:02x}", character as u32).unwrap();
                }
                character if character < '\u{10000}' => {
                    write!(result, "\\u{:04x}", character as u32).unwrap();
                }
                character => {
                    write!(result, "\\U{:08x}", character as u32).unwrap();
                }
            }
        }
        value = &value[width..];
    }
    result.push('"');
    result
}

fn full_month(value: &[u8]) -> bool {
    [
        b"january".as_slice(),
        b"february",
        b"march",
        b"april",
        b"may",
        b"june",
        b"july",
        b"august",
        b"september",
        b"october",
        b"november",
        b"december",
    ]
    .contains(&value)
}

fn weekday(value: &[u8]) -> bool {
    [
        b"mon".as_slice(),
        b"tue",
        b"wed",
        b"thu",
        b"fri",
        b"sat",
        b"sun",
        b"monday",
        b"tuesday",
        b"wednesday",
        b"thursday",
        b"friday",
        b"saturday",
        b"sunday",
    ]
    .contains(&value)
}

fn proper_end(
    value: &[u8],
    start: usize,
    end: usize,
    numeric: bool,
    alpha: bool,
    other: bool,
) -> usize {
    for (index, &byte) in value.iter().enumerate().take(end).skip(start) {
        if !(if byte.is_ascii_digit() {
            numeric
        } else if byte.is_ascii_alphabetic() {
            alpha
        } else {
            other
        }) {
            return index;
        }
    }
    end
}

impl Scanner {
    fn restart(&mut self, start: usize, end: usize) {
        let mut source = self.source[self.skip..start].to_vec();
        source.extend_from_slice(&self.source[end..]);
        *self = Self {
            original: source.clone(),
            format: source.clone(),
            source,
            ..Self::default()
        };
    }
    fn unknown(&self) -> String {
        format!("could not find format for {}", quote(&self.original))
    }
    fn tail(&self, start: usize) -> String {
        format!(
            "unexpected content after date/time:  {}",
            quote(&self.source[start..])
        )
    }
    fn set(&mut self, start: usize, value: &[u8]) {
        if start + value.len() > self.format.len() {
            return;
        }
        self.format[start..start + value.len()].copy_from_slice(value);
        self.format_set_len = self.format_set_len.max(start + value.len());
    }
    fn entire(&mut self, value: &[u8]) {
        self.format = value.to_vec();
        self.format_set_len = value.len();
    }
    fn month(&mut self) -> Result<(), String> {
        match self.molen {
            1 => self.set(self.moi, b"1"),
            2 => self.set(self.moi, b"01"),
            _ => return Err(self.unknown()),
        }
        Ok(())
    }
    fn day(&mut self) -> Result<(), String> {
        match self.daylen {
            1 => self.set(self.dayi, b"2"),
            2 => self.set(self.dayi, b"02"),
            _ => return Err(self.unknown()),
        }
        Ok(())
    }
    fn year(&mut self) -> Result<(), String> {
        match self.yearlen {
            2 => self.set(self.yeari, b"06"),
            4 => self.set(self.yeari, b"2006"),
            _ => return Err(self.unknown()),
        }
        Ok(())
    }
    fn tz_offset(&mut self, end: usize) -> Result<(), String> {
        match end - self.offseti {
            3 => self.set(self.offseti, b"-07"),
            5 => self.set(self.offseti, b"-0700"),
            6 => self.set(self.offseti, b"-07:00"),
            _ => {
                return Err(format!(
                    "TZ offset not recognized {} near {} (must be 2 or 4 digits optional colon)",
                    quote(&self.original),
                    quote(&self.source[self.offseti..end])
                ))
            }
        }
        Ok(())
    }
    fn tz_name(&mut self) -> Result<(), String> {
        match self.tzlen {
            3 => self.set(self.tzi, b"MST"),
            4 => self.set(self.tzi, b"MST "),
            _ => {
                return Err(format!(
                    "timezone not recognized {} near {} (must be 3 or 4 characters)",
                    quote(&self.original),
                    quote(&self.source[self.tzi..self.tzi + self.tzlen])
                ))
            }
        }
        Ok(())
    }
    fn coalesce_date(&mut self, end: usize) -> Result<(), String> {
        if self.yeari > 0 {
            if self.yearlen == 0 {
                self.yearlen =
                    proper_end(&self.source, self.yeari, end, true, false, false) - self.yeari;
            }
            self.year()?;
        }
        if self.moi > 0 && self.molen == 0 {
            self.molen = proper_end(&self.source, self.moi, end, true, true, false) - self.moi;
            let _ = self.month();
        }
        if self.dayi > 0 && self.daylen == 0 {
            self.daylen = proper_end(&self.source, self.dayi, end, true, false, false) - self.dayi;
            self.day()?;
        }
        Ok(())
    }
    fn coalesce_time(&mut self, end: usize) -> Result<(), String> {
        if self.houri > 0 {
            match self.hourlen {
                2 => self.set(self.houri, b"15"),
                1 => self.set(self.houri, b"3"),
                _ => return Err(self.unknown()),
            }
        }
        if self.mini > 0 {
            if self.minlen == 0 {
                self.minlen = end - self.mini;
            }
            match self.minlen {
                2 => self.set(self.mini, b"04"),
                1 => self.set(self.mini, b"4"),
                _ => return Err(self.unknown()),
            }
        }
        if self.seci > 0 {
            if self.seclen == 0 {
                self.seclen = end - self.seci;
            }
            match self.seclen {
                2 => self.set(self.seci, b"05"),
                1 => self.set(self.seci, b"5"),
                _ => return Err(self.unknown()),
            }
        }
        if self.msi > 0 {
            self.format[self.msi..self.msi + self.mslen].fill(b'0');
            self.format_set_len = self.format_set_len.max(self.msi + self.mslen);
        }
        Ok(())
    }
    fn trim_extra(&mut self) {
        if self.extra > 0 && self.format.len() > self.extra {
            self.format.truncate(self.extra);
            self.format_set_len = self.format_set_len.min(self.format.len());
            self.source.truncate(self.extra);
        }
    }
    fn named_month(&mut self, end: usize) -> Result<(), String> {
        if self.molen == 3 {
            self.set(self.moi, b"Jan");
        } else {
            let name = self.source[self.moi..self.moi + self.molen].to_ascii_lowercase();
            if end <= 3 || !full_month(&name) {
                return Err(self.unknown());
            }
            self.full_month = name;
        }
        Ok(())
    }
    fn dashed_year(&mut self, length: usize) -> Result<(), String> {
        if !matches!(length, 2 | 4) {
            return Err(self.unknown());
        }
        self.yearlen = length;
        self.year()?;
        self.dayi = self.skip;
        self.daylen = self.part1_len;
        self.day()
    }
    fn scan_date(&mut self) -> Result<usize, String> {
        use DateState::*;
        let mut index = 0;
        while index < self.source.len() {
            let (character, width) = rune(&self.source[index..]);
            index += width - 1;
            let adjusted = index - self.skip;
            match self.date {
                Start => match character {
                    '0'..='9' => self.date = Digit,
                    'a'..='z' | 'A'..='Z' => self.date = Alpha,
                    ' ' => self.skip = index + 1,
                    _ => return Err(self.unknown()),
                },
                Digit => {
                    match character {
                        '-' | '\u{2212}' => {
                            if adjusted == 4 {
                                self.date = YearDash;
                                self.yeari = self.skip;
                                self.yearlen = index - self.skip;
                                self.moi = index + 1;
                                self.set(self.skip, b"2006");
                            } else {
                                self.date = DigitDash;
                            }
                        }
                        '/' | ':' => {
                            self.date = if character == '/' {
                                DigitSlash
                            } else {
                                DigitColon
                            };
                            if adjusted == 4 {
                                self.yeari = self.skip;
                                self.yearlen = index - self.skip;
                                self.moi = index + 1;
                                self.year()?;
                                if character == '/' {
                                    self.date = DigitYearSlash;
                                }
                            } else if character == '/'
                                && index + 2 < self.source.len()
                                && letter(self.source[index + 1] as char)
                            {
                                self.date = DigitSlashAlpha;
                                self.moi = index + 1;
                                self.daylen = 2;
                                self.dayi = self.skip;
                                self.day()?;
                                index += 1;
                                continue;
                            } else {
                                if self.molen != 0 {
                                    return Err(self.unknown());
                                }
                                self.moi = self.skip;
                                self.molen = index - self.skip;
                                self.month()?;
                                self.dayi = index + 1;
                            }
                        }
                        '.' => {
                            self.date = DigitDot;
                            if adjusted == 4 {
                                self.yeari = self.skip;
                                self.yearlen = index - self.skip;
                                self.moi = index + 1;
                                self.year()?;
                            } else if adjusted <= 2 {
                                if self.molen != 0 {
                                    return Err(self.unknown());
                                }
                                self.moi = self.skip;
                                self.molen = index - self.skip;
                                self.month()?;
                                self.dayi = index + 1;
                            }
                        }
                        ' ' => match adjusted {
                            4 => {
                                self.yeari = self.skip;
                                self.yearlen = index - self.skip;
                                self.moi = index + 1;
                                self.year()?;
                                self.date = YearWs;
                            }
                            6 => self.date = DigitSt,
                            _ => {
                                self.date = DigitWs;
                                self.dayi = self.skip;
                                self.daylen = index - self.skip;
                            }
                        },
                        '\u{5e74}' => {
                            self.date = DigitChineseYear;
                            self.yeari = self.skip;
                            self.yearlen = index - 2 - self.skip;
                            self.moi = index + 1;
                            self.year()?;
                        }
                        's' | 'S' | 'r' | 'R' | 't' | 'T' | 'n' | 'N' => {
                            self.date = VariousDaySuffix;
                            index -= 1;
                        }
                        _ => {
                            if !digit(character) {
                                return Err(self.unknown());
                            }
                            index += 1;
                            continue;
                        }
                    }
                    self.part1_len = index - self.skip;
                }
                DigitSt => {
                    self.set(self.skip, b"060102");
                    index -= 1;
                    self.has_time = true;
                    break;
                }
                YearDash => match character {
                    '-' | '\u{2212}' => {
                        self.molen = index - self.moi;
                        self.dayi = index + 1;
                        self.date = YearDashDash;
                        self.month()?;
                    }
                    _ => {
                        if letter(character) {
                            self.date = YearDashAlpha;
                        } else if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                YearDashDash => match character {
                    '+' | '-' => {
                        self.offseti = index;
                        self.daylen = index - self.dayi;
                        self.date = YearDashDashOffset;
                        self.day()?;
                    }
                    ' ' | 'T' => {
                        self.daylen = index - self.dayi;
                        self.date = if character == ' ' {
                            YearDashDashWs
                        } else {
                            YearDashDashT
                        };
                        self.has_time = true;
                        self.day()?;
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                YearDashDashOffset | DigitDotDotOffset => {
                    if character != ':' && !digit(character) {
                        return Err(self.unknown());
                    }
                }
                YearDashAlpha | DigitDashAlpha => match character {
                    '-' | '\u{2212}' => {
                        self.molen = index - self.moi;
                        self.named_month(index)?;
                        if self.date == YearDashAlpha {
                            self.dayi = index + 1;
                            self.date = YearDashDash;
                        } else {
                            self.yeari = index + 1;
                            self.date = DigitDashAlphaDash;
                        }
                    }
                    _ => {
                        if !letter(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitDash => {
                    self.date = if letter(character) {
                        DigitDashAlpha
                    } else if digit(character) {
                        DigitDashDigit
                    } else {
                        return Err(self.unknown());
                    };
                    self.moi = index;
                }
                DigitDashDigit => match character {
                    '-' | '\u{2212}' => {
                        self.molen = index - self.moi;
                        if self.molen != 2 {
                            return Err(self.unknown());
                        }
                        self.set(self.moi, b"01");
                        self.yeari = index + 1;
                        self.date = DigitDashDigitDash;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitDashAlphaDash | DigitDashDigitDash => match character {
                    ' ' | ':' => {
                        let mut double_colon = false;
                        if character == ':' {
                            self.link += 1;
                            if self.link == 2 {
                                double_colon = index + 1 < self.source.len()
                                    && digit(rune(&self.source[index + 1..]).0);
                                if !double_colon {
                                    return Err(self.unknown());
                                }
                            }
                        } else if self.link > 0 {
                            return Err(self.unknown());
                        }
                        if character == ' ' || double_colon {
                            self.dashed_year(
                                index - (self.moi + self.molen + if double_colon { 2 } else { 1 }),
                            )?;
                            self.has_time = true;
                            break;
                        }
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitYearSlash => match character {
                    ' ' | ':' => {
                        self.has_time = true;
                        if self.daylen == 0 {
                            self.daylen = index - self.dayi;
                            self.day()?;
                        }
                        break;
                    }
                    '/' => {
                        if self.molen == 0 {
                            self.molen = index - self.moi;
                            self.month()?;
                            self.dayi = index + 1;
                        }
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitSlashAlpha => match character {
                    '/' => {
                        if self.molen != 0 {
                            return Err(self.unknown());
                        }
                        self.molen = index - self.moi;
                        self.named_month(index)?;
                        self.yeari = index + 1;
                        self.date = DigitSlashAlphaSlash;
                    }
                    _ => {
                        if !letter(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitSlashAlphaSlash => match character {
                    ' ' | ':' => {
                        self.has_time = true;
                        if self.yearlen == 0 {
                            self.yearlen = index - self.yeari;
                            self.year()?;
                        }
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitSlash => match character {
                    '/' => {
                        if self.daylen == 0 {
                            self.daylen = index - self.dayi;
                            self.day()?;
                            self.yeari = index + 1;
                        }
                    }
                    ' ' | ',' => {
                        self.has_time = true;
                        if self.yearlen == 0 {
                            self.yearlen = index - self.yeari;
                            if character == ',' {
                                index += 1;
                            }
                            self.year()?;
                        }
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitColon => match character {
                    ' ' => {
                        self.has_time = true;
                        if self.yearlen == 0 {
                            self.yearlen = index - self.yeari;
                            self.year()?;
                        } else if self.daylen == 0 {
                            self.daylen = index - self.dayi;
                            self.day()?;
                        } else if self.molen == 0 {
                            self.molen = index - self.moi;
                            self.month()?;
                        }
                        break;
                    }
                    ':' => {
                        if self.yearlen > 0 {
                            if self.molen == 0 {
                                self.molen = index - self.moi;
                                self.month()?;
                                self.dayi = index + 1;
                            }
                        } else if self.daylen == 0 {
                            self.daylen = index - self.dayi;
                            self.day()?;
                            self.yeari = index + 1;
                        }
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitChineseYear => match character {
                    '\u{6708}' => {
                        self.molen = index - self.moi - 2;
                        self.dayi = index + 1;
                        self.month()?;
                    }
                    '\u{65e5}' => {
                        self.daylen = index - self.dayi - 2;
                        self.day()?;
                    }
                    ' ' => {
                        if self.daylen == 0 {
                            return Err(self.unknown());
                        }
                        self.date = DigitChineseYearWs;
                        self.has_time = true;
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitDot => {
                    if character == '.' {
                        if self.moi == 0 {
                            self.daylen = index - self.dayi;
                            self.yeari = index + 1;
                            self.day()?;
                        } else if self.dayi == 0 && self.yearlen == 0 {
                            self.molen = index - self.moi;
                            self.yeari = index + 1;
                            self.month()?;
                        } else {
                            self.molen = index - self.moi;
                            self.dayi = index + 1;
                            self.month()?;
                        }
                        self.date = DigitDotDot;
                    } else if !digit(character) {
                        return Err(self.unknown());
                    }
                }
                DigitDotDot => match character {
                    '+' | '-' => {
                        self.offseti = index;
                        self.daylen = index - self.dayi;
                        self.date = DigitDotDotOffset;
                        self.day()?;
                    }
                    ' ' => {
                        if self.daylen == 0 && self.molen > 0 && self.yearlen > 0 {
                            self.daylen = index - self.dayi;
                            self.day()?;
                        } else if self.molen == 0 && self.daylen > 0 && self.yearlen > 0 {
                            self.molen = index - self.moi;
                            self.month()?;
                        } else if self.yearlen == 0 && self.daylen > 0 && self.molen > 0 {
                            self.yearlen = index - self.yeari;
                            self.year()?;
                        } else {
                            return Err(self.unknown());
                        }
                        self.date = DigitDotDotWs;
                        self.has_time = true;
                        break;
                    }
                    'T' => {
                        self.daylen = index - self.dayi;
                        self.date = DigitDotDotT;
                        self.has_time = true;
                        self.day()?;
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                DigitWs => {
                    if character == ' ' {
                        self.yeari = index + 1;
                        self.dayi = self.skip;
                        self.daylen = self.part1_len;
                        self.day()?;
                        self.has_time = true;
                        self.moi = self.dayi + self.daylen + 1;
                        self.molen = index - self.moi;
                        if adjusted > self.daylen + 4 {
                            let name = self.source[self.moi..index].to_ascii_lowercase();
                            if !full_month(&name) {
                                return Err(self.unknown());
                            }
                            self.full_month = name;
                        } else {
                            self.set(self.moi, b"Jan");
                        }
                        self.date = DigitWsMoYear;
                    } else if !letter(character) {
                        return Err(self.unknown());
                    }
                }
                DigitWsMoYear => match character {
                    ',' | ' ' => {
                        self.yearlen = index - self.yeari;
                        self.year()?;
                        if character == ',' {
                            index += 1;
                        }
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                YearWs => {
                    if character == ' ' {
                        self.molen = index - self.moi;
                        self.named_month(index)?;
                        self.dayi = index + 1;
                        self.date = YearWsMonthWs;
                    } else if !letter(character) {
                        return Err(self.unknown());
                    }
                }
                YearWsMonthWs => match character {
                    ',' | ' ' => {
                        self.daylen = index - self.dayi;
                        let _ = self.day();
                        if character == ',' {
                            index += 1;
                        }
                        self.has_time = true;
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                Alpha => match character {
                    ' ' => {
                        let name = self.source[self.skip..index].to_ascii_lowercase();
                        if weekday(&name) {
                            self.skip = index + 1;
                            self.date = Start;
                        } else if adjusted > 3 {
                            if !full_month(&name) {
                                return Err(self.unknown());
                            }
                            self.moi = self.skip;
                            self.molen = index - self.skip;
                            self.full_month = name;
                            self.date = AlphaFullMonthWs;
                            self.dayi = index + 1;
                        } else if adjusted == 3 {
                            self.date = AlphaWs;
                        } else {
                            return Err(self.unknown());
                        }
                    }
                    ',' => {
                        if adjusted >= 3 && self.source.get(index + 1) == Some(&b' ') {
                            if !weekday(&self.source[self.skip..index].to_ascii_lowercase()) {
                                return Err(self.unknown());
                            }
                            self.date = Start;
                            self.skip = index + 2;
                            index += 1;
                        }
                    }
                    '.' => {
                        self.date = AlphaPeriodWsDigit;
                        match adjusted {
                            3 => {
                                self.moi = self.skip;
                                self.molen = index - self.skip;
                                self.set(self.skip, b"Jan");
                            }
                            4 => {
                                self.restart(index - 1, index);
                                index = 0;
                                continue;
                            }
                            _ => return Err(self.unknown()),
                        }
                    }
                    '/' => {
                        self.moi = self.skip;
                        self.molen = index - self.moi;
                        if adjusted == 3 {
                            self.set(self.moi, b"Jan");
                        } else {
                            let name = self.source[self.skip..index].to_ascii_lowercase();
                            if adjusted <= 3 || !full_month(&name) {
                                return Err(self.unknown());
                            }
                            self.full_month = name;
                        }
                        self.date = AlphaSlash;
                    }
                    _ => {
                        if !letter(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                AlphaWs => {
                    if letter(character) {
                        if adjusted < 3
                            || index + 3 >= self.source.len()
                            || !weekday(&self.source[self.skip..index].to_ascii_lowercase())
                        {
                            return Err(self.unknown());
                        }
                        self.skip = index;
                        self.date = AlphaWsAlpha;
                        self.set(index, b"Jan");
                    } else if digit(character) {
                        self.set(self.skip, b"Jan");
                        self.date = AlphaWsDigit;
                        self.dayi = index;
                    } else if character != ' ' {
                        return Err(self.unknown());
                    }
                }
                AlphaWsDigit => {
                    if character == ',' {
                        self.daylen = index - self.dayi;
                        self.day()?;
                        self.date = AlphaWsDigitMore;
                    } else if character == ' ' {
                        self.daylen = index - self.dayi;
                        self.day()?;
                        self.yeari = index + 1;
                        self.date = AlphaWsDigitYearMaybe;
                        self.has_time = true;
                    } else if letter(character) {
                        self.date = VariousDaySuffix;
                        index -= 1;
                    } else if !digit(character) {
                        return Err(self.unknown());
                    }
                }
                AlphaWsDigitYearMaybe => {
                    if character == ':' {
                        self.yeari = 0;
                        index -= 3;
                        self.date = AlphaWsDigit;
                        break;
                    } else if character == ' ' {
                        self.yearlen = index - self.yeari;
                        self.year()?;
                        break;
                    } else if !digit(character) {
                        return Err(self.unknown());
                    }
                }
                AlphaWsDigitMore => {
                    if character != ' ' {
                        return Err(self.unknown());
                    }
                    self.yeari = index + 1;
                    self.date = AlphaWsDigitMoreWs;
                }
                AlphaWsDigitMoreWs => match character {
                    '\'' => self.yeari = index + 1,
                    ' ' | ',' => {
                        self.date = AlphaWsDigitMoreWsYear;
                        self.yearlen = index - self.yeari;
                        self.year()?;
                        self.has_time = true;
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                VariousDaySuffix => {
                    let expected = match character.to_ascii_lowercase() {
                        't' => b'h',
                        'n' | 'r' => b'd',
                        's' => b't',
                        _ => return Err(self.unknown()),
                    };
                    if self.source.len() <= index + 2
                        || self.source[index + 1].to_ascii_lowercase() != expected
                    {
                        return Err(self.unknown());
                    }
                    self.restart(index, index + 2);
                    index = 0;
                    continue;
                }
                AlphaFullMonthWs => {
                    if character == ',' {
                        if self.source.get(index + 1) != Some(&b' ') {
                            return Err(self.unknown());
                        }
                        self.daylen = index - self.dayi;
                        self.day()?;
                        self.yeari = index + 2;
                        self.date = AlphaFullMonthWsDayWs;
                        index += 1;
                    } else if character == ' ' {
                        self.daylen = index - self.dayi;
                        self.day()?;
                        self.yeari = index + 1;
                        self.date = AlphaFullMonthWsDayWs;
                    } else if digit(character) {
                    } else if letter(character) {
                        self.daylen = index - self.dayi;
                        self.day()?;
                        self.date = VariousDaySuffix;
                        index -= 1;
                    } else {
                        return Err(self.unknown());
                    }
                }
                AlphaFullMonthWsDayWs => match character {
                    ',' | ' ' => {
                        self.yearlen = index - self.yeari;
                        self.year()?;
                        self.has_time = true;
                        if character == ',' {
                            index += 1;
                        }
                        break;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.unknown());
                        }
                    }
                },
                AlphaPeriodWsDigit | AlphaSlash => {
                    if character == ' ' {
                    } else if digit(character) {
                        self.date = if self.date == AlphaSlash {
                            AlphaSlashDigit
                        } else {
                            AlphaWsDigit
                        };
                        self.dayi = index;
                    } else {
                        return Err(self.unknown());
                    }
                }
                AlphaSlashDigit => {
                    if character == '/' {
                        self.yeari = index + 1;
                        self.daylen = index - self.dayi;
                        self.day()?;
                        self.date = AlphaSlashDigitSlash;
                    } else if !digit(character) {
                        return Err(self.unknown());
                    }
                }
                AlphaSlashDigitSlash => {
                    if digit(character) {
                    } else if character == ' ' {
                        self.has_time = true;
                        break;
                    } else {
                        return Err(self.unknown());
                    }
                }
                _ => return Err(self.unknown()),
            }
            index += 1;
        }
        self.coalesce_date(index)?;
        Ok(index)
    }
    fn scan_time(&mut self, mut index: usize) -> Result<usize, String> {
        use TimeState::*;
        let mut state = Start;
        if index < self.source.len() {
            index += 1;
        }
        while self.source.get(index) == Some(&b' ') {
            index += 1;
        }
        while index < self.source.len() {
            let character = self.source[index] as char;
            match state {
                Start => {
                    if self.houri == 0 {
                        self.houri = index;
                    }
                    match character {
                        '-' | '+' => {
                            self.offseti = index;
                            state = WsOffset;
                            if self.seci == 0 {
                                self.minlen = index - self.mini;
                            } else if self.seclen == 0 {
                                self.seclen = index - self.seci;
                            } else if self.msi > 0 && self.mslen == 0 {
                                self.mslen = index - self.msi;
                            } else if !self.parsed_ampm {
                                return Err(self.unknown());
                            }
                        }
                        '.' | ',' => {
                            state = Period;
                            self.seclen = index - self.seci;
                            self.msi = index + 1;
                        }
                        'Z' => {
                            state = Z;
                            self.force_utc = true;
                            if self.seci == 0 {
                                self.minlen = index - self.mini;
                            } else {
                                self.seclen = index - self.seci;
                            }
                            self.format_set_len = self.format_set_len.max(index + 1);
                        }
                        'a' | 'A' | 'p' | 'P' => {
                            if matches!(character, 'a' | 'A')
                                && self
                                    .source
                                    .get(index + 1)
                                    .is_some_and(|byte| matches!(byte, b't' | b'T'))
                            {
                                index += 1;
                                if self.source.get(index + 1) != Some(&b' ') {
                                    return Err(self.unknown());
                                }
                                index += 1;
                                self.houri = 0;
                            } else {
                                let lower = matches!(character, 'a' | 'p');
                                let word = index + 2 == self.source.len()
                                    || self
                                        .source
                                        .get(index + 2)
                                        .is_some_and(|byte| matches!(byte, b' ' | b'+' | b'-'));
                                if self.source.get(index + 1)
                                    != Some(&if lower { b'm' } else { b'M' })
                                    || !word
                                    || self.parsed_ampm
                                {
                                    return Err(self.tail(index));
                                }
                                self.coalesce_time(index)?;
                                self.set(index, if lower { b"pm" } else { b"PM" });
                                self.parsed_ampm = true;
                                index += 1;
                            }
                        }
                        ' ' => {
                            self.coalesce_time(index)?;
                            state = Ws;
                        }
                        ':' => {
                            if self.mini == 0 {
                                self.mini = index + 1;
                                self.hourlen = index - self.houri;
                            } else if self.seci == 0 {
                                self.seci = index + 1;
                                self.minlen = index - self.mini;
                            } else {
                                self.seclen = index - self.seci;
                                if self.seclen != 2 {
                                    return Err(self.unknown());
                                }
                                self.set(self.seci, b"05");
                                self.msi = index + 1;
                                self.set(index, b".");
                                self.source[index] = b'.';
                                state = Period;
                            }
                        }
                        _ => {}
                    }
                }
                Ws => match character {
                    'a' | 'p' | 'A' | 'P' => {
                        self.tzi = index;
                        state = WsAMPMMaybe;
                    }
                    '+' | '-' => {
                        self.offseti = index;
                        state = WsOffset;
                    }
                    _ => {
                        if letter(character) {
                            self.tzi = index;
                            state = WsAlpha;
                        } else if digit(character) {
                            if self.yeari != 0 {
                                return Err(self.unknown());
                            }
                            state = WsYear;
                            self.yeari = index;
                        } else if character != '(' {
                            return Err(self.unknown());
                        }
                    }
                },
                WsYear => match character {
                    ' ' => {
                        if self.yearlen == 0 {
                            self.yearlen = index - self.yeari;
                            self.year()?;
                        }
                    }
                    '+' | '-' => {
                        if self.yearlen == 0 {
                            return Err(self.unknown());
                        }
                        self.offseti = index;
                        state = WsOffset;
                    }
                    _ => {
                        if !digit(character) || self.yearlen > 0 {
                            return Err(self.unknown());
                        }
                    }
                },
                WsAlpha => match character {
                    '+' | '-' => {
                        let name = self.source[self.tzi..index].to_ascii_lowercase();
                        if [b"gmt".as_slice(), b"utc", b"tz"].contains(&name.as_slice()) {
                            self.tzi = 0;
                            self.tzlen = 0;
                        } else {
                            self.tzlen = index - self.tzi;
                        }
                        if self.tzlen > 0 {
                            self.tz_name()?;
                        }
                        state = WsAlphaZoneOffset;
                        self.offseti = index;
                    }
                    ' ' | ')' => {
                        self.tzlen = index - self.tzi;
                        self.tz_name()?;
                        if character == ' ' {
                            state = WsAlphaWs;
                        } else {
                            if index + 1 != self.source.len() {
                                return Err(self.unknown());
                            }
                            state = WsAlphaRParen;
                        }
                    }
                    _ => {}
                },
                WsAlphaWs => {
                    if digit(character) {
                        if self.yeari != 0 {
                            return Err(self.unknown());
                        }
                        self.yeari = index;
                        state = WsAlphaWsYear;
                    } else if character == '(' {
                        self.extra = index - 1;
                        state = WsOffsetWsTZDescInParen;
                    }
                }
                WsAlphaWsYear | WsOffsetWsYear => {
                    if !digit(character) {
                        return Err(self.unknown());
                    }
                }
                WsAlphaZoneOffset => {
                    if character == ' ' {
                        self.tz_offset(index)?;
                        state = WsAlphaZoneOffsetWs;
                    } else if character != ':' && !digit(character) {
                        return Err(self.unknown());
                    }
                }
                WsAlphaZoneOffsetWs => {
                    if digit(character) {
                        if self.yeari != 0 {
                            return Err(self.unknown());
                        }
                        self.yeari = index;
                        state = WsAlphaZoneOffsetWsYear;
                    } else if character == '(' {
                        self.extra = index - 1;
                        state = WsOffsetWsTZDescInParen;
                    } else {
                        return Err(self.unknown());
                    }
                }
                WsOffsetWsTZDescInParen => {
                    if character == '(' || character == ')' && index != self.source.len() - 1 {
                        return Err(self.unknown());
                    }
                }
                WsAlphaZoneOffsetWsYear => {
                    if !digit(character) {
                        return Err(self.unknown());
                    }
                    self.yearlen = index - self.yeari + 1;
                    if self.yearlen == 4 {
                        self.year()?;
                    } else if self.yearlen > 4 {
                        return Err(self.unknown());
                    }
                }
                WsAMPMMaybe => {
                    let word =
                        index + 1 == self.source.len() || self.source.get(index + 1) == Some(&b' ');
                    if matches!(character, 'm' | 'M') && word {
                        if self.parsed_ampm {
                            return Err(self.tail(index));
                        }
                        self.tzi = 0;
                        state = WsAMPM;
                        self.set(index - 1, if character == 'm' { b"pm" } else { b"PM" });
                        self.parsed_ampm = true;
                        match self.hourlen {
                            2 => self.set(self.houri, b"03"),
                            1 => self.set(self.houri, b"3"),
                            _ => return Err(self.unknown()),
                        }
                    } else {
                        state = WsAlpha;
                    }
                }
                WsAMPM => {
                    if character == ' ' {
                        state = Ws;
                    } else {
                        return Err(self.tail(index));
                    }
                }
                WsOffset => {
                    if character == ' ' {
                        self.tz_offset(index)?;
                        state = WsOffsetWs;
                    } else if character != ':' && !digit(character) {
                        return Err(self.unknown());
                    }
                }
                WsOffsetWs => match character {
                    '+' | '-' => {
                        self.extra = index - 1;
                        self.trim_extra();
                        state = WsOffset;
                    }
                    '(' => {
                        self.extra = index - 1;
                        state = WsOffsetWsTZDescInParen;
                    }
                    ' ' => {}
                    _ => {
                        if digit(character) {
                            if self.yeari != 0 {
                                return Err(self.unknown());
                            }
                            self.yeari = index;
                            state = WsOffsetWsYear;
                        } else if letter(character) {
                            if character == 'm' && self.source.get(index + 1) == Some(&b'=') {
                                self.extra = index - 1;
                                self.trim_extra();
                            } else {
                                if self.tzi != 0 {
                                    return Err(self.unknown());
                                }
                                self.tzi = index;
                                state = WsOffsetWsAlphaZone;
                            }
                        } else {
                            return Err(self.unknown());
                        }
                    }
                },
                WsOffsetWsAlphaZone => {
                    if character == ' ' {
                        if self.tzi == 0 {
                            return Err(self.unknown());
                        }
                        self.tzlen = index - self.tzi;
                        self.tz_name()?;
                        state = WsOffsetWsAlphaZoneWs;
                    }
                }
                WsOffsetWsAlphaZoneWs => match character {
                    '=' => {
                        if self.source[index - 1] != b'm' {
                            return Err(self.unknown());
                        }
                        self.extra = index - 2;
                        self.trim_extra();
                    }
                    '(' => {
                        self.extra = index - 1;
                        state = WsOffsetWsTZDescInParen;
                    }
                    ' ' => {}
                    'm' => {
                        if self.source.get(index + 1) != Some(&b'=') {
                            return Err(self.unknown());
                        }
                    }
                    _ => {
                        if !digit(character) || self.yeari != 0 {
                            return Err(self.unknown());
                        }
                        self.yeari = index;
                        state = WsOffsetWsYear;
                    }
                },
                Period => match character {
                    ' ' => {
                        self.mslen = index - self.msi;
                        self.coalesce_time(index)?;
                        state = Ws;
                    }
                    '+' | '-' => {
                        self.mslen = index - self.msi;
                        self.offseti = index;
                        state = WsOffset;
                    }
                    'Z' => {
                        state = Z;
                        self.force_utc = true;
                        self.mslen = index - self.msi;
                        self.format_set_len = self.format_set_len.max(index + 1);
                    }
                    'a' | 'A' | 'p' | 'P' => {
                        let lower = matches!(character, 'a' | 'p');
                        let word = index + 2 == self.source.len()
                            || self.source.get(index + 2) == Some(&b' ');
                        if self.source.get(index + 1) != Some(&if lower { b'm' } else { b'M' })
                            || !word
                            || self.parsed_ampm
                        {
                            return Err(self.tail(index));
                        }
                        self.mslen = index - self.msi;
                        self.coalesce_time(index)?;
                        self.set(index, if lower { b"pm" } else { b"PM" });
                        self.parsed_ampm = true;
                        index += 1;
                        state = PeriodAMPM;
                    }
                    _ => {
                        if !digit(character) {
                            return Err(self.tail(index));
                        }
                    }
                },
                PeriodAMPM => match character {
                    ' ' => state = Ws,
                    '+' | '-' => {
                        self.offseti = index;
                        state = WsOffset;
                    }
                    _ => return Err(self.tail(index)),
                },
                Z => return Err(self.tail(index)),
                WsAlphaRParen => {}
            }
            index += 1;
        }
        match state {
            WsAlpha => {
                self.tzlen = index - self.tzi;
                self.tz_name()?;
            }
            WsYear | WsAlphaWsYear => {
                self.yearlen = index - self.yeari;
                self.year()?;
            }
            WsOffsetWsTZDescInParen => {
                if index == 0 || self.source[index - 1] != b')' {
                    return Err(self.unknown());
                }
                if self.source.len() >= self.extra + 5 {
                    let count = (index - 1) - (self.extra + 2);
                    if self.tzi == 0 && (3..=4).contains(&count) {
                        self.tzi = self.extra + 2;
                        self.tzlen = count;
                        self.tz_name()?;
                        self.extra = 0;
                    }
                }
                if self.extra > 0 {
                    self.trim_extra();
                }
            }
            WsAlphaZoneOffset => self.tz_offset(index)?,
            Period => {
                self.mslen = index - self.msi;
                if self.mslen >= 10 {
                    return Err(format!(
                        "fractional seconds too long in {} near {}",
                        quote(&self.original),
                        quote(&self.source[self.msi..self.mslen])
                    ));
                }
            }
            WsOffset => self.tz_offset(self.source.len())?,
            WsOffsetWsYear => {
                self.yearlen = self.source.len() - self.yeari;
                if self.yearlen == 4 {
                    self.year()?;
                } else if self.yearlen > 4 {
                    return Err(self.unknown());
                }
            }
            WsOffsetWsAlphaZone => {
                if self.tzi == 0 {
                    return Err(self.unknown());
                }
                self.tzlen = index - self.tzi;
                self.tz_name()?;
            }
            _ => {}
        }
        self.coalesce_time(index)?;
        Ok(index)
    }
    fn finish_date(
        &mut self,
        end: usize,
        local: &LocalTimeZone,
    ) -> Result<Option<Timestamp>, String> {
        use DateState::*;
        match self.date {
            Digit => {
                let scale = match self.source.len() {
                    4 => {
                        self.entire(b"2006");
                        return Ok(None);
                    }
                    8 => {
                        self.entire(b"20060102");
                        return Ok(None);
                    }
                    14 => {
                        self.entire(b"20060102150405");
                        return Ok(None);
                    }
                    19 => 1i64,
                    16 => 1000,
                    13 => 1_000_000,
                    10 => 0,
                    _ => return Err(self.unknown()),
                };
                let value: i64 = std::str::from_utf8(&self.source)
                    .ok()
                    .and_then(|text| text.parse().ok())
                    .ok_or_else(|| self.unknown())?;
                let (seconds, nanoseconds) = if scale == 0 {
                    (value, 0)
                } else {
                    let nanos = value.wrapping_mul(scale);
                    (
                        nanos.div_euclid(1_000_000_000),
                        nanos.rem_euclid(1_000_000_000) as u32,
                    )
                };
                let zone = local.lookup(seconds);
                return Ok(Some(Timestamp {
                    seconds,
                    nanoseconds,
                    offset: zone.offset,
                    zone: zone.name,
                }));
            }
            YearDashDashOffset | DigitDotDotOffset => self.tz_offset(self.source.len())?,
            DigitDashAlphaDash | DigitDashDigitDash => {
                if !self.has_time {
                    self.dashed_year(self.source.len() - (self.moi + self.molen + 1))?;
                }
            }
            DigitDot => {
                if self.original.len() == 18 {
                    self.entire(b"20060102150405.000");
                } else {
                    self.molen = end - self.moi;
                    self.month()?;
                }
            }
            AlphaFullMonthWs => {
                if !self.has_time && self.yearlen == 0 {
                    self.yearlen = end - self.yeari;
                    self.year()?;
                }
            }
            AlphaWsDigitMoreWs => {
                self.yearlen = end - self.yeari;
                self.year()?;
            }
            DigitSt
            | YearDash
            | YearDashDash
            | YearDashAlpha
            | YearDashDashWs
            | YearDashDashT
            | DigitDotDot
            | DigitDotDotWs
            | DigitDotDotT
            | DigitWsMoYear
            | AlphaFullMonthWsDayWs
            | AlphaWsDigitMoreWsYear
            | AlphaWsAlpha
            | AlphaWsDigit
            | AlphaWsDigitYearMaybe
            | DigitSlash
            | DigitSlashAlphaSlash
            | DigitYearSlash
            | DigitColon
            | DigitChineseYear
            | DigitChineseYearWs
            | AlphaSlashDigitSlash
            | YearWsMonthWs => {}
            _ => return Err(self.unknown()),
        }
        Ok(None)
    }
    fn finish(&mut self, local: &LocalTimeZone) -> Result<Timestamp, String> {
        if self.format_set_len < self.format.len()
            && proper_end(
                &self.format,
                self.format_set_len,
                self.format.len(),
                false,
                false,
                true,
            ) < self.format.len()
        {
            return Err(self.tail(self.format_set_len));
        }
        if self.tzlen == 4
            && self.tzi + 4 < self.format.len()
            && self.format[self.tzi + 3] == b' '
            && self.format[self.tzi + 4] != b' '
        {
            self.remove_format(self.tzi + 3, 1);
        }
        if !self.full_month.is_empty() {
            let old_len = self.format.len();
            self.format.splice(
                self.moi..self.moi + self.full_month.len(),
                b"January".iter().copied(),
            );
            if self.format_set_len >= self.moi {
                self.format_set_len = self
                    .format_set_len
                    .saturating_add_signed(self.format.len() as isize - old_len as isize);
            }
            self.format_set_len = self.format_set_len.min(self.format.len()).max(7);
        }
        self.skip = self.skip.min(self.format.len());
        if self.skip > 0 {
            self.format.drain(..self.skip);
            self.format_set_len = self.format_set_len.saturating_sub(self.skip);
            self.source.drain(..self.skip);
        }
        parse_layout(
            &String::from_utf8_lossy(&self.format),
            &String::from_utf8_lossy(&self.source),
            local,
        )
    }
    fn remove_format(&mut self, start: usize, count: usize) {
        if start >= self.format.len() {
            return;
        }
        let end = self.format.len();
        let erase = if start + count >= end {
            start
        } else {
            self.format.copy_within(start + count..end, start);
            end - count
        };
        self.format[erase..].fill(b' ');
    }
}

pub(crate) fn parse_with_local_timezone(
    source: &str,
    local: &LocalTimeZone,
) -> Result<(Timestamp, String), String> {
    let mut scanner = Scanner {
        original: source.as_bytes().to_vec(),
        source: source.as_bytes().to_vec(),
        format: source.as_bytes().to_vec(),
        ..Scanner::default()
    };
    if source.len() > 78 {
        return Err(scanner.unknown());
    }
    let mut end = scanner.scan_date()?;
    if scanner.has_time {
        end = scanner.scan_time(end)?;
    }
    let utc = LocalTimeZone::utc();
    let local = if scanner.force_utc { &utc } else { local };
    let timestamp = match scanner.finish_date(end, local)? {
        Some(timestamp) => timestamp,
        None => scanner.finish(local)?,
    };
    Ok((
        timestamp,
        String::from_utf8_lossy(&scanner.format).into_owned(),
    ))
}
