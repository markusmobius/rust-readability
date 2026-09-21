use super::{doctype, sink, HtmlTreeSink};
use crate::Kind;
use html5ever::{
    interface::QuirksMode,
    tendril::StrTendril,
    tree_builder::{ElementFlags, NodeOrText, TreeSink},
    Attribute, ExpandedName, QualName,
};
use std::{borrow::Cow, cell::RefCell, rc::Rc};

pub trait HtmlTreeStore: HtmlTreeSink {
    fn parent(&self, node: Self::Handle) -> Option<Self::Handle>;
    fn last_child(&self, node: Self::Handle) -> Option<Self::Handle>;
    fn previous_sibling(&self, node: Self::Handle) -> Option<Self::Handle>;
    fn append_text(&mut self, node: Self::Handle, text: &str) -> bool;
    fn append_child(&mut self, parent: Self::Handle, child: Self::Handle);
    fn insert_before(&mut self, sibling: Self::Handle, child: Self::Handle);
    fn detach(&mut self, node: Self::Handle);
    fn reparent_children(&mut self, node: Self::Handle, parent: Self::Handle);
    fn has_attribute(&self, node: Self::Handle, namespace: &str, key: &str) -> bool;
    fn sort_attributes(&mut self, node: Self::Handle);
}

struct ParsedNode<Handle> {
    output: Handle,
    name: Option<QualName>,
    template: Option<Rc<ParsedNode<Handle>>>,
    mathml_integration: bool,
}

impl<Handle> sink::ForeignNode for Rc<ParsedNode<Handle>> {
    fn is_foreign_element(&self) -> bool {
        self.name
            .as_ref()
            .is_some_and(|name| name.ns.as_ref() != "http://www.w3.org/1999/xhtml")
    }
}

struct DirectSink<'output, Output: HtmlTreeStore> {
    output: RefCell<&'output mut Output>,
    document: Rc<ParsedNode<Output::Handle>>,
    declaration: Option<doctype::Declaration>,
    templates: RefCell<Vec<(Output::Handle, Output::Handle)>>,
    formatting: RefCell<Vec<Output::Handle>>,
}

fn node<Handle>(output: Handle) -> Rc<ParsedNode<Handle>> {
    Rc::new(ParsedNode {
        output,
        name: None,
        template: None,
        mathml_integration: false,
    })
}

fn attribute_name<'attribute>(
    name: &'attribute QualName,
    foreign: bool,
) -> (Cow<'attribute, str>, Cow<'attribute, str>) {
    let qualified = name
        .prefix
        .as_ref()
        .filter(|prefix| !prefix.is_empty())
        .map_or_else(
            || Cow::Borrowed(name.local.as_ref()),
            |prefix| Cow::Owned(format!("{prefix}:{}", name.local)),
        );
    if foreign
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
        )
    {
        (
            Cow::Borrowed(name.prefix.as_ref().unwrap().as_ref()),
            Cow::Borrowed(name.local.as_ref()),
        )
    } else {
        (Cow::Borrowed(""), qualified)
    }
}

