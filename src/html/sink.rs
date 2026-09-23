use super::doctype::{self, Declaration};
use super::tokenizer::Tokenizer;
use html5ever::{
    buffer_queue::BufferQueue,
    interface::QuirksMode,
    tendril::StrTendril,
    tokenizer::{Doctype, TagKind, Token, TokenSink, TokenSinkResult},
    tree_builder::{ElementFlags, NodeOrText, Tracer, TreeBuilder, TreeSink},
    Attribute, QualName, TokenizerResult,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::{borrow::Cow, cell::Cell, marker::PhantomData};

#[derive(Default)]
struct Sink<Dom: TreeSink = RcDom> {
    dom: Dom,
    foreign: Cell<bool>,
    halted: Cell<bool>,
    declaration: Option<Declaration>,
}

impl<Dom: TreeSink> TreeSink for Sink<Dom> {
    type Handle = Dom::Handle;
    type Output = (Dom::Output, Option<Declaration>);
    type ElemName<'node>
        = Dom::ElemName<'node>
    where
        Self: 'node;

    fn finish(self) -> Self::Output {
        (self.dom.finish(), self.declaration)
    }
    fn parse_error(&self, message: Cow<'static, str>) {
        self.dom.parse_error(message);
    }
    fn get_document(&self) -> Self::Handle {
        self.dom.get_document()
    }
    fn elem_name<'node>(&'node self, target: &'node Self::Handle) -> Self::ElemName<'node> {
        self.dom.elem_name(target)
    }
    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Self::Handle {
        if name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
            && name.local.as_ref() == "template"
            && self.foreign.get()
        {
            self.halted.set(true);
        }
        self.dom.create_element(name, attrs, flags)
    }
    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        self.dom.create_comment(text)
    }
    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
        self.dom.create_pi(target, data)
    }
    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        if !self.halted.get() {
            self.dom.append(parent, child);
        }
    }
    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        previous: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        if !self.halted.get() {
            self.dom
                .append_based_on_parent_node(element, previous, child);
        }
    }
    fn append_doctype_to_document(&self, name: StrTendril, public: StrTendril, system: StrTendril) {
        self.dom.append_doctype_to_document(name, public, system);
    }
    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        self.dom.get_template_contents(target)
    }
    fn same_node(&self, first: &Self::Handle, second: &Self::Handle) -> bool {
        self.dom.same_node(first, second)
    }
    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.dom.set_quirks_mode(mode);
    }
    fn append_before_sibling(&self, sibling: &Self::Handle, child: NodeOrText<Self::Handle>) {
        if !self.halted.get() {
            self.dom.append_before_sibling(sibling, child);
        }
    }
    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
        self.dom.add_attrs_if_missing(target, attrs);
    }
    fn remove_from_parent(&self, target: &Self::Handle) {
        self.dom.remove_from_parent(target);
    }
    fn reparent_children(&self, node: &Self::Handle, parent: &Self::Handle) {
        self.dom.reparent_children(node, parent);
    }
    fn is_mathml_annotation_xml_integration_point(&self, handle: &Self::Handle) -> bool {
        self.dom.is_mathml_annotation_xml_integration_point(handle)
    }
}

pub(super) trait ForeignNode {
    fn is_foreign_element(&self) -> bool;
}

impl ForeignNode for Handle {
    fn is_foreign_element(&self) -> bool {
        matches!(&self.data, NodeData::Element { name, .. } if name.ns.as_ref() != "http://www.w3.org/1999/xhtml")
    }
}

struct Foreign<Node>(Cell<bool>, PhantomData<Node>);

impl<Node: ForeignNode> Tracer for Foreign<Node> {
    type Handle = Node;
    fn trace_handle(&self, node: &Node) {
        if node.is_foreign_element() {
            self.0.set(true);
        }
    }
}

struct Builder<Dom: TreeSink = RcDom>(TreeBuilder<Dom::Handle, Sink<Dom>>);

impl<Dom: TreeSink> TokenSink for Builder<Dom>
where
    Dom::Handle: ForeignNode,
{
    type Handle = Dom::Handle;

    fn process_token(&self, token: Token, line: u64) -> TokenSinkResult<Self::Handle> {
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
        let foreign = Foreign(Cell::new(false), PhantomData::<Dom::Handle>);
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
    parse_dom_into(source, RcDom::default(), doctype::declaration(source))
}

pub(super) fn parse_dom_into<Dom: TreeSink>(
    source: &str,
    dom: Dom,
    declaration: Option<Declaration>,
) -> (Dom::Output, Option<Declaration>)
where
    Dom::Handle: ForeignNode,
{
    let sink = Sink {
        dom,
        declaration,
        foreign: Cell::new(false),
        halted: Cell::new(false),
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
