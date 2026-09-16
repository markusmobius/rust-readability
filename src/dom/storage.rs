use smol_str::SmolStr;
use std::{
    borrow::Borrow,
    cell::Cell,
    cmp::Ordering,
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Clone, Default)]
pub struct Text(SmolStr);

impl Text {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn push_str(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        let mut updated = String::with_capacity(self.len() + value.len());
        updated.push_str(self.as_str());
        updated.push_str(value);
        *self = Self::from(updated);
    }
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Text {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self(SmolStr::new(value))
    }
}

impl From<String> for Text {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<Text> for String {
    fn from(value: Text) -> Self {
        value.as_str().to_owned()
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), formatter)
    }
}

impl<Other: AsRef<str> + ?Sized> PartialEq<Other> for Text {
    fn eq(&self, other: &Other) -> bool {
        self.as_str() == other.as_ref()
    }
}

impl Eq for Text {}

impl PartialOrd for Text {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Text {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for Text {
    fn hash<Hasher: std::hash::Hasher>(&self, state: &mut Hasher) {
        self.as_str().hash(state);
    }
}

impl serde::Serialize for Text {
    fn serialize<Serializer: serde::Serializer>(
        &self,
        serializer: Serializer,
    ) -> Result<Serializer::Ok, Serializer::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Document,
    Element,
    Text,
    Comment,
    Doctype,
}

pub type NodeId = usize;

#[derive(Clone, Default)]
pub struct Children(Option<Arc<Vec<NodeId>>>);

impl PartialEq for Children {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for Children {}

impl Children {
    pub fn clear(&mut self) {
        self.0 = None;
    }
}

impl Deref for Children {
    type Target = Vec<NodeId>;

    fn deref(&self) -> &Vec<NodeId> {
        static EMPTY: Vec<NodeId> = Vec::new();
        self.0.as_deref().unwrap_or(&EMPTY)
    }
}

impl DerefMut for Children {
    fn deref_mut(&mut self) -> &mut Vec<NodeId> {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new())))
    }
}

impl fmt::Debug for Children {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), formatter)
    }
}

impl IntoIterator for Children {
    type Item = NodeId;
    type IntoIter = std::vec::IntoIter<NodeId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0
            .map_or_else(Vec::new, |children| {
                Arc::try_unwrap(children).unwrap_or_else(|shared| (*shared).clone())
            })
            .into_iter()
    }
}

impl<'children> IntoIterator for &'children Children {
    type Item = &'children NodeId;
    type IntoIter = std::slice::Iter<'children, NodeId>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attribute {
    pub namespace: Text,
    pub key: Text,
    pub value: Text,
}

#[derive(Debug)]
pub struct Node {
    content: Arc<NodeData>,
    pub kind: Kind,
    pub tag: Text,
    pub parent: Option<NodeId>,
    pub children: Children,
    text_summary: Cell<u64>,
    score: Cell<u64>,
}

impl Clone for Node {
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            kind: self.kind,
            tag: self.tag.clone(),
            parent: self.parent,
            children: self.children.clone(),
            text_summary: Cell::new(u64::MAX),
            score: Cell::new(u64::MAX),
        }
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.tag == other.tag
            && self.content == other.content
            && self.parent == other.parent
            && self.children == other.children
    }
}

impl Eq for Node {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeData {
    pub data: Text,
    pub attrs: Vec<Attribute>,
}

impl Deref for Node {
    type Target = NodeData;

    fn deref(&self) -> &NodeData {
        &self.content
    }
}

impl DerefMut for Node {
    fn deref_mut(&mut self) -> &mut NodeData {
        *self.text_summary.get_mut() = u64::MAX;
        *self.score.get_mut() = u64::MAX;
        Arc::make_mut(&mut self.content)
    }
}

impl Node {
    pub fn new(kind: Kind) -> Self {
        Self {
            content: Arc::new(NodeData {
                data: Text::new(),
                attrs: Vec::new(),
            }),
            kind,
            tag: Text::new(),
            parent: None,
            children: Children::default(),
            text_summary: Cell::new(u64::MAX),
            score: Cell::new(u64::MAX),
        }
    }

    pub(crate) fn text_summary(&self) -> crate::score::TextSummary {
        let cached = self.text_summary.get();
        if cached != u64::MAX {
            return crate::score::TextSummary::from_cache_bits(cached);
        }
        let summary = crate::score::TextSummary::new(&self.data);
        if let Some(bits) = summary.cache_bits() {
            self.text_summary.set(bits);
        }
        summary
    }

    pub(crate) fn score(&self) -> f64 {
        let cached = self.score.get();
        if cached != u64::MAX {
            return f64::from_bits(cached);
        }
        let value = self
            .attr("data-readability-score")
            .trim_matches(crate::check::is_space)
            .parse::<f64>()
            .unwrap_or(0.0);
        self.score.set(value.to_bits());
        value
    }

