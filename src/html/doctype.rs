use crate::{entities::unescape, metadata::lowercase, Attribute};
use html5gum::{DefaultEmitter, Token, Tokenizer};

pub(super) struct Declaration {
    pub name: String,
    pub attrs: Vec<Attribute>,
    pub quirks: bool,
}

fn whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

pub(super) fn declaration(source: &str) -> Option<Declaration> {
    for token in Tokenizer::new_with_emitter(source, DefaultEmitter::<usize>::new_with_span()) {
        match token.unwrap() {
            Token::Error(_) | Token::Comment(_) => {}
            Token::String(text) if text.iter().all(|&byte| whitespace(char::from(byte))) => {}
            Token::Doctype(token) => {
                let raw = &source[token.span.start..token.span.end];
                let raw = raw[9..]
                    .strip_suffix('>')
                    .unwrap_or(&raw[9..])
                    .trim_start_matches(whitespace);
                return Some(parse(&unescape(
                    &raw.replace("\r\n", "\n").replace('\r', "\n"),
                )));
            }
            _ => return None,
        }
    }
    None
}

fn parse(mut source: &str) -> Declaration {
    let space = source.find(whitespace).unwrap_or(source.len());
    let mut result = Declaration {
        name: lowercase(&source[..space]),
        attrs: Vec::new(),
        quirks: &source[..space] != "html",
    };
    source = source[space..].trim_start_matches(whitespace);
    let Some(prefix) = source.get(..6) else {
        result.quirks |= !source.is_empty();
        return result;
    };
    let mut key = lowercase(prefix);
    source = &source[6..];
    while matches!(key.as_str(), "public" | "system") {
        source = source.trim_start_matches(whitespace);
        let Some(quote @ ('\'' | '"')) = source.chars().next() else {
            break;
        };
        source = &source[1..];
        let (identifier, remaining) = source.find(quote).map_or((source, ""), |position| {
            (&source[..position], &source[position + 1..])
        });
        result.attrs.push(Attribute {
            namespace: crate::Text::new(),
            key: key.as_str().into(),
            value: identifier.into(),
        });
        source = remaining;
        key = if key == "public" {
            "system".into()
        } else {
            String::new()
        };
    }
    if !key.is_empty() || !source.is_empty() {
        result.quirks = true;
    } else if let Some(first) = result.attrs.first() {
        if first.key == "public" {
            let public = lowercase(&first.value);
            result.quirks |= matches!(
                public.as_str(),
                "-//w3o//dtd w3 html strict 3.0//en//"
                    | "-/w3d/dtd html 4.0 transitional/en"
                    | "html"
            ) || QUIRKY_IDS.iter().any(|prefix| public.starts_with(prefix));
            if result.attrs.len() == 1
                && (public.starts_with("-//w3c//dtd html 4.01 frameset//")
                    || public.starts_with("-//w3c//dtd html 4.01 transitional//"))
            {
                result.quirks = true;
            }
        }
        if let Some(last) = result.attrs.last() {
            if last.key == "system"
                && last.value.eq_ignore_ascii_case(
                    "http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd",
                )
            {
                result.quirks = true;
            }
        }
    }
    result
}

const QUIRKY_IDS: &[&str] = &[
    "+//silmaril//dtd html pro v0r11 19970101//",
    "-//advasoft ltd//dtd html 3.0 aswedit + extensions//",
    "-//as//dtd html 3.0 aswedit + extensions//",
    "-//ietf//dtd html 2.0 level 1//",
    "-//ietf//dtd html 2.0 level 2//",
    "-//ietf//dtd html 2.0 strict level 1//",
    "-//ietf//dtd html 2.0 strict level 2//",
    "-//ietf//dtd html 2.0 strict//",
    "-//ietf//dtd html 2.0//",
    "-//ietf//dtd html 2.1e//",
    "-//ietf//dtd html 3.0//",
    "-//ietf//dtd html 3.2 final//",
    "-//ietf//dtd html 3.2//",
    "-//ietf//dtd html 3//",
    "-//ietf//dtd html level 0//",
    "-//ietf//dtd html level 1//",
    "-//ietf//dtd html level 2//",
    "-//ietf//dtd html level 3//",
    "-//ietf//dtd html strict level 0//",
    "-//ietf//dtd html strict level 1//",
    "-//ietf//dtd html strict level 2//",
    "-//ietf//dtd html strict level 3//",
    "-//ietf//dtd html strict//",
    "-//ietf//dtd html//",
    "-//metrius//dtd metrius presentational//",
    "-//microsoft//dtd internet explorer 2.0 html strict//",
    "-//microsoft//dtd internet explorer 2.0 html//",
    "-//microsoft//dtd internet explorer 2.0 tables//",
    "-//microsoft//dtd internet explorer 3.0 html strict//",
    "-//microsoft//dtd internet explorer 3.0 html//",
    "-//microsoft//dtd internet explorer 3.0 tables//",
    "-//netscape comm. corp.//dtd html//",
    "-//netscape comm. corp.//dtd strict html//",
    "-//o'reilly and associates//dtd html 2.0//",
    "-//o'reilly and associates//dtd html extended 1.0//",
    "-//o'reilly and associates//dtd html extended relaxed 1.0//",
    "-//softquad software//dtd hotmetal pro 6.0::19990601::extensions to html 4.0//",
    "-//softquad//dtd hotmetal pro 4.0::19971010::extensions to html 4.0//",
    "-//spyglass//dtd html 2.0 extended//",
    "-//sq//dtd html 2.0 hotmetal + extensions//",
    "-//sun microsystems corp.//dtd hotjava html//",
    "-//sun microsystems corp.//dtd hotjava strict html//",
    "-//w3c//dtd html 3 1995-03-24//",
    "-//w3c//dtd html 3.2 draft//",
    "-//w3c//dtd html 3.2 final//",
    "-//w3c//dtd html 3.2//",
    "-//w3c//dtd html 3.2s draft//",
    "-//w3c//dtd html 4.0 frameset//",
    "-//w3c//dtd html 4.0 transitional//",
    "-//w3c//dtd html experimental 19960712//",
    "-//w3c//dtd html experimental 970421//",
    "-//w3c//dtd w3 html//",
    "-//w3o//dtd w3 html 3.0//",
    "-//webtechs//dtd mozilla html 2.0//",
    "-//webtechs//dtd mozilla html//",
];
