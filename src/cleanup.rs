use crate::{
    check::is_space,
    dom::Tree,
    metadata::lowercase,
    score::{class_weight, link_coefficient, CharCounter},
    url::Url,
    Kind, NodeId, Options,
};
use regex::Regex;
use std::{num::IntErrorKind, sync::LazyLock};

static VIDEOS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)//(www\.)?((dailymotion|youtube|youtube-nocookie|player\.vimeo|v\.qq|bilibili|live\.bilibili)\.com|(archive|upload\.wikimedia)\.org|player\.twitch\.tv)").unwrap()
});
static SHARE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)((?-u:\b)|_)(share|sharedaddy)((?-u:\b)|_)").unwrap());
static LAZY_SRCSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(jpg|jpeg|png|webp)[\t\n\f\r ]+[0-9]").unwrap());
static LAZY_SRC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[\t\n\f\r ]*[^\t\n\f\r ]+\.(jpg|jpeg|png|webp)[^\t\n\f\r ]*[\t\n\f\r ]*$")
        .unwrap()
});
static IMAGE_EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(jpg|jpeg|png|webp)").unwrap());
static BASE64: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^data:[\t\n\f\r ]*([^\t\n\f\r ;,]+)[\t\n\f\r ]*;[\t\n\f\r ]*base64[\t\n\f\r ]*,",
    )
    .unwrap()
});
static ADS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(ad(vertising|vertisement)?|pub(licit\x{e9})?|werb(ung)?|\x{5e7f}\x{544a}|\x{420}\x{435}\x{43a}\x{43b}\x{430}\x{43c}\x{430}|Anuncio)$").unwrap()
});
static LOADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^((loading|\x{6b63}\x{5728}\x{52a0}\x{8f7d}|\x{417}\x{430}\x{433}\x{440}\x{443}\x{437}\x{43a}\x{430}|chargement|cargando)(\x{2026}|\.\.\.)?)$").unwrap()
});

pub(crate) fn integer(value: &str) -> i64 {
    value
        .parse()
        .unwrap_or_else(|error: std::num::ParseIntError| match error.kind() {
            IntErrorKind::PosOverflow => i64::MAX,
            IntErrorKind::NegOverflow => i64::MIN,
            _ => 0,
        })
}

fn mark_table(tree: &mut Tree, node: NodeId, data: bool) {
    if data {
        tree.nodes[node].set_attr("data-readability-table", "true");
    } else {
        tree.nodes[node].remove_attr("data-readability-table");
    }
}

fn mark_data_tables(tree: &mut Tree, root: NodeId) {
    for table in tree.tagged(root, "table") {
        let node = &tree.nodes[table];
        if node.attr("role") == "presentation" || node.attr("datatable") == "0" {
            mark_table(tree, table, false);
            continue;
        }
        if node.has_attr("summary") {
            mark_table(tree, table, true);
            continue;
        }
        let decision = tree.elements(table).into_iter().find_map(|child| {
            let node = &tree.nodes[child];
            match node.tag.as_str() {
                "col" | "colgroup" | "tfoot" | "thead" | "th" => Some(true),
                "caption" if !node.children.is_empty() => Some(true),
                "table" => Some(false),
                _ => None,
            }
        });
        if let Some(data) = decision {
            mark_table(tree, table, data);
            continue;
        }
        let mut rows = 0i64;
        let mut columns = 0i64;
        for row in tree.tagged(table, "tr") {
            let span = integer(tree.nodes[row].attr("rowspan"));
            rows = rows.wrapping_add(if span == 0 { 1 } else { span });
            let mut row_columns = 0i64;
            for cell in tree.tagged(row, "td") {
                let span = integer(tree.nodes[cell].attr("colspan"));
                row_columns = row_columns.wrapping_add(if span == 0 { 1 } else { span });
            }
            columns = columns.max(row_columns);
        }
        if rows == 1 || columns == 1 {
            mark_table(tree, table, false);
        } else if rows >= 10 || columns > 4 || rows.wrapping_mul(columns) > 10 {
            mark_table(tree, table, true);
        }
    }
}

