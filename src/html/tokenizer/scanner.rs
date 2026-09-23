use html5ever::{
    buffer_queue::{BufferQueue, SetResult},
    SmallCharSet,
};

fn nonmember_prefix_len(set: SmallCharSet, source: &str) -> u32 {
    if source.len() >= 32 {
        macro_rules! scan {
            ($($byte:expr),+) => {
                if set.bits == (0 $(| (1u64 << $byte))+) {
                    return jetscii::bytes!($($byte),+)
                        .find(source.as_bytes())
                        .unwrap_or(source.len()) as u32;
                }
            };
        }
        scan!(b'\r', b'\n', b'\0', b'<');
        scan!(b'\r', b'\n', b'\0', b'<', b'&');
        scan!(b'\r', b'\n', b'\0', b'<', b'-');
        scan!(b'\r', b'\n', b'\0', b'"', b'&');
        scan!(b'\r', b'\n', b'\0', b'\'', b'&');
        scan!(b'\r', b'\n', b'\0', b'\t', b'\x0c', b' ', b'>', b'&');
        scan!(b'\r', b'\n', b'\0');
    }
    set.nonmember_prefix_len(source)
}

pub(super) fn pop_except_from(input: &BufferQueue, set: SmallCharSet) -> Option<SetResult> {
    let mut chunk = input.peek_front_chunk_mut()?;
    let length = nonmember_prefix_len(set, &chunk);
    let result = if length == 0 {
        SetResult::FromSet(chunk.pop_front_char().expect("empty buffer in queue"))
    } else {
        let prefix = chunk.subtendril(0, length);
        chunk.pop_front(length);
        SetResult::NotFromSet(prefix)
    };
    if chunk.is_empty() {
        drop(chunk);
        input.pop_front();
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectorized_prefix_matches_released_scanner() {
        for delimiters in [
            b"\r\n\0<".as_slice(),
            b"\r\n\0<&",
            b"\r\n\0<-",
            b"\r\n\0\"&",
            b"\r\n\0'&",
            b"\r\n\0\t\x0c >&",
            b"\r\n\0",
            b"%!?",
        ] {
            let set = SmallCharSet {
                bits: delimiters
                    .iter()
                    .fold(0, |bits, byte| bits | (1u64 << byte)),
            };
            for position in 0..65 {
                for character in (0..=255).map(char::from).chain(['\u{20ac}', '\u{1f642}']) {
                    let source = format!("{}{character}{}", "x".repeat(position), "z".repeat(64));
                    assert_eq!(
                        nonmember_prefix_len(set, &source),
                        set.nonmember_prefix_len(&source)
                    );
                }
                let source = "x".repeat(position);
                assert_eq!(
                    nonmember_prefix_len(set, &source),
                    set.nonmember_prefix_len(&source)
                );
            }
        }
    }
}