    pub fn attr(&self, name: &str) -> &str {
        self.attrs
            .iter()
            .find(|attribute| attribute.key == name)
            .map_or("", |attribute| attribute.value.as_str())
    }

    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|attribute| attribute.key == name)
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        if let Some(attribute) = self
            .attrs
            .iter_mut()
            .find(|attribute| attribute.key == name)
        {
            attribute.value = value.into();
        } else {
            self.attrs.push(Attribute {
                namespace: Text::new(),
                key: name.into(),
                value: value.into(),
            });
        }
    }

    pub fn remove_attr(&mut self, name: &str) {
        if let Some(index) = self
            .attrs
            .iter()
            .position(|attribute| attribute.key == name)
        {
            self.attrs.remove(index);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub nodes: Vec<Node>,
}

impl Document {
    pub fn parse(source: &str) -> Self {
        crate::parse_html(source)
    }

    pub fn outer_html(&self, root: NodeId) -> String {
        let mut output = Vec::new();
        let _ = crate::render::html(self, root, &mut output);
        String::from_utf8(output).unwrap()
    }

    pub fn to_html(&self) -> String {
        self.outer_html(0)
    }

    pub fn elements(&self, root: NodeId) -> Vec<NodeId> {
        self.find_descendants(root, |node| node.kind == Kind::Element)
    }

    pub fn descendants(&self, root: NodeId) -> Vec<NodeId> {
        self.find_descendants(root, |_| true)
    }

    fn find_descendants(&self, root: NodeId, matches: impl Fn(&Node) -> bool) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut pending: Vec<_> = self.nodes[root].children.iter().rev().copied().collect();
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if matches(node) {
                result.push(index);
            }
            pending.extend(node.children.iter().rev());
        }
        result
    }

    pub fn tagged(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        self.find_descendants(root, |node| node.kind == Kind::Element && node.tag == tag)
    }

    pub(crate) fn find_element(
        &self,
        root: NodeId,
        matches: impl Fn(&Node) -> bool,
    ) -> Option<NodeId> {
        let mut pending: Vec<_> = self.nodes[root].children.iter().rev().copied().collect();
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if node.kind == Kind::Element && matches(node) {
                return Some(index);
            }
            pending.extend(node.children.iter().rev());
        }
        None
    }

    pub fn text(&self, root: NodeId) -> String {
        let mut output = String::new();
        let mut pending = vec![root];
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if node.kind == Kind::Text {
                output.push_str(&node.data);
            }
            pending.extend(node.children.iter().rev());
        }
        output
    }

    pub fn parent_element(&self, index: NodeId) -> Option<NodeId> {
        self.nodes[index]
            .parent
            .filter(|&parent| self.nodes[parent].kind == Kind::Element)
    }

    pub fn has_ancestor(&self, index: NodeId, tags: &[&str]) -> bool {
        let mut current = self.parent_element(index);
        while let Some(parent) = current {
            if tags.contains(&self.nodes[parent].tag.as_str()) {
                return true;
            }
            current = self.parent_element(parent);
        }
        false
    }

    pub fn contains(&self, ancestor: Option<NodeId>, node: Option<NodeId>) -> bool {
        let (Some(ancestor), Some(mut current)) = (ancestor, node) else {
            return false;
        };
        loop {
            if ancestor == current {
                return true;
            }
            let Some(parent) = self.nodes[current].parent else {
                return false;
            };
            current = parent;
        }
    }

    pub fn detach(&mut self, index: NodeId) {
        if let Some(parent) = self.nodes[index].parent.take() {
            self.nodes[parent].children.retain(|child| *child != index);
        }
    }

    pub fn append(&mut self, parent: NodeId, child: NodeId) {
        self.detach(child);
        self.nodes[parent].children.push(child);
        self.nodes[child].parent = Some(parent);
    }

    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let index = self.nodes.len();
        let mut node = Node::new(Kind::Element);
        node.tag = tag.into();
        self.nodes.push(node);
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloning_copies_dom_value_not_lazy_caches() {
        let mut source = Node::new(Kind::Text);
        source.data = "  words, separated by spaces  ".into();
        source.set_attr("data-readability-score", "1.2345");
        source.text_summary();
        source.score();
        let cloned = source.clone();
        assert_eq!(cloned.text_summary.get(), u64::MAX);
        assert_eq!(cloned.score.get(), u64::MAX);
        assert_eq!(cloned, source);
        assert_eq!(cloned.text_summary().commas, source.text_summary().commas);
        assert_eq!(cloned.score(), source.score());
    }

    #[test]
    fn cached_scores_follow_attribute_updates_and_clones() {
        let mut node = Node::new(Kind::Element);
        for value in [
            "0.3333",
            "1.2345",
            "-0.0000",
            "\u{a0}2.5\u{a0}",
            "+Inf",
            "-Inf",
            "NaN",
            "invalid",
            "",
        ] {
            node.set_attr("data-readability-score", value);
            let expected = value
                .trim_matches(crate::check::is_space)
                .parse::<f64>()
                .unwrap_or(0.0);
            let actual = node.score();
            assert!(actual.to_bits() == expected.to_bits() || actual.is_nan() && expected.is_nan());
            let mut cloned = node.clone();
            cloned.attrs[0].value = "42.1250".into();
            assert_eq!(cloned.score(), 42.125);
            let unchanged = node.score();
            assert!(
                unchanged.to_bits() == expected.to_bits()
                    || unchanged.is_nan() && expected.is_nan()
            );
        }
        node.remove_attr("data-readability-score");
        assert_eq!(node.score(), 0.0);
    }

    #[test]
    fn single_owner_caches_preserve_send_and_values() {
        fn transferable<Value: Send>() {}
        transferable::<crate::Parser>();
        transferable::<crate::Dom>();
        transferable::<crate::Article>();
        let mut node = Node::new(Kind::Element);
        node.data = "  an article, with details  ".into();
        node.set_attr("data-readability-score", "-0.0000");
        let expected = crate::score::TextSummary::new(&node.data).cache_bits();
        for _ in 0..100 {
            assert_eq!(node.score().to_bits(), (-0.0f64).to_bits());
            assert_eq!(node.text_summary().cache_bits(), expected);
        }
        node.data = "updated, text, values".into();
        node.set_attr("data-readability-score", "1.2345");
        assert_eq!(node.score(), 1.2345);
        assert_eq!(node.text_summary().commas, 2);
    }

    #[test]
    fn cached_text_statistics_follow_mutations_not_clone_history() {
        let mut original = Node::new(Kind::Text);
        original.data = "first, value".into();
        let mut cloned = original.clone();
        assert_eq!(cloned.text_summary().commas, 1);
        assert_eq!(original.text_summary.get(), u64::MAX);
        assert_eq!(original, cloned);
        cloned.data = "another, value, with commas".into();
        assert_eq!(cloned.text_summary().commas, 2);
        assert_eq!(original.text_summary().commas, 1);
        cloned.data = "first, value".into();
        assert_eq!(original, cloned);
    }

    #[test]
    fn arena_mutation_preserves_shared_strings_and_order() {
        let empty = Children::default();
        let mut allocated_empty = empty.clone();
        allocated_empty.push(0);
        allocated_empty.pop();
        assert_eq!(empty, allocated_empty);
        let mut original = Document { nodes: Vec::new() };
        let root = original.create_element("root");
        let first = original.create_element("section");
        let second = original.create_element("article");
        original.append(root, first);
        original.append(root, second);
        original.nodes[first].set_attr("class", "original attribute value with shared storage");
        original.nodes[first].set_attr("class", "updated attribute value with shared storage");
        let mut cloned = original.clone();
        assert!(Arc::ptr_eq(
            &cloned.nodes[first].content,
            &original.nodes[first].content
        ));
        assert!(Arc::ptr_eq(
            cloned.nodes[root].children.0.as_ref().unwrap(),
            original.nodes[root].children.0.as_ref().unwrap()
        ));
        assert_eq!(cloned.nodes[first].attrs.len(), 1);
        assert_eq!(
            cloned.nodes[first].attrs[0].value.as_ptr(),
            original.nodes[first].attrs[0].value.as_ptr()
        );
        cloned.nodes[first].set_attr("class", "independent");
        cloned.append(root, first);
        assert!(!Arc::ptr_eq(
            cloned.nodes[root].children.0.as_ref().unwrap(),
            original.nodes[root].children.0.as_ref().unwrap()
        ));
        assert_eq!(cloned.elements(root), [second, first]);
        assert_eq!(original.elements(root), [first, second]);
        assert_eq!(
            original.nodes[first].attr("class"),
            "updated attribute value with shared storage"
        );
        assert_eq!(cloned.nodes[first].attr("class"), "independent");
        cloned.nodes[first].remove_attr("class");
        assert!(!cloned.nodes[first].has_attr("class"));
        assert!(original.nodes[first].has_attr("class"));
    }

    #[test]
    fn text_clones_share_storage_until_mutation() {
        fn thread_safe<Value: Send + Sync>() {}
        thread_safe::<Text>();
        for value in [
            "",
            "article",
            "A longer article sentence, with enough text to use shared storage.",
        ] {
            let original = Text::from(value);
            let mut cloned = original.clone();
            assert_eq!(original, cloned);
            if value.len() > 16 {
                assert_eq!(original.as_ptr(), cloned.as_ptr());
            }
            cloned.push_str(" added");
            assert_eq!(original, value);
            assert_eq!(cloned, format!("{value} added"));
            cloned.clear();
            assert!(cloned.is_empty());
            assert_eq!(String::from(original), value);
        }
    }
}
