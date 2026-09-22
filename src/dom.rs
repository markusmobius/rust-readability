use crate::check::is_space;
use regex::Regex;
use std::{
    ops::{Deref, DerefMut},
    sync::LazyLock,
};

mod storage;
pub use storage::{Attribute, Children, Document, Kind, Node, NodeData, NodeId, Text};

static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\t\n\f\r ]{2,}").unwrap());
static ATOMS: LazyLock<Vec<String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("html-atoms.json")).unwrap());

pub(crate) fn original_atom(tag: &str) -> Text {
    if ATOMS
        .binary_search_by(|atom| atom.as_str().cmp(tag))
        .is_ok()
    {
        tag.into()
    } else {
        Text::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub document: Document,
    pub atoms: Vec<Text>,
    pub namespaces: Vec<Text>,
}

impl Deref for Tree {
    type Target = Document;
    fn deref(&self) -> &Document {
        &self.document
    }
}

impl DerefMut for Tree {
    fn deref_mut(&mut self) -> &mut Document {
        &mut self.document
    }
}

impl Tree {
    pub fn new(document: Document) -> Self {
        let atoms = document
            .nodes
            .iter()
            .map(|node| {
                if ATOMS
                    .binary_search_by(|atom| atom.as_str().cmp(node.tag.as_str()))
                    .is_ok()
                {
                    node.tag.clone()
                } else {
                    Text::new()
                }
            })
            .collect();
        let namespaces = vec![Text::new(); document.nodes.len()];
        Self {
            document,
            atoms,
            namespaces,
        }
    }

    pub fn clone_for_readability(&self) -> Self {
        Self {
            document: self.document.clone(),
            atoms: self.atoms.clone(),
            namespaces: vec![Text::new(); self.namespaces.len()],
        }
    }

    pub fn outer_html(&self, root: NodeId) -> String {
        let mut output = Vec::new();
        let _ = crate::render::html_dom(self, root, &mut output);
        String::from_utf8(output).unwrap()
    }

    pub fn inner_html(&self, root: NodeId) -> String {
        self.nodes[root]
            .children
            .iter()
            .map(|&child| self.outer_html(child))
            .collect::<String>()
            .trim_matches(is_space)
            .into()
    }

    pub fn to_html(&self) -> String {
        self.outer_html(0)
    }

    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let node = self.document.create_element(tag);
        self.atoms.push(Text::new());
        self.namespaces.push(Text::new());
        node
    }

    pub fn create_text(&mut self, data: &str) -> NodeId {
        let node = self.nodes.len();
        let mut text = Node::new(Kind::Text);
        text.data = data.into();
        self.nodes.push(text);
        self.atoms.push(Text::new());
        self.namespaces.push(Text::new());
        node
    }

    pub fn set_tag(&mut self, node: NodeId, tag: &str) {
        if self.nodes[node].kind == Kind::Element {
            self.nodes[node].tag = tag.into();
        }
    }

    pub fn all(&self, root: NodeId, tags: &[&str]) -> Vec<NodeId> {
        if tags.is_empty() {
            return Vec::new();
        }
        if tags.len() == 1 {
            return self.tagged(root, tags[0]);
        }
        let mut found = vec![Vec::new(); tags.len()];
        let mut pending: Vec<_> = self.nodes[root].children.iter().rev().copied().collect();
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if node.kind == Kind::Element {
                for (position, tag) in tags.iter().enumerate() {
                    if node.tag == *tag {
                        found[position].push(index);
                    }
                }
            }
            pending.extend(node.children.iter().rev());
        }
        found.into_iter().flatten().collect()
    }

    pub fn children_elements(&self, node: NodeId) -> Vec<NodeId> {
        self.nodes[node]
            .children
            .iter()
            .copied()
            .filter(|&child| self.nodes[child].kind == Kind::Element)
            .collect()
    }

    pub fn next_sibling(&self, node: NodeId) -> Option<NodeId> {
        let siblings = &self.nodes[self.nodes[node].parent?].children;
        let index = siblings.iter().position(|&child| child == node)?;
        siblings.get(index + 1).copied()
    }

    pub fn previous_element(&self, node: NodeId) -> Option<NodeId> {
        let siblings = &self.nodes[self.nodes[node].parent?].children;
        let index = siblings.iter().position(|&child| child == node)?;
        siblings[..index]
            .iter()
            .rev()
            .copied()
            .find(|&child| self.nodes[child].kind == Kind::Element)
    }

    pub fn next_node(&self, mut node: NodeId, skip_children: bool) -> Option<NodeId> {
        if !skip_children {
            if let Some(&child) = self.nodes[node].children.first() {
                return Some(child);
            }
        }
        loop {
            if let Some(sibling) = self.next_sibling(node) {
                return Some(sibling);
            }
            node = self.nodes[node].parent?;
        }
    }

    pub fn remove_next(&mut self, node: NodeId) -> Option<NodeId> {
        let next = self.next_element(node, true);
        self.detach(node);
        next
    }

    pub fn next_element(&self, mut node: NodeId, skip_children: bool) -> Option<NodeId> {
        if !skip_children {
            if let Some(child) = self.nodes[node]
                .children
                .iter()
                .copied()
                .find(|&child| self.nodes[child].kind == Kind::Element)
            {
                return Some(child);
            }
        }
        loop {
            let mut sibling = self.next_sibling(node);
            while let Some(index) = sibling {
                if self.nodes[index].kind == Kind::Element {
                    return Some(index);
                }
                sibling = self.next_sibling(index);
            }
            node = self.nodes[node].parent?;
        }
    }

    pub fn ancestors(&self, node: NodeId, max_depth: usize) -> Vec<NodeId> {
        let mut ancestors = Vec::new();
        let mut parent = self.nodes[node].parent;
        while let Some(index) = parent {
            ancestors.push(index);
            if max_depth > 0 && ancestors.len() == max_depth {
                break;
            }
            parent = self.nodes[index].parent;
        }
        ancestors
    }

    pub fn ancestor_tag(&self, node: NodeId, tag: &str, max_depth: usize) -> bool {
        self.ancestors(node, if max_depth > 0 { max_depth + 1 } else { 0 })
            .into_iter()
            .any(|node| self.nodes[node].tag == tag)
    }

    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, before: NodeId) {
        self.detach(child);
        let position = self.nodes[parent]
            .children
            .iter()
            .position(|&node| node == before)
            .unwrap();
        self.nodes[parent].children.insert(position, child);
        self.nodes[child].parent = Some(parent);
    }

    pub fn replace(&mut self, old: NodeId, new: NodeId) {
        if let Some(parent) = self.nodes[old].parent {
            self.insert_before(parent, new, old);
            self.detach(old);
        }
    }

    pub fn import(&mut self, source: &Tree, root: NodeId) -> NodeId {
        let imported = self.nodes.len();
        let mut pending = vec![(root, None)];
        while let Some((original, parent)) = pending.pop() {
            let mut node = source.nodes[original].clone();
            let index = self.nodes.len();
            pending.extend(
                node.children
                    .iter()
                    .rev()
                    .map(|&child| (child, Some(index))),
            );
            node.children.clear();
            node.parent = parent;
            self.nodes.push(node);
            self.atoms.push(source.atoms[original].clone());
            self.namespaces.push(Text::new());
            if let Some(parent) = parent {
                self.nodes[parent].children.push(index);
            }
        }
        imported
    }

    pub fn clone_node(&mut self, root: NodeId) -> NodeId {
        let cloned = self.nodes.len();
        let mut pending = vec![(root, None)];
        while let Some((original, parent)) = pending.pop() {
            let mut node = self.nodes[original].clone();
            let atom = self.atoms[original].clone();
            let index = self.nodes.len();
            pending.extend(
                node.children
                    .iter()
                    .rev()
                    .map(|&child| (child, Some(index))),
            );
            node.children.clear();
            node.parent = parent;
            self.nodes.push(node);
            self.atoms.push(atom);
            self.namespaces.push(Text::new());
            if let Some(parent) = parent {
                self.nodes[parent].children.push(index);
            }
        }
        cloned
    }

    pub fn first(&self, root: NodeId, tag: &str) -> Option<NodeId> {
        self.document.find_element(root, |node| node.tag == tag)
    }

    pub fn has_text(&self, node: NodeId) -> bool {
        let mut pending = vec![node];
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if node.kind == Kind::Text && node.data.chars().any(|character| !is_space(character)) {
                return true;
            }
            pending.extend(node.children.iter().rev());
        }
        false
    }

    pub fn whitespace(&self, node: NodeId) -> bool {
        (self.nodes[node].kind == Kind::Text && !self.has_text(node))
            || (self.nodes[node].kind == Kind::Element && self.nodes[node].tag == "br")
    }

    pub fn next_content(&self, mut node: Option<NodeId>) -> Option<NodeId> {
        while let Some(index) = node {
            if self.nodes[index].kind == Kind::Element || self.has_text(index) {
                return node;
            }
            node = self.next_sibling(index);
        }
        None
    }

    pub fn single_tag(&self, root: NodeId, tag: &str) -> bool {
        let mut found = false;
        for &child in &self.nodes[root].children {
            match self.nodes[child].kind {
                Kind::Element => {
                    if found || self.nodes[child].tag != tag {
                        return false;
                    }
                    found = true;
                }
                Kind::Text if self.has_text(child) => return false,
                _ => {}
            }
        }
        found
    }

    pub fn empty_element(&self, root: NodeId) -> bool {
        self.nodes[root].kind == Kind::Element
            && self.nodes[root]
                .children
                .iter()
                .all(|&child| match self.nodes[child].kind {
                    Kind::Text => !self.has_text(child),
                    Kind::Element => matches!(self.nodes[child].tag.as_str(), "br" | "hr"),
                    _ => true,
                })
    }

    pub fn phrasing(&self, root: NodeId, descend: bool) -> bool {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            match self.nodes[node].kind {
                Kind::Text | Kind::Comment => continue,
                Kind::Element => {}
                _ => return false,
            }
            match self.atoms[node].as_str() {
                "math" => continue,
                "a" | "abbr" | "acronym" | "area" | "audio" | "b" | "bdi" | "bdo" | "big"
                | "blink" | "br" | "button" | "canvas" | "cite" | "code" | "data" | "datalist"
                | "del" | "dfn" | "em" | "embed" | "font" | "i" | "img" | "input" | "ins"
                | "kbd" | "label" | "link" | "map" | "mark" | "marquee" | "meta" | "meter"
                | "noscript" | "object" | "output" | "progress" | "q" | "ruby" | "s" | "samp"
                | "script" | "select" | "slot" | "small" | "span" | "strike" | "strong" | "sub"
                | "sup" | "template" | "textarea" | "time" | "tt" | "u" | "var" | "wbr" => {}
                _ => return false,
            }
            if descend {
                pending.extend(self.nodes[node].children.iter().rev());
            }
        }
        true
    }

    pub fn has_block(&self, root: NodeId) -> bool {
        self.document
            .find_element(root, |node| {
                matches!(
                    node.tag.as_str(),
                    "blockquote"
                        | "dl"
                        | "div"
                        | "img"
                        | "ol"
                        | "p"
                        | "pre"
                        | "table"
                        | "ul"
                        | "select"
                )
            })
            .is_some()
    }

    pub fn inner_text(&self, node: NodeId, normalize: bool) -> String {
        let text = self.text(node);
        if normalize {
            normalize_whitespace(&text)
        } else {
            text.trim_matches(is_space).into()
        }
    }
}