fn fix_lazy_images(tree: &mut Tree, root: NodeId) {
    for image in tree.all(root, &["img", "picture", "figure"]) {
        let mut src = tree.nodes[image].attr("src").to_owned();
        let srcset = tree.nodes[image].attr("srcset").to_owned();
        let tag = tree.nodes[image].tag.clone();
        let class = tree.nodes[image].attr("class").to_owned();
        if let Some(parts) = BASE64.captures(&src) {
            if &parts[1] == "image/svg+xml" {
                continue;
            }
            let removable = tree.nodes[image].attrs.iter().any(|attr| {
                attr.key != "src"
                    && IMAGE_EXTENSION.is_match(&attr.value)
                    && Url::request(&attr.value).is_some()
            });
            if removable && src.len() - src.find("base64").map_or(6, |index| index + 7) < 133 {
                src.clear();
                tree.nodes[image].remove_attr("src");
            }
        }
        if (!src.is_empty() || !srcset.is_empty()) && !lowercase(&class).contains("lazy") {
            continue;
        }
        for attr in tree.nodes[image].attrs.clone() {
            if matches!(attr.key.as_str(), "src" | "srcset" | "alt") {
                continue;
            }
            let target = if LAZY_SRCSET.is_match(&attr.value) {
                "srcset"
            } else if LAZY_SRC.is_match(&attr.value) {
                "src"
            } else {
                continue;
            };
            if Url::request(&attr.value).is_none() {
                continue;
            }
            if tag == "img" || tag == "picture" {
                tree.nodes[image].set_attr(target, &attr.value);
            } else if tag == "figure" && tree.all(image, &["img", "picture"]).is_empty() {
                let created = tree.create_element("img");
                tree.nodes[created].set_attr(target, &attr.value);
                tree.append(image, created);
            }
        }
    }
}

fn video(tree: &Tree, node: NodeId, options: &Options) -> bool {
    if !matches!(tree.nodes[node].tag.as_str(), "object" | "embed" | "iframe") {
        return false;
    }
    let pattern = options.allowed_video_regex.as_ref().unwrap_or(&VIDEOS);
    tree.nodes[node]
        .attrs
        .iter()
        .any(|attr| pattern.is_match(&attr.value))
        || (tree.nodes[node].tag == "object" && pattern.is_match(&tree.inner_html(node)))
}

fn clean(tree: &mut Tree, root: NodeId, tags: &[&str], options: &Options) {
    let reversed: Vec<_> = tags.iter().rev().copied().collect();
    for node in tree.all(root, &reversed).into_iter().rev() {
        if tree.contains(Some(root), Some(node)) && !video(tree, node, options) {
            tree.detach(node);
        }
    }
}

#[derive(Default)]
struct Statistics {
    chars: CharCounter,
    text: CharCounter,
    list: CharCounter,
    heading: CharCounter,
    link_weight: f64,
    commas: i64,
    paragraphs: i64,
    images: i64,
    items: i64,
    inputs: i64,
    embeds: i64,
    video: bool,
    last_text: String,
}

