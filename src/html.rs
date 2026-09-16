use crate::{dom::Tree, sort::sort_by, Attribute, Document, Kind, Node, Text};
use markup5ever_rcdom::NodeData;

mod doctype;
mod sink;

pub fn parse_html(source: &str) -> Document {
    parse_dom(source).document
}

pub fn parse_dom(source: &str) -> Tree {
    let (parsed, declaration) = sink::parse(source);
    let mut nodes: Vec<Node> = Vec::new();
    let mut namespaces = Vec::new();
    let mut pending = vec![(parsed.document.clone(), None)];
    while let Some((handle, parent)) = pending.pop() {
        let mut node = Node::new(Kind::Document);
        node.parent = parent;
        let mut namespace = Text::new();
        let mut children = handle.children.borrow().clone();
        match &handle.data {
            NodeData::Document => {}
            NodeData::Element {
                name,
                attrs,
                template_contents,
                ..
            } => {
                node.kind = Kind::Element;
                node.tag = name.local.as_ref().into();
                namespace = match name.ns.as_ref() {
                    "http://www.w3.org/1999/xhtml" => "",
                    "http://www.w3.org/2000/svg" => "svg",
                    "http://www.w3.org/1998/Math/MathML" => "math",
                    other => other,
                }
                .into();
                node.attrs = attrs
                    .borrow()
                    .iter()
                    .map(|attribute| {
                        let qualified = attribute
                            .name
                            .prefix
                            .as_ref()
                            .filter(|prefix| !prefix.is_empty())
                            .map_or_else(
                                || attribute.name.local.to_string(),
                                |prefix| format!("{prefix}:{}", attribute.name.local),
                            );
                        let (namespace, key) = if matches!(
                            name.ns.as_ref(),
                            "http://www.w3.org/2000/svg" | "http://www.w3.org/1998/Math/MathML"
                        ) && matches!(
                            qualified.as_str(),
                            "xlink:actuate"
                                | "xlink:arcrole"
                                | "xlink:href"
                                | "xlink:role"
                                | "xlink:show"
                                | "xlink:title"
                                | "xlink:type"
                                | "xml:lang"
                                | "xml:space"
                                | "xmlns:xlink"
                        ) {
                            qualified.split_once(':').unwrap()
                        } else {
                            ("", qualified.as_str())
                        };
                        Attribute {
                            namespace: namespace.into(),
                            key: key.into(),
                            value: attribute.value.as_ref().into(),
                        }
                    })
                    .collect();
                if name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
                    && matches!(
                        node.tag.as_str(),
                        "a" | "b"
                            | "big"
                            | "code"
                            | "em"
                            | "font"
                            | "i"
                            | "nobr"
                            | "s"
                            | "small"
                            | "strike"
                            | "strong"
                            | "tt"
                            | "u"
                    )
                {
                    sort_by(&mut node.attrs, |first, second| {
                        (&first.namespace, &first.key, &first.value)
                            < (&second.namespace, &second.key, &second.value)
                    });
                }
                if let Some(template) = template_contents.borrow().as_ref() {
                    children.extend(template.children.borrow().iter().cloned());
                }
            }
            NodeData::Text { contents } => {
                node.kind = Kind::Text;
                node.data = contents.borrow().as_ref().into();
            }
            NodeData::Comment { contents } => {
                node.kind = Kind::Comment;
                node.data = crate::entities::unescape(contents).into();
            }
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => {
                node.kind = Kind::Doctype;
                if let Some(declaration) = &declaration {
                    node.data = declaration.name.clone().into();
                    node.attrs = declaration.attrs.clone();
                } else {
                    node.data = crate::entities::unescape(name).into();
                    for (key, value) in [("public", public_id), ("system", system_id)] {
                        if !value.is_empty() {
                            node.attrs.push(Attribute {
                                namespace: Text::new(),
                                key: key.into(),
                                value: crate::entities::unescape(value).into(),
                            });
                        }
                    }
                }
            }
            NodeData::ProcessingInstruction { target, contents } => {
                node.kind = Kind::Comment;
                node.data = format!("?{target} {contents}?").into();
            }
        }
        let index = nodes.len();
        nodes.push(node);
        namespaces.push(namespace);
        if let Some(parent) = parent {
            nodes[parent].children.push(index);
        }
        pending.extend(children.into_iter().rev().map(|child| (child, Some(index))));
    }
    let mut tree = Tree::new(Document { nodes });
    tree.namespaces = namespaces;
    tree
}

#[cfg(test)]
mod tests {
    use super::parse_html;

    #[test]
    fn go_formatting_attribute_order() {
        let document = parse_html("<p z='1' a='2'><a target='_blank' rel='noopener' href='/x'>link</a></p><svg><a z='1' a='2'>foreign</a></svg>");
        let anchors = document.tagged(0, "a");
        assert_eq!(
            document.nodes[anchors[0]]
                .attrs
                .iter()
                .map(|attr| attr.key.as_str())
                .collect::<Vec<_>>(),
            ["href", "rel", "target"]
        );
        assert_eq!(
            document.nodes[anchors[1]]
                .attrs
                .iter()
                .map(|attr| attr.key.as_str())
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
        let paragraph = document.tagged(0, "p")[0];
        assert_eq!(
            document.nodes[paragraph]
                .attrs
                .iter()
                .map(|attr| attr.key.as_str())
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
    }
}
