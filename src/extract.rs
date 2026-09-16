use crate::{
    check::{maybe_candidate, probably_visible, unlikely},
    cleanup,
    dom::{normalize_whitespace, Tree},
    metadata::text_similarity,
    score::{byline, counts, get_score, has_score, initialize, link_density, set_score},
    sort::sort_by,
    Kind, NodeId, Options,
};
use regex::Regex;
use std::sync::LazyLock;

static SENTENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.( |$)").unwrap());

pub(crate) fn grab_article(
    mut next_tree: impl FnMut() -> Tree,
    options: &Options,
    title: &str,
    article_byline: &mut String,
    language: &mut String,
) -> Option<(Tree, NodeId)> {
    let mut strip_unlikely = true;
    let mut use_classes = true;
    let mut conditional = true;
    let mut attempts: Vec<(Tree, NodeId, i64)> = Vec::new();
    loop {
        let mut tree = next_tree();
        let page = tree.first(0, "body")?;
        let mut elements_to_score = Vec::new();
        let mut pending = tree.children_elements(0).first().copied();
        let mut remove_title = true;
        'prepare: while let Some(mut node) = pending {
            let matched = smol_str::format_smolstr!(
                "{} {}",
                tree.nodes[node].attr("class"),
                tree.nodes[node].attr("id")
            );
            if tree.nodes[node].tag == "html" {
                *language = tree.nodes[node].attr("lang").into();
            }
            if !probably_visible(&tree.nodes[node])
                || (tree.nodes[node].attr("aria-modal") == "true"
                    && tree.nodes[node].attr("role") == "dialog")
            {
                pending = tree.remove_next(node);
                continue;
            }
            if article_byline.is_empty() && byline(&tree, node, &matched) {
                let end = tree.next_element(node, true);
                let mut next = tree.next_element(node, false);
                while next.is_some() && next != end {
                    let child = next.unwrap();
                    if tree.nodes[child].attr("itemprop").contains("name") {
                        *article_byline = tree.inner_text(child, false);
                        pending = tree.remove_next(node);
                        continue 'prepare;
                    }
                    next = tree.next_element(child, false);
                }
                let content = tree.inner_text(node, false);
                let length = content.chars().count();
                if length > 0 && length < 100 {
                    *article_byline = normalize_whitespace(&content);
                    pending = tree.remove_next(node);
                    continue;
                }
            }
            if remove_title
                && matches!(tree.nodes[node].tag.as_str(), "h1" | "h2")
                && text_similarity(title, &tree.inner_text(node, false)) > 0.75
            {
                remove_title = false;
                pending = tree.remove_next(node);
                continue;
            }
            let tag = tree.nodes[node].tag.clone();
            if strip_unlikely {
                if tag != "body"
                    && tag != "a"
                    && unlikely(&matched)
                    && !maybe_candidate(&matched)
                    && !tree.ancestor_tag(node, "table", 3)
                    && !tree.ancestor_tag(node, "code", 3)
                {
                    pending = tree.remove_next(node);
                    continue;
                }
                if matches!(
                    tree.nodes[node].attr("role"),
                    "menu"
                        | "menubar"
                        | "complementary"
                        | "navigation"
                        | "alert"
                        | "alertdialog"
                        | "dialog"
                ) {
                    pending = tree.remove_next(node);
                    continue;
                }
            }
            if matches!(
                tag.as_str(),
                "div" | "section" | "header" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            ) && tree.empty_element(node)
            {
                pending = tree.remove_next(node);
                continue;
            }
            if options
                .tags_to_score
                .iter()
                .any(|candidate| candidate == tag.as_str())
            {
                elements_to_score.push(node);
            }
            if tag == "div" && !tree.ancestor_tag(node, "pre", 3) {
                let mut paragraph: Option<NodeId> = None;
                let mut child = tree.nodes[node].children.first().copied();
                while let Some(current) = child {
                    child = tree.next_sibling(current);
                    if tree.phrasing(current, true) {
                        if let Some(paragraph) = paragraph {
                            tree.append(paragraph, current);
                        } else if !tree.whitespace(current) {
                            let created = tree.create_element("p");
                            let cloned = tree.clone_node(current);
                            tree.append(created, cloned);
                            tree.replace(current, created);
                            paragraph = Some(created);
                        }
                    } else if let Some(previous) = paragraph.take() {
                        while let Some(&last) = tree.nodes[previous].children.last() {
                            if !tree.whitespace(last) {
                                break;
                            }
                            if let Some(next) = tree
                                .next_sibling(previous)
                                .filter(|&next| tree.nodes[next].kind == Kind::Element)
                            {
                                let parent = tree.nodes[previous].parent.unwrap();
                                tree.insert_before(parent, last, next);
                            } else {
                                tree.detach(last);
                            }
                        }
                    }
                }
                if tree.single_tag(node, "p") && link_density(&tree, node) < 0.25 {
                    let id = tree.nodes[node].attr("id").to_owned();
                    let class = tree.nodes[node].attr("class").to_owned();
                    let child = tree.children_elements(node)[0];
                    tree.replace(node, child);
                    node = child;
                    if !id.is_empty() && tree.nodes[node].attr("id").is_empty() {
                        tree.nodes[node].set_attr("id", &id);
                    }
                    if !class.is_empty() && tree.nodes[node].attr("class").is_empty() {
                        tree.nodes[node].set_attr("class", &class);
                    }
                    elements_to_score.push(node);
                } else if !tree.has_block(node) {
                    tree.set_tag(node, "p");
                    elements_to_score.push(node);
                }
            }
            pending = tree.next_element(node, false);
        }

        let mut candidates = Vec::new();
        for node in elements_to_score {
            if !tree.nodes[node].parent.is_some_and(|parent| {
                tree.nodes[parent].kind == Kind::Element && !tree.nodes[parent].tag.is_empty()
            }) {
                continue;
            }
            let (characters, commas) = counts(&tree, node);
            if characters < 25 {
                continue;
            }
            let content_score = 2 + commas + (characters / 100).min(3);
            for (level, ancestor) in tree.ancestors(node, 5).into_iter().enumerate() {
                if tree.nodes[ancestor].tag.is_empty()
                    || !tree.nodes[ancestor]
                        .parent
                        .is_some_and(|parent| tree.nodes[parent].kind == Kind::Element)
                {
                    continue;
                }
                if !has_score(&tree, ancestor) {
                    initialize(&mut tree, ancestor, use_classes);
                    candidates.push(ancestor);
                }
                let divider = match level {
                    0 => 1,
                    1 => 2,
                    _ => level * 3,
                };
                let score = get_score(&tree, ancestor) + content_score as f64 / divider as f64;
                set_score(&mut tree, ancestor, score);
            }
        }
        for &candidate in &candidates {
            let score = get_score(&tree, candidate) * (1.0 - link_density(&tree, candidate));
            set_score(&mut tree, candidate, score);
        }
        sort_by(&mut candidates, |first, second| {
            get_score(&tree, *first) > get_score(&tree, *second)
        });
        let maximum = usize::try_from(options.n_top_candidates).expect("negative NTopCandidates");
        candidates.truncate(maximum);
        let top = candidates.first().copied();
        let created_top = top.is_none_or(|top| tree.nodes[top].tag == "body");
        let mut top = if created_top {
            let top = tree.create_element("div");
            for child in tree.nodes[page].children.clone() {
                tree.append(top, child);
            }
            tree.append(page, top);
            initialize(&mut tree, top, use_classes);
            top
        } else {
            top.unwrap()
        };
        if !created_top {
            let top_score = get_score(&tree, top);
            let alternatives: Vec<_> = candidates
                .iter()
                .skip(1)
                .filter(|&&node| get_score(&tree, node) / top_score >= 0.75)
                .map(|&node| tree.ancestors(node, 0))
                .collect();
            if alternatives.len() >= 3 {
                let mut parent = tree.nodes[top].parent;
                while let Some(node) = parent {
                    if tree.nodes[node].tag == "body" {
                        break;
                    }
                    if alternatives
                        .iter()
                        .filter(|ancestors| ancestors.contains(&node))
                        .count()
                        >= 3
                    {
                        top = node;
                        break;
                    }
                    parent = tree.nodes[node].parent;
                }
            }
            if !has_score(&tree, top) {
                initialize(&mut tree, top, use_classes);
            }
            let mut last_score = get_score(&tree, top);
            let threshold = last_score / 3.0;
            let mut parent = tree.nodes[top].parent;
            while let Some(node) = parent {
                if tree.nodes[node].tag == "body" {
                    break;
                }
                parent = tree.nodes[node].parent;
                if !has_score(&tree, node) {
                    continue;
                }
                let score = get_score(&tree, node);
                if score < threshold {
                    break;
                }
                if score > last_score {
                    top = node;
                    break;
                }
                last_score = score;
            }
            let mut parent = tree.nodes[top].parent;
            while let Some(node) = parent {
                if tree.nodes[node].tag == "body" || tree.children_elements(node).len() != 1 {
                    break;
                }
                top = node;
                parent = tree.nodes[top].parent;
            }
            if !has_score(&tree, top) {
                initialize(&mut tree, top, use_classes);
            }
        }
        let article = tree.create_element("div");
        let top_score = get_score(&tree, top);
        let threshold = if top_score.is_nan() {
            f64::NAN
        } else {
            (top_score * 0.2).max(10.0)
        };
        let class = tree.nodes[top].attr("class").to_owned();
        let parent = tree.nodes[top].parent.unwrap();
        for sibling in tree.children_elements(parent) {
            let mut append = sibling == top;
            if !append {
                let bonus = if !class.is_empty() && tree.nodes[sibling].attr("class") == class {
                    top_score * 0.2
                } else {
                    0.0
                };
                if has_score(&tree, sibling) && get_score(&tree, sibling) + bonus >= threshold {
                    append = true;
                } else if tree.nodes[sibling].tag == "p" {
                    let density = link_density(&tree, sibling);
                    let content = tree.inner_text(sibling, true);
                    let length = content.chars().count();
                    append = (length > 80 && density < 0.25)
                        || (length < 80
                            && length > 0
                            && density == 0.0
                            && SENTENCE.is_match(&content));
                }
            }
            if append {
                if !matches!(
                    tree.nodes[sibling].tag.as_str(),
                    "div" | "article" | "section" | "p" | "ol" | "ul"
                ) {
                    tree.set_tag(sibling, "div");
                }
                tree.append(article, sibling);
            }
        }
        cleanup::article(&mut tree, article, options, use_classes, conditional);
        if created_top {
            if let Some(&first) = tree
                .children_elements(article)
                .first()
                .filter(|&&child| tree.nodes[child].tag == "div")
            {
                tree.nodes[first].set_attr("id", "readability-page-1");
                tree.nodes[first].set_attr("class", "page");
            }
        } else {
            let wrapper = tree.create_element("div");
            tree.nodes[wrapper].set_attr("id", "readability-page-1");
            tree.nodes[wrapper].set_attr("class", "page");
            for child in tree.nodes[article].children.clone() {
                tree.append(wrapper, child);
            }
            tree.append(article, wrapper);
        }
        let length = counts(&tree, article).0;
        if length >= options.char_thresholds {
            return Some((tree, article));
        }
        attempts.push((tree, article, length));
        if strip_unlikely {
            strip_unlikely = false;
        } else if use_classes {
            use_classes = false;
        } else if conditional {
            conditional = false;
        } else {
            sort_by(&mut attempts, |first, second| first.2 > second.2);
            let (tree, article, length) = attempts.remove(0);
            return (length != 0).then_some((tree, article));
        }
    }
}