impl Statistics {
    fn scan(tree: &Tree, root: NodeId, options: &Options) -> Self {
        let mut stats = Self::default();
        let mut links = Vec::<CharCounter>::new();
        let mut pending: Vec<(NodeId, bool, bool, bool, Option<usize>, bool)> = tree.nodes[root]
            .children
            .iter()
            .rev()
            .map(|&child| (child, false, false, false, None, false))
            .collect();
        while let Some((node, mut text, mut list, mut heading, mut link, closing)) = pending.pop() {
            if closing {
                let coefficient = if tree.nodes[node]
                    .parent
                    .is_some_and(|parent| tree.nodes[parent].tag == "figcaption")
                {
                    0.0
                } else {
                    link_coefficient(tree, node)
                };
                stats.link_weight += links[link.unwrap()].total as f64 * coefficient;
                continue;
            }
            if tree.nodes[node].kind == Kind::Text {
                let previous = stats.chars.total;
                let summary = tree.nodes[node].text_summary();
                stats.chars.append(&summary);
                stats.commas += summary.commas;
                if text {
                    stats.text.append(&summary);
                }
                if list {
                    stats.list.append(&summary);
                }
                if heading {
                    stats.heading.append(&summary);
                }
                if let Some(link) = link {
                    links[link].append(&summary);
                }
                if stats.chars.total > previous {
                    tree.nodes[node]
                        .data
                        .as_str()
                        .clone_into(&mut stats.last_text);
                }
                continue;
            }
            if tree.nodes[node].kind == Kind::Element {
                match tree.nodes[node].tag.as_str() {
                    "ul" | "ol" => {
                        list = true;
                        stats.list.reset_context();
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        heading = true;
                        stats.heading.reset_context();
                    }
                    "a" => {
                        link = Some(links.len());
                        links.push(CharCounter::default());
                        pending.push((node, text, list, heading, link, true));
                    }
                    "p" => stats.paragraphs += 1,
                    "img" => stats.images += 1,
                    "li" => stats.items += 1,
                    "input" => stats.inputs += 1,
                    "object" | "embed" | "iframe" => {
                        stats.embeds += 1;
                        stats.video |= video(tree, node, options);
                    }
                    _ => {}
                }
                if matches!(
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
                        | "span"
                        | "li"
                        | "td"
                ) {
                    text = true;
                    stats.text.reset_context();
                }
            }
            pending.extend(
                tree.nodes[node]
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, text, list, heading, link, false)),
            );
        }
        stats
    }
}

fn should_remove(
    tree: &Tree,
    node: NodeId,
    tag: &str,
    options: &Options,
    use_classes: bool,
) -> bool {
    if tag == "table" && tree.nodes[node].has_attr("data-readability-table") {
        return false;
    }
    if tree.ancestors(node, 0).iter().any(|&ancestor| {
        tree.nodes[ancestor].tag == "table"
            && tree.nodes[ancestor].has_attr("data-readability-table")
    }) {
        return false;
    }
    if tree.ancestor_tag(node, "code", 3) {
        return false;
    }
    let weight = class_weight(tree, node, use_classes);
    if weight < 0 {
        return true;
    }
    let stats = Statistics::scan(tree, node, options);
    if stats.video || stats.commas >= 10 {
        return false;
    }
    let last_text = stats.last_text.trim_matches(is_space);
    if !last_text.is_empty() && (ADS.is_match(last_text) || LOADING.is_match(last_text)) {
        return true;
    }
    let list =
        tag == "ul" || tag == "ol" || stats.list.total as f64 / stats.chars.total as f64 > 0.9;
    let (text_density, link_density, heading_density) = if stats.chars.total > 0 {
        (
            stats.text.total as f64 / stats.chars.total as f64,
            stats.link_weight / stats.chars.total as f64,
            stats.heading.total as f64 / stats.chars.total as f64,
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    let remove = (stats.images > 1
        && stats.paragraphs as f64 / (stats.images as f64) < 0.5
        && !tree.ancestor_tag(node, "figure", 3))
        || (!list && stats.items - 100 > stats.paragraphs)
        || (stats.inputs > stats.paragraphs / 3)
        || (!list
            && heading_density < 0.9
            && stats.chars.total < 25
            && (stats.images == 0 || stats.images > 2)
            && link_density > 0.0
            && !tree.ancestor_tag(node, "figure", 3))
        || (!list && weight < 25 && link_density > 0.2)
        || (weight >= 25 && link_density > 0.5)
        || (stats.embeds == 1 && stats.chars.total < 75)
        || stats.embeds > 1
        || (stats.images == 0 && text_density == 0.0);
    if list && remove {
        if tree
            .children_elements(node)
            .iter()
            .any(|&child| tree.children_elements(child).len() > 1)
        {
            return true;
        }
        if stats.images == stats.items {
            return false;
        }
    }
    remove
}

fn clean_conditionally(
    tree: &mut Tree,
    root: NodeId,
    tags: &[&str],
    options: &Options,
    use_classes: bool,
    enabled: bool,
) {
    if !enabled {
        return;
    }
    let reversed: Vec<_> = tags.iter().rev().copied().collect();
    for node in tree.all(root, &reversed).into_iter().rev() {
        if tree.contains(Some(root), Some(node))
            && should_remove(tree, node, &tree.nodes[node].tag, options, use_classes)
        {
            tree.detach(node);
        }
    }
}

fn clean_styles(tree: &mut Tree, root: NodeId) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree.nodes[node].tag == "svg" {
            continue;
        }
        let deprecated_size = matches!(
            tree.nodes[node].tag.as_str(),
            "table" | "th" | "td" | "hr" | "pre"
        );
        tree.nodes[node]
            .attrs
            .retain(|attr| match attr.key.as_str() {
                "width" | "height" => !deprecated_size,
                "align" | "background" | "bgcolor" | "border" | "cellpadding" | "cellspacing"
                | "frame" | "hspace" | "rules" | "style" | "valign" | "vspace" => false,
                _ => true,
            });
        pending.extend(tree.children_elements(node).into_iter().rev());
    }
}

