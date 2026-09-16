use std::{borrow::Cow, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UrlError {
    pub input: String,
    pub reason: String,
}

impl fmt::Display for UrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parse {}: {}",
            quote(self.input.as_bytes()),
            self.reason
        )
    }
}

impl std::error::Error for UrlError {}

#[derive(Clone, Debug, Default)]
pub struct Url {
    pub scheme: String,
    pub opaque: String,
    pub user: Option<(Vec<u8>, Option<Vec<u8>>)>,
    pub host: Vec<u8>,
    pub path: Vec<u8>,
    pub raw_path: String,
    pub query: String,
    pub force_query: bool,
    pub fragment: Vec<u8>,
    pub raw_fragment: String,
    pub omit_host: bool,
}

#[derive(Clone, Copy)]
enum Encoding {
    Path,
    Fragment,
    Host,
    Zone,
    User,
}

fn allowed(byte: u8, encoding: Encoding) -> bool {
    if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
        return true;
    }
    match encoding {
        Encoding::Path => b"$&+,/:;=@".contains(&byte),
        Encoding::Fragment => b"$&+,/:;=?@!()*".contains(&byte),
        Encoding::Host | Encoding::Zone => b"!$&'()*+,;=:[]<>\"".contains(&byte),
        Encoding::User => b"$&+,;=".contains(&byte),
    }
}

fn escape(value: &[u8], encoding: Encoding) -> String {
    let mut output = String::new();
    for &byte in value {
        if allowed(byte, encoding) {
            output.push(byte as char);
        } else {
            const HEX: &[u8] = b"0123456789ABCDEF";
            output.push('%');
            output.push(HEX[(byte >> 4) as usize] as char);
            output.push(HEX[(byte & 15) as usize] as char);
        }
    }
    output
}

fn unescape(value: &str) -> Option<Vec<u8>> {
    unescape_checked(value, Encoding::Path).ok()
}

fn unescape_checked(value: &str, encoding: Encoding) -> Result<Vec<u8>, String> {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return Err(format!(
                    "invalid URL escape {}",
                    quote(&bytes[index..bytes.len().min(index + 3)])
                ));
            }
            let decoded = (bytes[index + 1] as char).to_digit(16).unwrap() as u8 * 16
                + (bytes[index + 2] as char).to_digit(16).unwrap() as u8;
            if (matches!(encoding, Encoding::Host) && decoded < 128 && decoded != b'%')
                || (matches!(encoding, Encoding::Zone)
                    && decoded != b'%'
                    && decoded != b' '
                    && !allowed(decoded, Encoding::Host))
            {
                return Err(format!(
                    "invalid URL escape {}",
                    quote(&bytes[index..index + 3])
                ));
            }
            index += 3;
        } else {
            if matches!(encoding, Encoding::Host | Encoding::Zone)
                && bytes[index] < 128
                && !allowed(bytes[index], encoding)
            {
                return Err(format!(
                    "invalid character {} in host name",
                    quote(&bytes[index..index + 1])
                ));
            }
            index += 1;
        }
    }
    Ok(percent_encoding::percent_decode_str(value).collect())
}

fn encoded(raw: &str, decoded: &[u8], encoding: Encoding) -> String {
    if !raw.is_empty()
        && raw
            .bytes()
            .all(|byte| b"!$&'()*+,;=:@[]%".contains(&byte) || allowed(byte, encoding))
        && unescape(raw).as_deref() == Some(decoded)
    {
        return raw.into();
    }
    escape(decoded, encoding)
}

fn port_valid(value: &str) -> bool {
    value.is_empty()
        || value
            .strip_prefix(':')
            .is_some_and(|port| port.bytes().all(|byte| byte.is_ascii_digit()))
}

