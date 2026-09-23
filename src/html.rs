use crate::{dom::Tree, sort::sort_by, Attribute, Document, Kind, Node, Text};
use markup5ever_rcdom::NodeData;
use std::borrow::Cow;

mod direct;
mod doctype;
mod sink;
#[allow(dead_code, unused_imports, unused_parens)]
mod tokenizer;

pub use direct::{parse_html_direct, HtmlTreeStore};

pub trait HtmlTreeSink {
    type Handle: Copy;

    fn append_node(
        &mut self,
        parent: Option<Self::Handle>,
        kind: Kind,
        tag: &str,
        namespace: &str,
        data: &str,
    ) -> Self::Handle;

    fn append_attribute(&mut self, node: Self::Handle, namespace: &str, key: &str, value: &str);
}

#[derive(Default)]
struct DocumentSink {
    nodes: Vec<Node>,
    namespaces: Vec<Text>,
}

impl HtmlTreeSink for DocumentSink {
    type Handle = usize;

    fn append_node(
        &mut self,
        parent: Option<Self::Handle>,
        kind: Kind,
        tag: &str,
        namespace: &str,
        data: &str,
    ) -> Self::Handle {
        let mut node = Node::new(kind);
        node.parent = parent;
        node.tag = tag.into();
        node.data = data.into();
        let index = self.nodes.len();
        self.nodes.push(node);
        self.namespaces.push(namespace.into());
        if let Some(parent) = parent {
            self.nodes[parent].children.push(index);
        }
        index
    }

    fn append_attribute(&mut self, node: Self::Handle, namespace: &str, key: &str, value: &str) {
        self.nodes[node].attrs.push(Attribute {
            namespace: namespace.into(),
            key: key.into(),
            value: value.into(),
        });
    }
}

pub fn parse_html(source: &str) -> Document {
    parse_dom(source).document
}

pub fn parse_dom(source: &str) -> Tree {
    let mut output = DocumentSink::default();
    parse_html_into(source, &mut output);
    let mut tree = Tree::new(Document {
        nodes: output.nodes,
    });
    tree.namespaces = output.namespaces;
    tree
}

pub fn parse_html_into<Output: HtmlTreeSink>(source: &str, output: &mut Output) -> Output::Handle {
    let (parsed, declaration) = sink::parse(source);
    let mut root = None;
    let mut pending = vec![(parsed.document.clone(), None)];
    while let Some((handle, parent)) = pending.pop() {
        let mut children = std::mem::take(&mut *handle.children.borrow_mut());
        let index = match &handle.data {
            NodeData::Document => output.append_node(parent, Kind::Document, "", "", ""),
            NodeData::Element {
                name,
                attrs,
                template_contents,
                ..
            } => {
                let namespace = match name.ns.as_ref() {
                    "http://www.w3.org/1999/xhtml" => "",
                    "http://www.w3.org/2000/svg" => "svg",
                    "http://www.w3.org/1998/Math/MathML" => "math",
                    other => other,
                };
                let index = output.append_node(parent, Kind::Element, &name.local, namespace, "");
                let attrs = attrs.borrow();
                let attributes = attrs.iter().map(|attribute| {
                    let qualified = attribute
                        .name
                        .prefix
                        .as_ref()
                        .filter(|prefix| !prefix.is_empty())
                        .map_or_else(
                            || Cow::Borrowed(attribute.name.local.as_ref()),
                            |prefix| Cow::Owned(format!("{prefix}:{}", attribute.name.local)),
                        );
                    (qualified, attribute.value.as_ref())
                });
                let append_attribute = |(qualified, value): (Cow<'_, str>, &str)| {
                    let (namespace, key) = if matches!(namespace, "svg" | "math")
                        && matches!(
                            qualified.as_ref(),
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
                        ("", qualified.as_ref())
                    };
                    output.append_attribute(index, namespace, key, value);
                };
                if attrs.len() > 1
                    && name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
                    && matches!(
                        name.local.as_ref(),
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
                    let mut attributes: Vec<_> = attributes.collect();
                    sort_by(&mut attributes, |first, second| first < second);
                    attributes.into_iter().for_each(append_attribute);
                } else {
                    attributes.for_each(append_attribute);
                }
                if let Some(template) = template_contents.borrow().as_ref() {
                    children.append(&mut template.children.borrow_mut());
                }
                index
            }
            NodeData::Text { contents } => {
                output.append_node(parent, Kind::Text, "", "", &contents.borrow())
            }
            NodeData::Comment { contents } => output.append_node(
                parent,
                Kind::Comment,
                "",
                "",
                &crate::entities::unescape(contents),
            ),
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => {
                if let Some(declaration) = &declaration {
                    let index =
                        output.append_node(parent, Kind::Doctype, "", "", &declaration.name);
                    for attribute in &declaration.attrs {
                        output.append_attribute(
                            index,
                            &attribute.namespace,
                            &attribute.key,
                            &attribute.value,
                        );
                    }
                    index
                } else {
                    let index = output.append_node(
                        parent,
                        Kind::Doctype,
                        "",
                        "",
                        &crate::entities::unescape(name),
                    );
                    for (key, value) in [("public", public_id), ("system", system_id)] {
                        if !value.is_empty() {
                            output.append_attribute(
                                index,
                                "",
                                key,
                                &crate::entities::unescape(value),
                            );
                        }
                    }
                    index
                }
            }
            NodeData::ProcessingInstruction { target, contents } => output.append_node(
                parent,
                Kind::Comment,
                "",
                "",
                &format!("?{target} {contents}?"),
            ),
        };
        root.get_or_insert(index);
        pending.extend(children.into_iter().rev().map(|child| (child, Some(index))));
    }
    root.unwrap()
}

#[cfg(test)]
mod tests {
    use super::{parse_dom, parse_html, parse_html_into, DocumentSink, HtmlTreeSink};
    use crate::Kind;
    use std::num::NonZeroUsize;

    #[test]
    fn output_sink_preserves_nodes_with_custom_handles() {
        #[derive(Default)]
        struct Output(DocumentSink);

        impl HtmlTreeSink for Output {
            type Handle = NonZeroUsize;

            fn append_node(
                &mut self,
                parent: Option<Self::Handle>,
                kind: Kind,
                tag: &str,
                namespace: &str,
                data: &str,
            ) -> Self::Handle {
                let index = self.0.append_node(
                    parent.map(|handle| handle.get() - 1),
                    kind,
                    tag,
                    namespace,
                    data,
                );
                NonZeroUsize::new(index + 1).unwrap()
            }

            fn append_attribute(
                &mut self,
                node: Self::Handle,
                namespace: &str,
                key: &str,
                value: &str,
            ) {
                self.0
                    .append_attribute(node.get() - 1, namespace, key, value);
            }
        }

        for source in [
            "",
            "<!DOCTYPE html PUBLIC 'public' 'system'><!--&amp;--><p><a z='1' a='2' href='/x'>words</a>tail",
            "<template><p>template</p></template><svg><a xml:lang='fr' xlink:href='/x'>foreign</a></svg><math><mi>x</mi></math>",
            "<table>fostered<p><b title='bold'>text</table>tail</b><svg><template>stopped</template></svg>",
        ] {
            let mut output = Output::default();
            let root = parse_html_into(source, &mut output);
            let expected = parse_dom(source);
            assert_eq!(root.get(), 1);
            assert_eq!(output.0.nodes, expected.document.nodes);
            assert_eq!(output.0.namespaces, expected.namespaces);
        }
    }

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
