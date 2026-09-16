use crate::{check::is_space, Document, Error, Kind, NodeId};
use std::io::Write;

fn escape(writer: &mut impl Write, source: &str, comment: bool) -> std::io::Result<()> {
    let mut start = 0;
    for (offset, &byte) in source.as_bytes().iter().enumerate() {
        let escaped = match byte {
            b'&' => "&amp;",
            b'>' if !comment
                || offset == 0
                || matches!(source.as_bytes()[offset - 1], b'!' | b'-') =>
            {
                "&gt;"
            }
            b'<' if !comment => "&lt;",
            b'\'' if !comment => "&#39;",
            b'"' if !comment => "&#34;",
            b'\r' if !comment => "&#13;",
            _ => continue,
        };
        writer.write_all(&source.as_bytes()[start..offset])?;
        writer.write_all(escaped.as_bytes())?;
        start = offset + 1;
    }
    writer.write_all(&source.as_bytes()[start..])
}

fn doctype_identifier(writer: &mut impl Write, source: &str) -> Result<(), Error> {
    let quote = if source.contains('"') {
        if source.contains('\'') {
            return Err(Error::Html(
                "doctype contains both quote types, cannot be safely rendered".into(),
            ));
        }
        b'\''
    } else {
        b'"'
    };
    writer.write_all(&[quote])?;
    writer.write_all(source.replace('>', "&gt;").as_bytes())?;
    writer.write_all(&[quote])?;
    Ok(())
}

pub fn html(document: &Document, root: NodeId, mut writer: impl Write) -> Result<(), Error> {
    html_with_context(document, None, root, &mut writer)
}

pub fn html_dom(document: &crate::Dom, root: NodeId, mut writer: impl Write) -> Result<(), Error> {
    html_with_context(&document.document, Some(document), root, &mut writer)
}

fn literal_children(document: &Document, context: Option<&crate::Dom>, root: NodeId) -> bool {
    if !matches!(
        document.nodes[root].tag.as_str(),
        "iframe" | "noembed" | "noframes" | "noscript" | "plaintext" | "script" | "style" | "xmp"
    ) {
        return false;
    }
    let Some(context) = context else {
        return true;
    };
    if !context.namespaces[root].is_empty() {
        return false;
    }
    let mut parent = document.nodes[root].parent;
    while let Some(index) = parent {
        let namespace = context.namespaces[index].as_str();
        if !namespace.is_empty() {
            return match context.atoms[index].as_str() {
                "desc" | "foreignObject" | "title" => namespace == "svg",
                "annotation-xml" if namespace == "math" => {
                    let encoding = document.nodes[index].attr("encoding");
                    encoding.eq_ignore_ascii_case("text/html")
                        || encoding.eq_ignore_ascii_case("application/xhtml+xml")
                }
                _ => false,
            };
        }
        parent = document.nodes[index].parent;
    }
    true
}

