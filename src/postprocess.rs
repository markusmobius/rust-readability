use crate::{
    check::is_space,
    dom::Tree,
    url::{absolute, Url},
    Kind, NodeId, Options,
};
use regex::Regex;
use std::sync::LazyLock;

static SRCSET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([^\t\n\f\r ]+)([\t\n\f\r ]+[0-9.]+[xw])?([\t\n\f\r ]*(?:,|$))").unwrap()
});

pub(crate) fn process(tree: &mut Tree, root: NodeId, options: &Options, base: Option<&Url>) {
    for link in tree.tagged(root, "a") {
        let href = tree.nodes[link].attr("href").to_owned();
        if href.is_empty() {
            continue;
        }
        if href.starts_with("javascript:") {
            let children = tree.nodes[link].children.clone();
            let replacement = if children.len() == 1 && tree.nodes[children[0]].kind == Kind::Text {
                let text = tree.text(link);
                tree.create_text(&text)
            } else {
                let container = tree.create_element("span");
                for child in children {
                    tree.append(container, child);
                }
                container
            };
            tree.replace(link, replacement);
        } else {
            let value = absolute(&href, base);
            if value.is_empty() {
                tree.nodes[link].remove_attr("href");
            } else {
                tree.nodes[link].set_attr("href", &value);
            }
        }
    }
    for media in tree.all(
        root,
        &["img", "picture", "figure", "video", "audio", "source"],
    ) {
        for key in ["src", "poster"] {
            let value = tree.nodes[media].attr(key);
            if !value.is_empty() {
                let value = absolute(value, base);
                tree.nodes[media].set_attr(key, &value);
            }
        }
        let srcset = tree.nodes[media].attr("srcset");
        if !srcset.is_empty() {
            let value = SRCSET
                .replace_all(srcset, |matched: &regex::Captures<'_>| {
                    format!(
                        "{}{}{}",
                        absolute(&matched[1], base),
                        matched.get(2).map_or("", |part| part.as_str()),
                        &matched[3]
                    )
                })
                .into_owned();
            tree.nodes[media].set_attr("srcset", &value);
        }
    }
    let mut pending = Some(root);
    while let Some(node) = pending {
        if tree.nodes[node].parent.is_some()
            && matches!(tree.nodes[node].tag.as_str(), "div" | "section")
            && !tree.nodes[node].attr("id").starts_with("readability")
        {
            if tree.empty_element(node) {
                pending = tree.remove_next(node);
                continue;
            }
            if tree.single_tag(node, "div") || tree.single_tag(node, "section") {
                let child = tree.children_elements(node)[0];
                for attr in tree.nodes[node].attrs.clone() {
                    tree.nodes[child].set_attr(&attr.key, &attr.value);
                }
                tree.replace(node, child);
                pending = Some(child);
                continue;
            }
        }
        pending = tree.next_element(node, false);
    }
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if !options.keep_classes {
            if let Some(index) = tree.nodes[node]
                .attrs
                .iter()
                .position(|attr| attr.key == "class")
            {
                let preserved = tree.nodes[node].attrs[index]
                    .value
                    .split(is_space)
                    .filter(|part| {
                        !part.is_empty()
                            && options
                                .classes_to_preserve
                                .iter()
                                .any(|class| class == part)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if preserved.is_empty() {
                    tree.nodes[node].attrs.remove(index);
                } else {
                    tree.nodes[node].attrs[index].value = preserved.into();
                }
            }
        }
        tree.nodes[node].remove_attr("data-readability-score");
        tree.nodes[node].remove_attr("data-readability-table");
        pending.extend(tree.children_elements(node).into_iter().rev());
    }
}