impl<Output: HtmlTreeStore> TreeSink for DirectSink<'_, Output> {
    type Handle = Rc<ParsedNode<Output::Handle>>;
    type Output = ();
    type ElemName<'node>
        = ExpandedName<'node>
    where
        Self: 'node;

    fn finish(self) {
        let output = self.output.into_inner();
        for (template, contents) in self.templates.into_inner() {
            output.reparent_children(contents, template);
        }
        for element in self.formatting.into_inner() {
            output.sort_attributes(element);
        }
    }

    fn parse_error(&self, _message: Cow<'static, str>) {}

    fn get_document(&self) -> Self::Handle {
        self.document.clone()
    }

    fn elem_name<'node>(&'node self, target: &'node Self::Handle) -> ExpandedName<'node> {
        target.name.as_ref().unwrap().expanded()
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Self::Handle {
        let mut output = self.output.borrow_mut();
        let namespace = match name.ns.as_ref() {
            "http://www.w3.org/1999/xhtml" => "",
            "http://www.w3.org/2000/svg" => "svg",
            "http://www.w3.org/1998/Math/MathML" => "math",
            other => other,
        };
        let element = output.append_node(None, Kind::Element, &name.local, namespace, "");
        for attribute in attrs {
            let (namespace, key) =
                attribute_name(&attribute.name, matches!(namespace, "svg" | "math"));
            output.append_attribute(element, &namespace, &key, &attribute.value);
        }
        if namespace.is_empty()
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
            self.formatting.borrow_mut().push(element);
        }
        let template = flags.template.then(|| {
            let contents = output.append_node(None, Kind::Document, "", "", "");
            self.templates.borrow_mut().push((element, contents));
            node(contents)
        });
        Rc::new(ParsedNode {
            output: element,
            name: Some(name),
            template,
            mathml_integration: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        node(self.output.borrow_mut().append_node(
            None,
            Kind::Comment,
            "",
            "",
            &crate::entities::unescape(&text),
        ))
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
        node(self.output.borrow_mut().append_node(
            None,
            Kind::Comment,
            "",
            "",
            &format!("?{target} {data}?"),
        ))
    }

    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut output = self.output.borrow_mut();
        match child {
            NodeOrText::AppendNode(child) => output.append_child(parent.output, child.output),
            NodeOrText::AppendText(text) => {
                if !output
                    .last_child(parent.output)
                    .is_some_and(|last| output.append_text(last, &text))
                {
                    output.append_node(Some(parent.output), Kind::Text, "", "", &text);
                }
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        previous: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let has_parent = self.output.borrow().parent(element.output).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(previous, child);
        }
    }

    fn append_doctype_to_document(&self, name: StrTendril, public: StrTendril, system: StrTendril) {
        let mut output = self.output.borrow_mut();
        if let Some(declaration) = &self.declaration {
            let element = output.append_node(
                Some(self.document.output),
                Kind::Doctype,
                "",
                "",
                &declaration.name,
            );
            for attribute in &declaration.attrs {
                output.append_attribute(
                    element,
                    &attribute.namespace,
                    &attribute.key,
                    &attribute.value,
                );
            }
        } else {
            let element = output.append_node(
                Some(self.document.output),
                Kind::Doctype,
                "",
                "",
                &crate::entities::unescape(&name),
            );
            for (key, value) in [("public", public), ("system", system)] {
                if !value.is_empty() {
                    output.append_attribute(element, "", key, &crate::entities::unescape(&value));
                }
            }
        }
    }

    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        target.template.as_ref().unwrap().clone()
    }

    fn same_node(&self, first: &Self::Handle, second: &Self::Handle) -> bool {
        Rc::ptr_eq(first, second)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut output = self.output.borrow_mut();
        match child {
            NodeOrText::AppendNode(child) => output.insert_before(sibling.output, child.output),
            NodeOrText::AppendText(text) => {
                if !output
                    .previous_sibling(sibling.output)
                    .is_some_and(|previous| output.append_text(previous, &text))
                {
                    let child = output.append_node(None, Kind::Text, "", "", &text);
                    output.insert_before(sibling.output, child);
                }
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let mut output = self.output.borrow_mut();
        let foreign = target.name.as_ref().is_some_and(|name| {
            matches!(
                name.ns.as_ref(),
                "http://www.w3.org/2000/svg" | "http://www.w3.org/1998/Math/MathML"
            )
        });
        for attribute in attrs {
            let (namespace, key) = attribute_name(&attribute.name, foreign);
            if !output.has_attribute(target.output, &namespace, &key) {
                output.append_attribute(target.output, &namespace, &key, &attribute.value);
            }
        }
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        self.output.borrow_mut().detach(target.output);
    }

    fn reparent_children(&self, node: &Self::Handle, parent: &Self::Handle) {
        self.output
            .borrow_mut()
            .reparent_children(node.output, parent.output);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &Self::Handle) -> bool {
        handle.mathml_integration
    }
}

pub fn parse_html_direct<Output: HtmlTreeStore>(
    source: &str,
    output: &mut Output,
) -> Output::Handle {
    let declaration = doctype::declaration(source);
    let root = output.append_node(None, Kind::Document, "", "", "");
    let direct = DirectSink {
        output: RefCell::new(output),
        document: node(root),
        declaration: declaration.clone(),
        templates: RefCell::new(Vec::new()),
        formatting: RefCell::new(Vec::new()),
    };
    sink::parse_dom_into(source, direct, declaration);
    root
}
