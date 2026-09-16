use crate::{dom::Tree, parse_html, Kind, NodeId};
use regex::Regex;
use std::sync::LazyLock;

static IMAGE_EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(jpg|jpeg|png|webp)").unwrap());

fn single_image(tree: &Tree, mut node: NodeId) -> bool {
    loop {
        if tree.nodes[node].tag == "img" {
            return true;
        }
        let children = tree.children_elements(node);
        if children.len() != 1 || tree.has_text(node) {
            return false;
        }
        node = children[0];
    }
}

pub(crate) fn unwrap_noscript_images(tree: &mut Tree) {
    for image in tree.tagged(0, "img") {
        if !tree.nodes[image].attrs.iter().any(|attr| {
            matches!(
                attr.key.as_str(),
                "src" | "data-src" | "srcset" | "data-srcset"
            ) || IMAGE_EXTENSION.is_match(&attr.value)
        }) {
            tree.detach(image);
        }
    }
    for noscript in tree.tagged(0, "noscript") {
        let mut parsed = Tree::new(parse_html(&tree.text(noscript)));
        let Some(body) = parsed.first(0, "body") else {
            continue;
        };
        if !single_image(&parsed, body) {
            continue;
        }
        if let Some(previous) = tree
            .previous_element(noscript)
            .filter(|&node| single_image(tree, node))
        {
            let previous_image = if tree.nodes[previous].tag == "img" {
                previous
            } else {
                tree.first(previous, "img").unwrap()
            };
            let image = parsed.first(body, "img").unwrap();
            for attr in tree.nodes[previous_image].attrs.clone() {
                if attr.value.is_empty()
                    || !(matches!(attr.key.as_str(), "src" | "srcset")
                        || IMAGE_EXTENSION.is_match(&attr.value))
                {
                    continue;
                }
                if parsed.nodes[image].attr(&attr.key) == attr.value.as_str() {
                    continue;
                }
                let key = if parsed.nodes[image].has_attr(&attr.key) {
                    format!("data-old-{}", attr.key).into()
                } else {
                    attr.key
                };
                parsed.nodes[image].set_attr(&key, &attr.value);
            }
            let root = parsed.children_elements(body)[0];
            let imported = tree.import(&parsed, root);
            tree.replace(previous, imported);
        } else {
            let mut image = parsed.children_elements(body)[0];
            if parsed.nodes[image].tag != "img" {
                image = parsed.first(image, "img").unwrap();
            }
            if parsed.nodes[image].attr("width") == "1" && parsed.nodes[image].attr("height") == "1"
            {
                continue;
            }
            let imported = tree.import(&parsed, image);
            tree.replace(noscript, imported);
        }
    }
}

pub(crate) fn remove_scripts(tree: &mut Tree) {
    for node in tree.all(0, &["script", "noscript"]).into_iter().rev() {
        tree.detach(node);
    }
}

pub(crate) fn prepare_document(tree: &mut Tree) {
    for node in tree.descendants(0) {
        if tree.nodes[node].kind == Kind::Comment {
            tree.detach(node);
        }
    }
    for node in tree.tagged(0, "style").into_iter().rev() {
        tree.detach(node);
    }
    if let Some(body) = tree.first(0, "body") {
        replace_breaks(tree, body);
    }
    for node in tree.tagged(0, "font").into_iter().rev() {
        tree.set_tag(node, "span");
    }
}

fn replace_breaks(tree: &mut Tree, root: NodeId) {
    let mut pending = tree.nodes[root].children.first().copied();
    while let Some(mut node) = pending {
        if !tree.contains(Some(root), Some(node)) {
            break;
        }
        if tree.nodes[node].kind == Kind::Element {
            if tree.nodes[node].tag == "pre" {
                pending = tree.next_node(node, true);
                continue;
            }
            if tree.nodes[node].tag == "br" {
                let mut next = tree.next_sibling(node);
                let mut replaced = false;
                loop {
                    next = tree.next_content(next);
                    let Some(index) = next else {
                        break;
                    };
                    if tree.nodes[index].tag != "br" {
                        break;
                    }
                    replaced = true;
                    next = tree.next_sibling(index);
                    tree.detach(index);
                }
                if replaced {
                    let paragraph = tree.create_element("p");
                    tree.replace(node, paragraph);
                    next = tree.next_sibling(paragraph);
                    while let Some(index) = next {
                        if tree.nodes[index].tag == "br"
                            && tree
                                .next_content(tree.next_sibling(index))
                                .is_some_and(|sibling| tree.nodes[sibling].tag == "br")
                        {
                            break;
                        }
                        if !tree.phrasing(index, false) {
                            break;
                        }
                        next = tree.next_sibling(index);
                        tree.append(paragraph, index);
                    }
                    while let Some(&last) = tree.nodes[paragraph].children.last() {
                        if !tree.whitespace(last) {
                            break;
                        }
                        tree.detach(last);
                    }
                    if let Some(parent) = tree.nodes[paragraph]
                        .parent
                        .filter(|&parent| tree.nodes[parent].tag == "p")
                    {
                        tree.set_tag(parent, "div");
                    }
                    node = paragraph;
                }
                pending = tree.next_node(node, true);
                continue;
            }
        }
        pending = tree.next_node(node, false);
    }
}