fn parse_host(scheme: &str, value: &str) -> Result<Vec<u8>, String> {
    if let Some(bracket) = value.rfind('[') {
        if bracket != 0 {
            return Err("invalid IP-literal".into());
        }
        let end = value.rfind(']').ok_or("missing ']' in host")?;
        let port = &value[end + 1..];
        if !port_valid(port) {
            return Err(format!(
                "invalid port {} after host",
                quote(port.as_bytes())
            ));
        }
        let port = unescape_checked(port, Encoding::Host)?;
        let hostname = &value[1..end];
        let hostname = if let Some(zone) = hostname.find("%25") {
            let mut address = unescape_checked(&hostname[..zone], Encoding::Host)?;
            address.extend(unescape_checked(&hostname[zone..], Encoding::Zone)?);
            address
        } else {
            unescape_checked(hostname, Encoding::Host)?
        };
        if !ip_address(&hostname).map_err(|error| format!("invalid host: {error}"))? {
            return Err("invalid IP-literal".into());
        }
        let mut output = Vec::from(b"[");
        output.extend(hostname);
        output.push(b']');
        output.extend(port);
        return Ok(output);
    } else if let Some(first) = value.find(':') {
        let index = if matches!(scheme, "http" | "https") {
            first
        } else {
            value.rfind(':').unwrap()
        };
        if !port_valid(&value[index..]) {
            return Err(format!(
                "invalid port {} after host",
                quote(&value.as_bytes()[index..])
            ));
        }
    }
    unescape_checked(value, Encoding::Host)
}

fn quote(value: &[u8]) -> String {
    use std::fmt::Write;
    static PRINTABLE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{L}\p{M}\p{N}\p{P}\p{S} ]$").unwrap());
    let mut output = String::from("\"");
    for chunk in value.utf8_chunks() {
        for character in chunk.valid().chars() {
            match character {
                '\"' => output.push_str("\\\""),
                '\\' => output.push_str("\\\\"),
                '\x07' => output.push_str("\\a"),
                '\x08' => output.push_str("\\b"),
                '\x0c' => output.push_str("\\f"),
                '\n' => output.push_str("\\n"),
                '\r' => output.push_str("\\r"),
                '\t' => output.push_str("\\t"),
                '\x0b' => output.push_str("\\v"),
                character if character < ' ' || character == '\x7f' => {
                    write!(output, "\\x{:02x}", character as u32).unwrap()
                }
                character
                    if character.is_ascii()
                        || PRINTABLE.is_match(character.encode_utf8(&mut [0; 4])) =>
                {
                    output.push(character)
                }
                character if character as u32 <= 0xffff => {
                    write!(output, "\\u{:04x}", character as u32).unwrap()
                }
                character => write!(output, "\\U{:08x}", character as u32).unwrap(),
            }
        }
        for byte in chunk.invalid() {
            write!(output, "\\x{byte:02x}").unwrap();
        }
    }
    output.push('"');
    output
}

fn ip_error(input: &[u8], message: &str, rest: &[u8]) -> String {
    let mut output = format!("ParseAddr({}): {message}", quote(input));
    if !rest.is_empty() {
        output.push_str(&format!(" (at {})", quote(rest)));
    }
    output
}

fn ipv4_fields(input: &[u8], fields: &[u8]) -> Result<(), String> {
    let mut value = 0;
    let mut position = 0;
    let mut digits = 0;
    for (index, &byte) in fields.iter().enumerate() {
        if byte.is_ascii_digit() {
            if digits == 1 && value == 0 {
                return Err(ip_error(
                    input,
                    "IPv4 field has octet with leading zero",
                    &[],
                ));
            }
            value = value * 10 + u32::from(byte - b'0');
            digits += 1;
            if value > 255 {
                return Err(ip_error(input, "IPv4 field has value >255", &[]));
            }
        } else if byte == b'.' {
            if index == 0 || index == fields.len() - 1 || fields[index - 1] == b'.' {
                return Err(ip_error(
                    input,
                    "IPv4 field must have at least one digit",
                    &fields[index..],
                ));
            }
            if position == 3 {
                return Err(ip_error(input, "IPv4 address too long", &[]));
            }
            position += 1;
            value = 0;
            digits = 0;
        } else {
            return Err(ip_error(input, "unexpected character", &fields[index..]));
        }
    }
    if position < 3 {
        return Err(ip_error(input, "IPv4 address too short", &[]));
    }
    Ok(())
}