pub(crate) fn normalize_whitespace(text: &str) -> String {
    SPACES
        .replace_all(text.trim_matches(is_space), " ")
        .into_owned()
}

pub(crate) fn word_count(text: &str) -> usize {
    text.split(is_space).filter(|word| !word.is_empty()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_preserve_group_order_duplicates_and_reparenting() {
        let mut tree = Tree::new(Document::parse("<article><p>first</p><section><p>second</p></section><div> &nbsp; <br></div></article>"));
        let article = tree.first(0, "article").unwrap();
        let paragraph = tree.first(article, "p").unwrap();
        tree.append(article, paragraph);
        let detached = tree.create_element("p");
        tree.set_tag(detached, "section");
        for root in 0..tree.nodes.len() {
            for tags in [
                &[][..],
                &["p"][..],
                &["p", "section", "p", "missing"][..],
                &["article", "div", "br"][..],
            ] {
                let expected: Vec<_> = tags.iter().flat_map(|tag| tree.tagged(root, tag)).collect();
                assert_eq!(tree.all(root, tags), expected);
                for tag in tags {
                    assert_eq!(
                        tree.first(root, tag),
                        tree.tagged(root, tag).first().copied()
                    );
                }
            }
            assert_eq!(
                tree.has_text(root),
                tree.text(root)
                    .chars()
                    .any(|character| !is_space(character))
            );
            assert_eq!(
                tree.has_block(root),
                tree.elements(root).into_iter().any(|node| matches!(
                    tree.nodes[node].tag.as_str(),
                    "blockquote"
                        | "dl"
                        | "div"
                        | "img"
                        | "ol"
                        | "p"
                        | "pre"
                        | "table"
                        | "ul"
                        | "select"
                ))
            );
        }
    }

    #[test]
    fn go_created_and_retagged_atoms() {
        let mut tree = Tree::new(Document::parse("<span>text</span><div>block</div>"));
        let span = tree.first(0, "span").unwrap();
        assert!(tree.phrasing(span, true));
        tree.set_tag(span, "div");
        assert!(tree.phrasing(span, true));
        let div = tree.first(0, "div").unwrap();
        let created = tree.create_element("span");
        assert!(!tree.phrasing(created, false));
        let cloned = tree.clone_node(div);
        assert_eq!(tree.atoms[cloned], tree.atoms[div]);
    }
}
