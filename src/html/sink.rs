use super::doctype::{self, Declaration};
use html5ever::{
    buffer_queue::BufferQueue,
    interface::QuirksMode,
    tendril::StrTendril,
    tokenizer::{Doctype, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer},
    tree_builder::{ElementFlags, NodeOrText, Tracer, TreeBuilder, TreeSink},
    Attribute, ExpandedName, QualName, TokenizerResult,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::{borrow::Cow, cell::Cell};

#[derive(Default)]
struct Sink {
    dom: RcDom,
    foreign: Cell<bool>,
    halted: Cell<bool>,
    declaration: Option<Declaration>,
}

impl TreeSink for Sink {
    type Handle = Handle;
    type Output = (RcDom, Option<Declaration>);
    type ElemName<'node> = ExpandedName<'node>;

    fn finish(self) -> Self::Output {
        (self.dom, self.declaration)
    }
    fn parse_error(&self, message: Cow<'static, str>) {
        self.dom.parse_error(message);
    }
    fn get_document(&self) -> Handle {
        self.dom.get_document()
    }
    fn elem_name<'node>(&'node self, target: &'node Handle) -> ExpandedName<'node> {
        self.dom.elem_name(target)
    }
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        if name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
            && name.local.as_ref() == "template"
            && self.foreign.get()
        {
            self.halted.set(true);
        }
        self.dom.create_element(name, attrs, flags)
    }
    fn create_comment(&self, text: StrTendril) -> Handle {
        self.dom.create_comment(text)
    }
    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        self.dom.create_pi(target, data)
    }
    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        if !self.halted.get() {
            self.dom.append(parent, child);
        }
    }
    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        previous: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if !self.halted.get() {
            self.dom
                .append_based_on_parent_node(element, previous, child);
        }
    }
    fn append_doctype_to_document(&self, name: StrTendril, public: StrTendril, system: StrTendril) {
        self.dom.append_doctype_to_document(name, public, system);
    }
    fn get_template_contents(&self, target: &Handle) -> Handle {
        self.dom.get_template_contents(target)
    }
    fn same_node(&self, first: &Handle, second: &Handle) -> bool {
        self.dom.same_node(first, second)
    }
    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.dom.set_quirks_mode(mode);
    }
    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        if !self.halted.get() {
            self.dom.append_before_sibling(sibling, child);
        }
    }
    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        self.dom.add_attrs_if_missing(target, attrs);
    }
    fn remove_from_parent(&self, target: &Handle) {
        self.dom.remove_from_parent(target);
    }
    fn reparent_children(&self, node: &Handle, parent: &Handle) {
        self.dom.reparent_children(node, parent);
    }
    fn is_mathml_annotation_xml_integration_point(&self, handle: &Handle) -> bool {
        self.dom.is_mathml_annotation_xml_integration_point(handle)
    }
}

#[derive(Default)]
struct Foreign(Cell<bool>);

impl Tracer for Foreign {
    type Handle = Handle;
    fn trace_handle(&self, node: &Handle) {
        if let NodeData::Element { name, .. } = &node.data {
            if name.ns.as_ref() != "http://www.w3.org/1999/xhtml" {
                self.0.set(true);
            }
        }
    }
}

struct Builder(TreeBuilder<Handle, Sink>);

impl TokenSink for Builder {
    type Handle = Handle;

    fn process_token(&self, token: Token, line: u64) -> TokenSinkResult<Handle> {
        if self.0.sink.halted.get() {
            return TokenSinkResult::Continue;
        }
        let token = if matches!(token, Token::DoctypeToken(_)) {
            self.0
                .sink
                .declaration
                .as_ref()
                .map_or(token, |declaration| {
                    Token::DoctypeToken(Doctype {
                        name: Some("html".into()),
                        public_id: None,
                        system_id: None,
                        force_quirks: declaration.quirks,
                    })
                })
        } else {
            token
        };
        let foreign = Foreign::default();
        if matches!(&token, Token::TagToken(tag) if tag.kind == TagKind::StartTag && tag.name.as_ref() == "template")
        {
            self.0.trace_handles(&foreign);
        }
        self.0.sink.foreign.set(foreign.0.get());
        self.0.process_token(token, line)
    }

    fn end(&self) {
        if !self.0.sink.halted.get() {
            self.0.end();
        }
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.0
            .adjusted_current_node_present_but_not_in_html_namespace()
    }
}

pub(super) fn parse(source: &str) -> (RcDom, Option<Declaration>) {
    let sink = Sink {
        declaration: doctype::declaration(source),
        ..Sink::default()
    };
    let tokenizer = Tokenizer::new(
        Builder(TreeBuilder::new(sink, Default::default())),
        Default::default(),
    );
    let input = BufferQueue::default();
    input.push_back(source.into());
    while !matches!(tokenizer.feed(&input), TokenizerResult::Done) {}
    tokenizer.end();
    tokenizer.sink.0.sink.finish()
}