fn ip_address(input: &[u8]) -> Result<bool, String> {
    match input
        .iter()
        .find(|&&byte| matches!(byte, b'.' | b':' | b'%'))
    {
        Some(b'.') => {
            ipv4_fields(input, input)?;
            return Ok(false);
        }
        Some(b'%') => return Err(ip_error(input, "missing IPv6 address", &[])),
        None => return Err(ip_error(input, "unable to parse IP", &[])),
        _ => {}
    }
    let mut rest = if let Some(zone) = input.iter().position(|&byte| byte == b'%') {
        if zone + 1 == input.len() {
            return Err(ip_error(input, "zone must be a non-empty string", &[]));
        }
        &input[..zone]
    } else {
        input
    };
    let mut ellipsis = false;
    if rest.starts_with(b"::") {
        ellipsis = true;
        rest = &rest[2..];
        if rest.is_empty() {
            return Ok(true);
        }
    }
    let mut width = 0;
    while width < 16 {
        let mut digits = 0;
        while digits < rest.len() && rest[digits].is_ascii_hexdigit() {
            if digits > 3 {
                return Err(ip_error(
                    input,
                    "each group must have 4 or less digits",
                    rest,
                ));
            }
            digits += 1;
        }
        if digits == 0 {
            return Err(ip_error(
                input,
                "each colon-separated field must have at least one digit",
                rest,
            ));
        }
        if rest.get(digits) == Some(&b'.') {
            if !ellipsis && width != 12 {
                return Err(ip_error(
                    input,
                    "embedded IPv4 address must replace the final 2 fields of the address",
                    rest,
                ));
            }
            if width + 4 > 16 {
                return Err(ip_error(
                    input,
                    "too many hex fields to fit an embedded IPv4 at the end of the address",
                    rest,
                ));
            }
            ipv4_fields(input, rest)?;
            rest = &[];
            width += 4;
            break;
        }
        width += 2;
        rest = &rest[digits..];
        if rest.is_empty() {
            break;
        }
        if rest[0] != b':' {
            return Err(ip_error(input, "unexpected character, want colon", rest));
        }
        if rest.len() == 1 {
            return Err(ip_error(
                input,
                "colon must be followed by more characters",
                rest,
            ));
        }
        rest = &rest[1..];
        if rest[0] == b':' {
            if ellipsis {
                return Err(ip_error(input, "multiple :: in address", rest));
            }
            ellipsis = true;
            rest = &rest[1..];
            if rest.is_empty() {
                break;
            }
        }
    }
    if !rest.is_empty() {
        return Err(ip_error(input, "trailing garbage after address", rest));
    }
    if width < 16 && !ellipsis {
        return Err(ip_error(input, "address string too short", &[]));
    }
    if width == 16 && ellipsis {
        return Err(ip_error(
            input,
            "the :: must expand to at least one field of zeros",
            &[],
        ));
    }
    Ok(true)
}

impl Url {
    pub fn parse(value: &str) -> Option<Self> {
        Self::try_parse(value).ok()
    }
    pub fn request(value: &str) -> Option<Self> {
        Self::try_request(value).ok()
    }

    pub fn try_parse(value: &str) -> Result<Self, UrlError> {
        let (base, fragment) = value.split_once('#').unwrap_or((value, ""));
        let mut url = Self::parse_inner(base, false).map_err(|reason| UrlError {
            input: base.into(),
            reason,
        })?;
        if !fragment.is_empty() {
            url.fragment =
                unescape_checked(fragment, Encoding::Fragment).map_err(|reason| UrlError {
                    input: value.into(),
                    reason,
                })?;
            if escape(&url.fragment, Encoding::Fragment) != fragment {
                url.raw_fragment = fragment.into();
            }
        }
        Ok(url)
    }

    pub fn try_request(value: &str) -> Result<Self, UrlError> {
        Self::parse_inner(value, true).map_err(|reason| UrlError {
            input: value.into(),
            reason,
        })
    }

