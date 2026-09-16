use serde::Deserialize;
use serde_json::Value;
use std::borrow::Cow;

pub(crate) struct JsonValue(pub Value);

impl Drop for JsonValue {
    fn drop(&mut self) {
        let mut pending = vec![std::mem::replace(&mut self.0, Value::Null)];
        while let Some(value) = pending.pop() {
            match value {
                Value::Array(values) => pending.extend(values),
                Value::Object(values) => pending.extend(values.into_values()),
                _ => {}
            }
        }
    }
}

fn hex_escape(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 6 || &bytes[..2] != b"\\u" {
        return None;
    }
    let mut value = 0;
    for &byte in &bytes[2..6] {
        value = value * 16 + (byte as char).to_digit(16)? as u16;
    }
    Some(value)
}

fn compatible_input(source: &str) -> Option<Cow<'_, str>> {
    let bytes = source.as_bytes();
    let mut replacement: Option<Vec<u8>> = None;
    let mut index = 0;
    let mut quoted = false;
    let mut depth = 0usize;
    while index < bytes.len() {
        if quoted && bytes[index] == b'\\' {
            if let Some(value) = hex_escape(&bytes[index..]) {
                if (0xd800..=0xdbff).contains(&value)
                    && hex_escape(&bytes[index + 6..])
                        .is_some_and(|low| (0xdc00..=0xdfff).contains(&low))
                {
                    index += 12;
                    continue;
                }
                if (0xd800..=0xdfff).contains(&value) {
                    replacement.get_or_insert_with(|| bytes.to_vec())[index + 2..index + 6]
                        .copy_from_slice(b"fffd");
                }
                index += 6;
                continue;
            }
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            quoted = !quoted;
        } else if !quoted {
            match bytes[index] {
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 10_000 {
                        return None;
                    }
                }
                b']' | b'}' => depth = depth.checked_sub(1)?,
                _ => {}
            }
        }
        index += 1;
    }
    Some(match replacement {
        Some(bytes) => Cow::Owned(String::from_utf8(bytes).unwrap()),
        None => Cow::Borrowed(source),
    })
}

pub(crate) fn parse(source: &str) -> Option<JsonValue> {
    let source = compatible_input(source)?;
    let mut deserializer = serde_json::Deserializer::from_str(&source);
    deserializer.disable_recursion_limit();
    let value = Value::deserialize(serde_stacker::Deserializer::new(&mut deserializer)).ok()?;
    let value = JsonValue(value);
    deserializer.end().ok()?;
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn go_json_depth_limit() {
        let source = format!("{}0{}", "[".repeat(10_000), "]".repeat(10_000));
        assert!(parse(&source).is_some());
        assert!(parse(&format!("[{source}]")).is_none());
        assert!(parse(r#"{"value":"bad\xescape"}"#).is_none());
        assert_eq!(
            parse(r#"{"value":"\\ud800"}"#).unwrap().0["value"],
            r"\ud800"
        );
    }
}