fn has_paragraph_content(tree: &Tree, root: NodeId) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree.nodes[node].kind == Kind::Element
            && matches!(
                tree.nodes[node].tag.as_str(),
                "img" | "picture" | "embed" | "object" | "iframe"
            )
        {
            return true;
        }
        if tree.nodes[node].kind == Kind::Text && tree.has_text(node) {
            return true;
        }
        pending.extend(tree.nodes[node].children.iter().rev());
    }
    false
}

pub(crate) fn article(
    tree: &mut Tree,
    root: NodeId,
    options: &Options,
    use_classes: bool,
    conditional: bool,
) {
    mark_data_tables(tree, root);
    fix_lazy_images(tree, root);
    clean_conditionally(
        tree,
        root,
        &["form", "fieldset"],
        options,
        use_classes,
        conditional,
    );
    clean(
        tree,
        root,
        &["object", "embed", "footer", "link", "aside"],
        options,
    );
    let mut pending = tree
        .children_elements(root)
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        let matched = smol_str::format_smolstr!(
            "{} {}",
            tree.nodes[node].attr("class"),
            tree.nodes[node].attr("id")
        );
        if matched.len() > 1
            && SHARE.is_match(&matched)
            && (tree.text(node).chars().count() as i64) < options.char_thresholds
        {
            tree.detach(node);
        } else {
            pending.extend(tree.children_elements(node).into_iter().rev());
        }
    }
    clean(
        tree,
        root,
        &["iframe", "input", "textarea", "select", "button"],
        options,
    );
    for node in tree.all(root, &["h1", "h2"]).into_iter().rev() {
        if tree.nodes[node].parent.is_some() && class_weight(tree, node, use_classes) < 0 {
            tree.detach(node);
        }
    }
    clean_conditionally(
        tree,
        root,
        &["table", "ul", "div"],
        options,
        use_classes,
        conditional,
    );
    for node in tree.tagged(root, "h1").into_iter().rev() {
        tree.set_tag(node, "h2");
    }
    for node in tree.tagged(root, "p").into_iter().rev() {
        if tree.nodes[node].parent.is_some() && !has_paragraph_content(tree, node) {
            tree.detach(node);
        }
    }
    for node in tree.tagged(root, "br") {
        if tree
            .next_content(tree.next_sibling(node))
            .is_some_and(|next| tree.nodes[next].tag == "p")
        {
            tree.detach(node);
        }
    }
    clean_styles(tree, root);
    for table in tree.tagged(root, "table") {
        let body = if tree.single_tag(table, "tbody") {
            tree.children_elements(table)[0]
        } else {
            table
        };
        if !tree.single_tag(body, "tr") {
            continue;
        }
        let row = tree.children_elements(body)[0];
        if !tree.single_tag(row, "td") {
            continue;
        }
        let cell = tree.children_elements(row)[0];
        let tag = if tree.nodes[cell]
            .children
            .iter()
            .all(|&child| tree.phrasing(child, false))
        {
            "p"
        } else {
            "div"
        };
        tree.set_tag(cell, tag);
        tree.replace(table, cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batched_conditional_removals_match_sequential_tag_queries() {
        let tags = ["form", "fieldset", "table", "ul", "div"];
        for outer in tags {
            for inner in tags {
                for class in ["", "comment", "content"] {
                    let mut source = crate::parse_dom("<article></article>");
                    let root = source.first(0, "article").unwrap();
                    let parent = source.create_element(outer);
                    let child = source.create_element(inner);
                    let sibling = source.create_element(inner);
                    let paragraph = source.create_element("p");
                    let text = source
                        .create_text(&"An article sentence, with useful context. ".repeat(12));
                    source.append(root, parent);
                    source.append(parent, child);
                    source.append(root, sibling);
                    source.append(child, paragraph);
                    source.append(paragraph, text);
                    source.nodes[parent].set_attr("class", class);
                    for requested in [&tags[..2], &tags[2..], &tags[..]] {
                        for use_classes in [false, true] {
                            for enabled in [false, true] {
                                let options = Options::default();
                                let mut expected = source.clone();
                                if enabled {
                                    for tag in requested {
                                        for node in expected.tagged(root, tag).into_iter().rev() {
                                            if expected.nodes[node].parent.is_some()
                                                && should_remove(
                                                    &expected,
                                                    node,
                                                    tag,
                                                    &options,
                                                    use_classes,
                                                )
                                            {
                                                expected.detach(node);
                                            }
                                        }
                                    }
                                }
                                let mut actual = source.clone();
                                clean_conditionally(
                                    &mut actual,
                                    root,
                                    requested,
                                    &options,
                                    use_classes,
                                    enabled,
                                );
                                assert_eq!(actual, expected, "{outer}/{inner}, class={class}, classes={use_classes}, enabled={enabled}, tags={requested:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn batched_removals_match_sequential_tag_queries() {
        let tags = [
            "object", "embed", "footer", "link", "aside", "iframe", "input", "textarea", "select",
            "button",
        ];
        for outer in tags {
            for inner in tags {
                for preserve_outer in [false, true] {
                    let mut source = crate::parse_dom("<article></article>");
                    let root = source.first(0, "article").unwrap();
                    let parent = source.create_element(outer);
                    let child = source.create_element(inner);
                    let sibling = source.create_element(inner);
                    source.append(root, parent);
                    source.append(parent, child);
                    source.append(root, sibling);
                    if preserve_outer {
                        source.nodes[parent]
                            .set_attr("src", "https://www.youtube.com/embed/example");
                    }
                    let original = source.clone();
                    for requested in [&tags[..5], &tags[5..], &tags[..]] {
                        let options = Options::default();
                        let mut expected = source.clone();
                        for tag in requested {
                            for node in expected.tagged(root, tag).into_iter().rev() {
                                if expected.nodes[node].parent.is_some()
                                    && !video(&expected, node, &options)
                                {
                                    expected.detach(node);
                                }
                            }
                        }
                        let mut actual = source.clone();
                        clean(&mut actual, root, requested, &options);
                        assert_eq!(
                            actual, expected,
                            "{outer}/{inner}, preserved={preserve_outer}, tags={requested:?}"
                        );
                        assert_eq!(source, original);
                    }
                }
            }
        }
    }
}