    fn parse_inner(value: &str, request: bool) -> Result<Self, String> {
        if value.bytes().any(|byte| byte < 32 || byte == 127) {
            return Err("net/url: invalid control character in URL".into());
        }
        if request && value.is_empty() {
            return Err("empty url".into());
        }
        let mut url = Self::default();
        if value == "*" {
            url.path = b"*".to_vec();
            return Ok(url);
        }
        let mut rest = value;
        for (index, byte) in value.bytes().enumerate() {
            if byte.is_ascii_alphabetic() {
                continue;
            }
            if byte.is_ascii_digit() || b"+-.".contains(&byte) {
                if index == 0 {
                    break;
                }
            } else if byte == b':' {
                if index == 0 {
                    return Err("missing protocol scheme".into());
                }
                url.scheme = value[..index].to_ascii_lowercase();
                rest = &value[index + 1..];
                break;
            } else {
                break;
            }
        }
        if let Some((path, query)) = rest.split_once('?') {
            url.force_query = query.is_empty();
            url.query = query.into();
            rest = path;
        }
        if !rest.starts_with('/') {
            if !url.scheme.is_empty() {
                url.opaque = rest.into();
                return Ok(url);
            }
            if request {
                return Err("invalid URI for request".into());
            }
            if rest
                .split('/')
                .next()
                .is_some_and(|part| part.contains(':'))
            {
                return Err("first path segment in URL cannot contain colon".into());
            }
        }
        if (!url.scheme.is_empty() || (!request && !rest.starts_with("///")))
            && rest.starts_with("//")
        {
            let authority = &rest[2..];
            let end = authority.find('/').unwrap_or(authority.len());
            rest = &authority[end..];
            let authority = &authority[..end];
            if let Some((user, host)) = authority.rsplit_once('@') {
                url.host = parse_host(&url.scheme, host)?;
                if !user.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:%@".contains(&byte)
                }) {
                    return Err("net/url: invalid userinfo".into());
                }
                url.user = Some(if let Some((name, password)) = user.split_once(':') {
                    (
                        unescape_checked(name, Encoding::User)?,
                        Some(unescape_checked(password, Encoding::User)?),
                    )
                } else {
                    (unescape_checked(user, Encoding::User)?, None)
                });
            } else {
                url.host = parse_host(&url.scheme, authority)?;
            }
        } else if !url.scheme.is_empty() && rest.starts_with('/') {
            url.omit_host = true;
        }
        url.set_path(rest)?;
        Ok(url)
    }

    fn set_path(&mut self, value: &str) -> Result<(), String> {
        self.path = unescape_checked(value, Encoding::Path)?;
        self.raw_path = if escape(&self.path, Encoding::Path) == value {
            String::new()
        } else {
            value.into()
        };
        Ok(())
    }

    pub fn hostname(&self) -> Cow<'_, str> {
        let mut host = self.host.as_slice();
        if let Some(colon) = host.iter().rposition(|&byte| byte == b':') {
            if host[colon + 1..].iter().all(u8::is_ascii_digit) {
                host = &host[..colon];
            }
        }
        if host.starts_with(b"[") && host.ends_with(b"]") {
            host = &host[1..host.len() - 1];
        }
        String::from_utf8_lossy(host)
    }

    pub fn escaped_path(&self) -> String {
        if self.path == b"*" {
            "*".into()
        } else {
            encoded(&self.raw_path, &self.path, Encoding::Path)
        }
    }

    pub fn resolve(&self, mut reference: Self) -> Self {
        let absolute =
            !reference.scheme.is_empty() || !reference.host.is_empty() || reference.user.is_some();
        if reference.scheme.is_empty() {
            reference.scheme.clone_from(&self.scheme);
        }
        if absolute {
            let path = resolve_path(&reference.escaped_path(), "");
            reference.set_path(&path).unwrap();
            return reference;
        }
        if !reference.opaque.is_empty() {
            reference.user = None;
            reference.host.clear();
            reference.path.clear();
            return reference;
        }
        if reference.path.is_empty() && !reference.force_query && reference.query.is_empty() {
            reference.query.clone_from(&self.query);
            if reference.fragment.is_empty() {
                reference.fragment.clone_from(&self.fragment);
                reference.raw_fragment.clone_from(&self.raw_fragment);
            }
        }
        if reference.path.is_empty() && !self.opaque.is_empty() {
            reference.opaque.clone_from(&self.opaque);
            reference.user = None;
            reference.host.clear();
            reference.path.clear();
            return reference;
        }
        reference.host.clone_from(&self.host);
        reference.user.clone_from(&self.user);
        let path = resolve_path(&self.escaped_path(), &reference.escaped_path());
        reference.set_path(&path).unwrap();
        reference
    }

    fn render(&self) -> String {
        let mut output = String::new();
        if !self.scheme.is_empty() {
            output.push_str(&self.scheme);
            output.push(':');
        }
        if !self.opaque.is_empty() {
            output.push_str(&self.opaque);
        } else {
            let omit_host = self.omit_host && self.host.is_empty() && self.user.is_none();
            if (!self.scheme.is_empty() || !self.host.is_empty() || self.user.is_some())
                && !omit_host
            {
                if !self.host.is_empty() || !self.path.is_empty() || self.user.is_some() {
                    output.push_str("//");
                }
                if let Some((user, password)) = &self.user {
                    output.push_str(&escape(user, Encoding::User));
                    if let Some(password) = password {
                        output.push(':');
                        output.push_str(&escape(password, Encoding::User));
                    }
                    output.push('@');
                }
                output.push_str(&escape(&self.host, Encoding::Host));
            }
            let mut path = self.escaped_path();
            if omit_host && path.starts_with("//") {
                output.push_str("%2F");
                path.remove(0);
            }
            if !path.is_empty() && !path.starts_with('/') && !self.host.is_empty() {
                output.push('/');
            }
            if output.is_empty()
                && path
                    .split('/')
                    .next()
                    .is_some_and(|part| part.contains(':'))
            {
                output.push_str("./");
            }
            output.push_str(&path);
        }
        if self.force_query || !self.query.is_empty() {
            output.push('?');
            output.push_str(&self.query);
        }
        if !self.fragment.is_empty() {
            output.push('#');
            output.push_str(&encoded(
                &self.raw_fragment,
                &self.fragment,
                Encoding::Fragment,
            ));
        }
        output
    }
}

