use markup5ever::data::{C1_REPLACEMENTS, NAMED_ENTITIES};

pub(crate) fn unescape(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find('&') {
        output.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        let bytes = remaining.as_bytes();
        if bytes.len() <= 1 || (bytes[1] == b'#' && bytes.len() <= 3) {
            output.push('&');
            remaining = &remaining[1..];
            continue;
        }
        if bytes[1] == b'#' {
            let mut index = 2;
            let hexadecimal = matches!(bytes[index], b'x' | b'X');
            if hexadecimal {
                index += 1;
            }
            let mut value = 0i32;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if let Some(digit) = (byte as char).to_digit(if hexadecimal { 16 } else { 10 }) {
                    value = value
                        .wrapping_mul(if hexadecimal { 16 } else { 10 })
                        .wrapping_add(digit as i32);
                    continue;
                }
                if byte != b';' {
                    index -= 1;
                }
                break;
            }
            if index <= 3 {
                output.push('&');
                remaining = &remaining[1..];
                continue;
            }
            let character = if (0x80..=0x9f).contains(&value) {
                C1_REPLACEMENTS[value as usize - 0x80]
                    .unwrap_or(char::from_u32(value as u32).unwrap())
            } else if value == 0 {
                '\u{fffd}'
            } else {
                char::from_u32(value as u32).unwrap_or('\u{fffd}')
            };
            output.push(character);
            remaining = &remaining[index..];
            continue;
        }
        let mut end = 1;
        while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
            end += 1;
        }
        if bytes.get(end) == Some(&b';') {
            end += 1;
        }
        let name = &remaining[1..end];
        let exact = NAMED_ENTITIES
            .get(name)
            .copied()
            .filter(|&(first, _)| first != 0);
        let found = exact.map(|pair| (end, pair)).or_else(|| {
            (2..name.len()).rev().find_map(|length| {
                NAMED_ENTITIES
                    .get(&name[..length])
                    .copied()
                    .filter(|&(first, _)| first != 0)
                    .map(|pair| (length + 1, pair))
            })
        });
        if let Some((length, (first, second))) = found {
            output.push(char::from_u32(first).unwrap());
            if second != 0 {
                output.push(char::from_u32(second).unwrap());
            }
            remaining = &remaining[length..];
        } else {
            output.push_str(&remaining[..end]);
            remaining = &remaining[end..];
        }
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::unescape;

    #[test]
    fn go_entity_edges() {
        for (source, expected) in [
            (
                "&amp;lt; &notit; &NotEqualTilde;",
                "&lt; \u{ac}it; \u{2242}\u{338}",
            ),
            ("&#65; &#x41; &#128; &#129;", "A A \u{20ac} \u{81}"),
            (
                "&#x; &#; &#x &#0; &#55296;",
                "\u{fffd} &#; &#x \u{fffd} \u{fffd}",
            ),
            ("&#4294967361; &#2147483648;", "A \u{fffd}"),
            ("&unknown; & <tag>\r", "&unknown; & <tag>\r"),
        ] {
            assert_eq!(unescape(source), expected, "{source}");
        }
    }
}