fn html_with_context(
    document: &Document,
    context: Option<&crate::Dom>,
    root: NodeId,
    writer: &mut impl Write,
) -> Result<(), Error> {
    enum Step {
        Node(NodeId, bool),
        Close(NodeId),
        Abort,
    }
    let mut pending = vec![Step::Node(root, false)];
    while let Some(step) = pending.pop() {
        let (index, literal) = match step {
            Step::Abort => return Ok(()),
            Step::Close(index) => {
                writer.write_all(b"</")?;
                writer.write_all(document.nodes[index].tag.as_bytes())?;
                writer.write_all(b">")?;
                continue;
            }
            Step::Node(index, literal) => (index, literal),
        };
        let node = &document.nodes[index];
        match node.kind {
            Kind::Text => {
                if literal {
                    writer.write_all(node.data.as_bytes())?;
                } else {
                    escape(writer, &node.data, false)?;
                }
            }
            Kind::Comment => {
                writer.write_all(b"<!--")?;
                escape(writer, &node.data, true)?;
                writer.write_all(b"-->")?;
            }
            Kind::Doctype => {
                writer.write_all(b"<!DOCTYPE ")?;
                escape(writer, &node.data, false)?;
                let mut public = "";
                let mut system = "";
                for attribute in &node.attrs {
                    match attribute.key.as_str() {
                        "public" => public = &attribute.value,
                        "system" => system = &attribute.value,
                        _ => {}
                    }
                }
                if !public.is_empty() {
                    writer.write_all(b" PUBLIC ")?;
                    doctype_identifier(writer, public)?;
                    if !system.is_empty() {
                        writer.write_all(b" ")?;
                        doctype_identifier(writer, system)?;
                    }
                } else if !system.is_empty() {
                    writer.write_all(b" SYSTEM ")?;
                    doctype_identifier(writer, system)?;
                }
                writer.write_all(b">")?;
            }
            Kind::Document => pending.extend(
                node.children
                    .iter()
                    .rev()
                    .map(|&child| Step::Node(child, false)),
            ),
            Kind::Element => {
                writer.write_all(b"<")?;
                writer.write_all(node.tag.as_bytes())?;
                for attribute in &node.attrs {
                    writer.write_all(b" ")?;
                    if !attribute.namespace.is_empty() {
                        writer.write_all(attribute.namespace.as_bytes())?;
                        writer.write_all(b":")?;
                    }
                    writer.write_all(attribute.key.as_bytes())?;
                    writer.write_all(b"=\"")?;
                    escape(writer, &attribute.value, false)?;
                    writer.write_all(b"\"")?;
                }
                if matches!(
                    node.tag.as_str(),
                    "area"
                        | "base"
                        | "br"
                        | "col"
                        | "embed"
                        | "hr"
                        | "img"
                        | "input"
                        | "keygen"
                        | "link"
                        | "meta"
                        | "param"
                        | "source"
                        | "track"
                        | "wbr"
                ) {
                    if !node.children.is_empty() {
                        return Err(Error::Html(format!(
                            "html: void element <{}> has child nodes",
                            node.tag
                        )));
                    }
                    writer.write_all(b"/>")?;
                    continue;
                }
                writer.write_all(b">")?;
                if matches!(node.tag.as_str(), "pre" | "listing" | "textarea")
                    && node.children.first().is_some_and(|&child| {
                        document.nodes[child].kind == Kind::Text
                            && document.nodes[child].data.starts_with('\n')
                    })
                {
                    writer.write_all(b"\n")?;
                }
                let literal = literal_children(document, context, index);
                pending.push(if literal && node.tag == "plaintext" {
                    Step::Abort
                } else {
                    Step::Close(index)
                });
                pending.extend(
                    node.children
                        .iter()
                        .rev()
                        .map(|&child| Step::Node(child, literal)),
                );
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct TextBuilder {
    output: String,
    space: Option<char>,
    newlines: u8,
}

impl TextBuilder {
    fn queue_space(&mut self, character: char) {
        if self.space.is_none() {
            self.space = Some(character);
        }
    }

    fn newline(&mut self, mut count: u8, collapse: bool) {
        if collapse {
            if self.newlines >= count {
                return;
            }
            count -= self.newlines;
        }
        self.newlines = self.newlines.wrapping_add(count);
        if collapse && self.output.is_empty() {
            return;
        }
        for _ in 0..count {
            self.output.push('\n');
        }
    }

    fn write(&mut self, text: &str) {
        if self.newlines == 0 {
            if let Some(space) = self.space {
                self.output.push(space);
            }
        }
        self.output.push_str(text);
        self.newlines = 0;
        self.space = None;
    }

    fn tex(&mut self, expression: &str, block: bool) {
        if block {
            self.newline(2, true);
            self.write("$$\n");
        } else {
            self.write("$");
        }
        self.write(expression.trim_matches(is_space));
        if block {
            self.write("\n$$");
            self.newline(2, true);
        } else {
            self.write("$");
        }
    }
}

fn direct_text(document: &Document, root: NodeId) -> String {
    document.nodes[root]
        .children
        .iter()
        .filter_map(|&child| {
            let node = &document.nodes[child];
            (node.kind == Kind::Text).then_some(node.data.as_str())
        })
        .collect()
}

fn annotation(document: &Document, root: NodeId) -> Option<NodeId> {
    document.elements(root).into_iter().find(|&node| {
        document.nodes[node].tag == "annotation"
            && document.nodes[node].attr("encoding") == "application/x-tex"
    })
}

fn latex(document: &Document, root: NodeId) -> String {
    for &child in &document.nodes[root].children {
        let node = &document.nodes[child];
        if node.kind != Kind::Element {
            continue;
        }
        if node.has_attr("data-latex") {
            return node.attr("data-latex").into();
        }
        let found = latex(document, child);
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

pub fn inner_text(document: &Document, root: NodeId) -> String {
    let mut builder = TextBuilder::default();
    let mut pending = vec![(root, false)];
    while let Some((index, mut preformatted)) = pending.pop() {
        let node = &document.nodes[index];
        if node.kind == Kind::Text {
            if preformatted {
                builder.write(&node.data);
            } else {
                let mut word_start = None;
                for (offset, character) in node.data.char_indices() {
                    if is_space(character) {
                        if let Some(start) = word_start.take() {
                            builder.write(&node.data[start..offset]);
                        }
                        builder.queue_space(if character == '\u{a0}' {
                            character
                        } else {
                            ' '
                        });
                    } else if word_start.is_none() {
                        word_start = Some(offset);
                    }
                }
                if let Some(start) = word_start {
                    builder.write(&node.data[start..]);
                }
            }
            continue;
        }
        if node.kind == Kind::Element {
            if node.has_attr("aria-hidden") && matches!(node.attr("aria-hidden"), "" | "true") {
                continue;
            }
            match node.tag.as_str() {
                "head" | "meta" | "style" | "iframe" | "audio" | "video" | "track" | "source"
                | "canvas" | "svg" | "map" | "area" => continue,
                "script" => {
                    let mime = node.attr("type");
                    let (mime, block) =
                        mime.split_once(';').map_or((mime, false), |(mime, rest)| {
                            (mime, rest.contains("mode=display"))
                        });
                    if mime == "math/tex" {
                        builder.tex(&direct_text(document, index), block);
                    }
                    continue;
                }
                "math" => {
                    if let Some(annotation) = annotation(document, index) {
                        builder.tex(
                            &direct_text(document, annotation),
                            node.attr("display") == "block",
                        );
                    }
                    continue;
                }
                "mjx-container" => {
                    let expression = latex(document, index);
                    if !expression.is_empty() {
                        builder.tex(&expression, node.attr("display") == "true");
                    }
                    continue;
                }
                "br" => builder.newline(1, false),
                "hr" | "p" | "blockquote" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul"
                | "ol" | "dl" | "table" => builder.newline(2, true),
                "pre" => {
                    builder.newline(2, true);
                    preformatted = true;
                }
                "th" | "td" => builder.queue_space('\t'),
                "div" | "figure" | "figcaption" | "picture" | "li" | "dt" | "dd" | "header"
                | "footer" | "main" | "section" | "article" | "aside" | "nav" | "address"
                | "details" | "summary" | "dialog" | "form" | "fieldset" | "caption" | "thead"
                | "tbody" | "tfoot" | "tr" => builder.newline(1, true),
                _ => {}
            }
        }
        pending.extend(
            node.children
                .iter()
                .rev()
                .map(|&child| (child, preformatted)),
        );
    }
    builder.output
}