impl fmt::Display for Url {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render())
    }
}

fn resolve_path(base: &str, reference: &str) -> String {
    let full = if reference.is_empty() {
        base.into()
    } else if !reference.starts_with('/') {
        format!(
            "{}{reference}",
            &base[..base.rfind('/').map_or(0, |index| index + 1)]
        )
    } else {
        reference.to_owned()
    };
    if full.is_empty() {
        return full;
    }
    let mut output = String::from("/");
    let mut first = true;
    let mut last = "";
    for part in full.split('/') {
        last = part;
        match part {
            "." => {
                first = false;
                continue;
            }
            ".." => {
                if let Some(index) = output[1..].rfind('/') {
                    output.truncate(index + 1);
                } else {
                    output.truncate(1);
                }
                first = output.len() == 1;
            }
            _ => {
                if !first {
                    output.push('/');
                }
                output.push_str(part);
                first = false;
            }
        }
    }
    if matches!(last, "." | "..") {
        output.push('/');
    }
    if output.starts_with("//") {
        output.remove(0);
    }
    output
}

pub(crate) fn absolute(uri: &str, base: Option<&Url>) -> String {
    let Some(base) = base else {
        return uri.into();
    };
    if uri.is_empty() || uri.starts_with('#') || uri.starts_with("data:") {
        return uri.into();
    }
    if uri.starts_with("//") {
        return format!("{}:{uri}", base.scheme);
    }
    if uri.starts_with("https://") || uri.starts_with("http://") {
        return uri.into();
    }
    if Url::request(uri).is_some_and(|url| !url.scheme.is_empty() && !url.hostname().is_empty()) {
        return uri.into();
    }
    Url::parse(uri).map_or_else(
        || uri.into(),
        |reference| base.resolve(reference).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::{absolute, Url};

    #[test]
    fn readeck_absolute_uri() {
        let base = Url::request("http://localhost:8080/absolute/").unwrap();
        for (source, expected) in [
            ("#here", "#here"),
            ("/test/123", "http://localhost:8080/test/123"),
            ("test/123", "http://localhost:8080/absolute/test/123"),
            ("//www.google.com", "http://www.google.com"),
            ("https://www.google.com", "https://www.google.com"),
            ("ftp://ftp.server.com", "ftp://ftp.server.com"),
            (
                "www.google.com",
                "http://localhost:8080/absolute/www.google.com",
            ),
            (
                "http//www.google.com",
                "http://localhost:8080/absolute/http//www.google.com",
            ),
            ("../hello/relative", "http://localhost:8080/hello/relative"),
        ] {
            assert_eq!(absolute(source, Some(&base)), expected);
            assert_eq!(absolute(source, None), source);
        }
    }
}
