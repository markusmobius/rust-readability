use crate::{
    check_document, cleanup, dom::Tree, metadata, parse_html, prepare, render, url::Url, Attribute,
    Document, Kind, Node, Options, Parser,
};
use serde_json::Value;

fn cases() -> impl Iterator<Item = Value> {
    include_str!("../testdata/go-reference.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
}

#[test]
fn pinned_go_preparation() {
    let cases = cases().collect::<Vec<_>>();
    assert_eq!(cases.len(), 171);
    for case in cases {
        let source = case["source"].as_str().unwrap();
        let document = parse_html(source);
        assert_eq!(
            check_document(&document),
            case["readable"].as_bool().unwrap(),
            "readability {}",
            case["id"]
        );
        let mut tree = Tree::new(document.clone());
        prepare::unwrap_noscript_images(&mut tree);
        prepare::remove_scripts(&mut tree);
        prepare::prepare_document(&mut tree);
        assert_eq!(
            tree.to_html(),
            case["prepared"].as_str().unwrap(),
            "prepared {}",
            case["id"]
        );
        assert_eq!(document, parse_html(source));
    }
}

#[test]
fn pinned_go_metadata() {
    for case in cases() {
        let mut tree = Tree::new(parse_html(case["source"].as_str().unwrap()));
        let base = Url::parse(case["url"].as_str().unwrap()).unwrap();
        prepare::unwrap_noscript_images(&mut tree);
        let json_ld = if case["profile"] == "no-jsonld" {
            metadata::Metadata::new()
        } else {
            metadata::json_ld(&tree)
        };
        prepare::remove_scripts(&mut tree);
        prepare::prepare_document(&mut tree);
        assert_eq!(
            metadata::article_title(&tree),
            case["title_helper"].as_str().unwrap(),
            "title {}",
            case["id"]
        );
        assert_eq!(
            serde_json::to_value(metadata::article_metadata(&tree, &json_ld, Some(&base))).unwrap(),
            case["metadata"],
            "metadata {}",
            case["id"]
        );
    }
}

fn reference_document(case: &Value) -> Document {
    let mut document = Document { nodes: Vec::new() };
    let mut ancestors: Vec<usize> = Vec::new();
    for record in case["tree"].as_array().unwrap() {
        let depth = record["depth"].as_u64().unwrap() as usize;
        ancestors.truncate(depth);
        let parent = ancestors.last().copied();
        let index = document.nodes.len();
        if let Some(parent) = parent {
            document.nodes[parent].children.push(index);
        }
        let kind = match record["kind"].as_u64().unwrap() {
            1 => Kind::Text,
            2 => Kind::Document,
            3 => Kind::Element,
            4 => Kind::Comment,
            5 => Kind::Doctype,
            kind => panic!("unsupported Go node kind {kind}"),
        };
        let data = record["data"].as_str().unwrap();
        let attrs = record["attrs"]
            .as_array()
            .map(|attrs| {
                attrs
                    .iter()
                    .map(|attr| Attribute {
                        namespace: attr["Namespace"].as_str().unwrap().into(),
                        key: attr["Key"].as_str().unwrap().into(),
                        value: attr["Val"].as_str().unwrap().into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut node = Node::new(kind);
        if kind == Kind::Element {
            node.tag = data.into();
        } else {
            node.data = data.into();
        }
        node.attrs = attrs;
        node.parent = parent;
        document.nodes.push(node);
        ancestors.push(index);
    }
    document
}

#[test]
fn pinned_go_rendering() {
    for case in cases() {
        let document = reference_document(&case);
        if document.nodes.is_empty() {
            continue;
        }
        assert_eq!(
            document.outer_html(0),
            case["html"].as_str().unwrap(),
            "HTML renderer {}",
            case["id"]
        );
        assert_eq!(
            render::inner_text(&document, 0),
            case["text"].as_str().unwrap(),
            "text renderer {}",
            case["id"]
        );
    }
}

#[test]
fn pinned_go_render_nodes() {
    let helpers = helpers();
    let cases = helpers["render_nodes"].as_array().unwrap();
    assert_eq!(cases.len(), 154);
    for (index, case) in cases.iter().enumerate() {
        let mut document = Tree::new(reference_document(case));
        for (index, record) in case["tree"].as_array().unwrap().iter().enumerate() {
            document.namespaces[index] = record["namespace"].as_str().unwrap().into();
        }
        if let Some(atoms) = case["atoms"].as_array() {
            document.atoms = atoms
                .iter()
                .map(|atom| atom.as_str().unwrap().into())
                .collect();
        }
        let article = crate::Article {
            document,
            node: Some(0),
            metadata: Default::default(),
            language: String::new(),
        };
        let mut output = Vec::new();
        let error = article
            .render_html(&mut output)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert_eq!(
            error,
            case["error"].as_str().unwrap(),
            "render error {index}"
        );
        assert_eq!(
            String::from_utf8(output).unwrap(),
            case["html"].as_str().unwrap(),
            "render output {index}"
        );
    }
}

#[test]
fn pinned_go_parse_nodes() {
    let helpers = helpers();
    let cases = helpers["parse_nodes"].as_array().unwrap();
    assert_eq!(cases.len(), 44);
    let mut differences = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        differences.extend(parsing_differences(case, &index.to_string()));
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

fn parsing_differences(case: &Value, label: &str) -> Vec<String> {
    let mut differences = Vec::new();
    let actual = crate::parse_dom(case["source"].as_str().unwrap());
    if actual.to_html() != case["html"].as_str().unwrap() {
        differences.push(format!(
            "HTML {label}: {:?} != {:?}",
            actual.to_html(),
            case["html"]
        ));
    }
    let expected = reference_document(case);
    if actual.document != expected {
        differences.push(format!("nodes {label}"));
    }
    let namespaces = case["tree"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["namespace"].as_str().unwrap())
        .collect::<Vec<_>>();
    if actual.namespaces != namespaces {
        differences.push(format!(
            "namespaces {label}: {:?} != {namespaces:?}",
            actual.namespaces
        ));
    }
    let atoms = case["atoms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| atom.as_str().unwrap())
        .collect::<Vec<_>>();
    if actual.atoms != atoms {
        differences.push(format!("atoms {label}: {:?} != {atoms:?}", actual.atoms));
    }
    differences
}

#[test]
#[ignore = "requires tools/go_reference.py --html-corpus"]
fn html_corpus() {
    use std::io::BufRead;
    let file = std::fs::File::open(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/go-html-corpus.jsonl"),
    )
    .unwrap();
    let selected = std::env::var("RUST_READABILITY_CASE").ok();
    let mut count = 0;
    let mut checked = 0;
    let mut differences = Vec::new();
    for line in std::io::BufReader::new(file).lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        count += 1;
        let label = case["id"].as_str().unwrap();
        if selected.as_ref().is_some_and(|selected| selected != label) {
            continue;
        }
        checked += 1;
        differences.extend(parsing_differences(&case, label));
    }
    assert_eq!(count, 1793);
    assert!(checked > 0, "no matching parser cases");
    assert!(
        differences.is_empty(),
        "{checked} cases, {} differences:\n{}",
        differences.len(),
        differences.join("\n")
    );
}

fn options(case: &Value) -> Options {
    let mut options = Options::default();
    match case["profile"].as_str().unwrap() {
        "keep-classes" => options.keep_classes = true,
        "short" => options.char_thresholds = 50,
        "no-jsonld" => options.disable_json_ld = true,
        "custom-classes" => options.classes_to_preserve = vec!["keep".into(), "article".into()],
        "max-elements" => options.max_elems_to_parse = 8,
        "top-one" => options.n_top_candidates = 1,
        "custom-video" => {
            options.allowed_video_regex =
                Some(regex::Regex::new(r"(?i)example\.org/player").unwrap())
        }
        "no-score-tags" => options.tags_to_score.clear(),
        "default" => {}
        other => panic!("unknown profile {other}"),
    }
    options
}

#[test]
fn pinned_go_cleanup() {
    for case in cases() {
        let mut tree = Tree::new(parse_html(case["source"].as_str().unwrap()));
        prepare::unwrap_noscript_images(&mut tree);
        prepare::remove_scripts(&mut tree);
        prepare::prepare_document(&mut tree);
        if let Some(body) = tree.first(0, "body") {
            cleanup::article(&mut tree, body, &options(&case), true, true);
        }
        assert_eq!(
            tree.to_html(),
            case["cleaned"].as_str().unwrap(),
            "cleanup {}",
            case["id"]
        );
    }
}

fn assert_extraction(case: &Value) {
    let document = parse_html(case["source"].as_str().unwrap());
    let original = document.clone();
    let base = Url::parse(case["url"].as_str().unwrap()).unwrap();
    let result = Parser::with_options(options(case)).parse_document(&document, Some(&base));
    assert_eq!(document, original, "input mutation {}", case["id"]);
    let article = match result {
        Ok(article) => {
            assert_eq!(case["error"], "", "missing error {}", case["id"]);
            article
        }
        Err(error) => {
            assert_eq!(
                error.to_string(),
                case["error"].as_str().unwrap(),
                "error {}",
                case["id"]
            );
            return;
        }
    };
    for (field, actual) in [
        ("title", article.title()),
        ("byline", article.byline()),
        ("site_name", article.site_name()),
        ("image", article.image_url()),
        ("favicon", article.favicon()),
        ("language", article.language()),
        ("published", article.published_time_raw()),
        ("modified", article.modified_time_raw()),
    ] {
        assert_eq!(
            actual,
            case[field].as_str().unwrap(),
            "{field} {}",
            case["id"]
        );
    }
    assert_eq!(
        article.excerpt(),
        case["excerpt"].as_str().unwrap(),
        "excerpt {}",
        case["id"]
    );
    match article.node {
        None => {
            assert!(
                case["tree"].as_array().unwrap().is_empty(),
                "missing node {}",
                case["id"]
            );
            assert_eq!(
                article.html().unwrap_err().to_string(),
                "the Node field is nil"
            );
            assert_eq!(
                article.text().unwrap_err().to_string(),
                "the Node field is nil"
            );
        }
        Some(root) => {
            assert_eq!(
                article.html().unwrap(),
                case["html"].as_str().unwrap(),
                "HTML {}",
                case["id"]
            );
            assert_eq!(
                article.text().unwrap(),
                case["text"].as_str().unwrap(),
                "text {}",
                case["id"]
            );
            let expected = reference_document(case);
            let mut actual = vec![(root, 0)];
            let mut expected_nodes = vec![(0, 0)];
            while let Some((node, depth)) = actual.pop() {
                let (expected_node, expected_depth) =
                    expected_nodes.pop().expect("extra output node");
                let actual_node = &article.document.nodes[node];
                let expected_node = &expected.nodes[expected_node];
                assert_eq!(depth, expected_depth, "node depth {}", case["id"]);
                assert_eq!(
                    (
                        &actual_node.kind,
                        &actual_node.tag,
                        &actual_node.data,
                        &actual_node.attrs
                    ),
                    (
                        &expected_node.kind,
                        &expected_node.tag,
                        &expected_node.data,
                        &expected_node.attrs
                    ),
                    "node {}",
                    case["id"]
                );
                actual.extend(
                    actual_node
                        .children
                        .iter()
                        .rev()
                        .map(|&child| (child, depth + 1)),
                );
                expected_nodes.extend(
                    expected_node
                        .children
                        .iter()
                        .rev()
                        .map(|&child| (child, depth + 1)),
                );
            }
            assert!(
                expected_nodes.is_empty(),
                "missing output nodes {}",
                case["id"]
            );
        }
    }
}

#[test]
fn pinned_go_extraction() {
    for case in cases() {
        assert_extraction(&case);
    }
}

#[test]
#[ignore = "requires tools/go_reference.py --corpus"]
fn upstream_corpus() {
    use std::io::BufRead;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/go-corpus.jsonl");
    let file = std::fs::File::open(path)
        .expect("generate the pinned Go corpus with tools/go_reference.py --corpus");
    let selected = std::env::var("RUST_READABILITY_CASE").ok();
    let mut total = 0;
    let mut checked = 0;
    for line in std::io::BufReader::new(file).lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        total += 1;
        if selected
            .as_ref()
            .is_some_and(|selected| case["id"].as_str().unwrap() != selected)
        {
            continue;
        }
        assert_extraction(&case);
        checked += 1;
    }
    assert_eq!(total, 133);
    assert!(checked > 0, "no matching corpus page");
}

fn helpers() -> Value {
    serde_json::from_str(include_str!("../testdata/go-helpers.json")).unwrap()
}

#[test]
fn pinned_go_urls() {
    use base64::Engine;
    let helpers = helpers();
    let cases = helpers["urls"].as_array().unwrap();
    assert_eq!(cases.len(), 110);
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let request = case["request"].as_bool().unwrap();
        let actual = if request {
            Url::try_request(input)
        } else {
            Url::try_parse(input)
        };
        let actual = match actual {
            Ok(actual) => actual,
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    case["error"].as_str().unwrap(),
                    "{input:?}, request={request}"
                );
                continue;
            }
        };
        assert_eq!(case["error"], "", "unexpected URL {input:?}");
        for (field, value) in [
            ("rendered", actual.to_string()),
            ("host", String::from_utf8_lossy(&actual.host).into_owned()),
            ("hostname", actual.hostname().into_owned()),
            ("scheme", actual.scheme.clone()),
            ("opaque", actual.opaque.clone()),
            ("raw_path", actual.raw_path.clone()),
            ("query", actual.query.clone()),
            ("raw_fragment", actual.raw_fragment.clone()),
        ] {
            assert_eq!(
                value,
                case[field].as_str().unwrap(),
                "{field} {input:?}, request={request}"
            );
        }
        for (field, value) in [("path", actual.path), ("fragment", actual.fragment)] {
            assert_eq!(
                value,
                base64::engine::general_purpose::STANDARD
                    .decode(case[field].as_str().unwrap())
                    .unwrap(),
                "{field} {input:?}"
            );
        }
        assert_eq!(
            actual.force_query,
            case["force_query"].as_bool().unwrap(),
            "force_query {input:?}"
        );
    }
}

#[test]
fn pinned_go_parser_api() {
    let helpers = helpers();
    let cases = helpers["parser_api"].as_array().unwrap();
    assert_eq!(cases.len(), 29);
    let mut parser = Parser::new();
    let mut previous_mode = "";
    let mut document = Tree::new(Document { nodes: Vec::new() });
    for (index, case) in cases.iter().enumerate() {
        let mode = case["mode"].as_str().unwrap();
        if mode != previous_mode {
            parser = Parser::new();
            previous_mode = mode;
        }
        if case["pass"] == 0 {
            document = crate::parse_dom(case["source"].as_str().unwrap());
            if matches!(mode, "body" | "article") {
                let source = document;
                let root = source.first(0, mode).unwrap();
                let mut subtree = Tree::new(Document { nodes: Vec::new() });
                subtree.import(&source, root);
                document = subtree;
            }
        }
        assert_eq!(
            document.outer_html(0),
            case["before"].as_str().unwrap(),
            "before {index} {mode}"
        );
        let result = if matches!(mode, "clone" | "body" | "article") {
            parser.parse_dom(&document, None)
        } else {
            parser.parse_dom_and_mutate(&mut document, None)
        };
        assert_eq!(
            document.outer_html(0),
            case["after"].as_str().unwrap(),
            "after {index} {mode}"
        );
        let article = match result {
            Ok(article) => {
                assert_eq!(case["error"], "", "missing error {index} {mode}");
                article
            }
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    case["error"].as_str().unwrap(),
                    "error {index} {mode}"
                );
                continue;
            }
        };
        for (field, actual) in [
            ("title", article.title()),
            ("byline", article.byline()),
            ("language", article.language()),
        ] {
            assert_eq!(
                actual,
                case[field].as_str().unwrap(),
                "{field} {index} {mode}"
            );
        }
        assert_eq!(
            article.excerpt(),
            case["excerpt"].as_str().unwrap(),
            "excerpt {index} {mode}"
        );
        for (field, result) in [("html", article.html()), ("text", article.text())] {
            match result {
                Ok(output) => {
                    assert_eq!(
                        case[format!("{field}_error")],
                        "",
                        "missing {field} error {index} {mode}"
                    );
                    assert_eq!(
                        output,
                        case[field].as_str().unwrap(),
                        "{field} {index} {mode}"
                    );
                }
                Err(error) => assert_eq!(
                    error.to_string(),
                    case[format!("{field}_error")].as_str().unwrap(),
                    "{field} error {index} {mode}"
                ),
            }
        }
    }
}

#[test]
fn pinned_go_readers() {
    use base64::Engine;
    let helpers = helpers();
    let readers = helpers["readers"].as_array().unwrap();
    assert_eq!(readers.len(), 273);
    for (index, record) in readers.iter().enumerate() {
        let input = base64::engine::general_purpose::STANDARD
            .decode(record["input"].as_str().unwrap())
            .unwrap();
        match crate::encoding::decode(&input) {
            Ok(decoded) => {
                assert_eq!(record["error"], "", "reader {index}");
                assert_eq!(
                    parse_html(&decoded).to_html(),
                    record["html"].as_str().unwrap(),
                    "reader {index}"
                );
            }
            Err(error) => assert_eq!(
                error.to_string(),
                record["error"].as_str().unwrap(),
                "reader {index}"
            ),
        }
    }
}

#[test]
fn pinned_go_scalar_helpers() {
    let helpers = helpers();
    let tree = Tree::new(parse_html("<div></div>"));
    let node = tree.first(0, "div").unwrap();
    assert_eq!(helpers["strings"].as_array().unwrap().len(), 490);
    for case in helpers["strings"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let lowered = input.to_ascii_lowercase();
        assert_eq!(
            crate::score::negative(&lowered),
            case["negative"].as_bool().unwrap(),
            "negative {input:?}"
        );
        assert_eq!(
            crate::score::positive(&lowered),
            case["positive"].as_bool().unwrap(),
            "positive {input:?}"
        );
        assert_eq!(
            crate::score::byline(&tree, node, input),
            case["byline"].as_bool().unwrap(),
            "byline {input:?}"
        );
        assert_eq!(
            crate::dom::normalize_whitespace(input),
            case["normalize"].as_str().unwrap(),
            "normalize {input:?}"
        );
    }
    assert_eq!(helpers["entities"].as_array().unwrap().len(), 7);
    for case in helpers["entities"].as_array().unwrap() {
        assert_eq!(
            crate::entities::unescape(case["input"].as_str().unwrap()),
            case["output"].as_str().unwrap(),
            "entity {}",
            case["input"]
        );
    }
}

#[test]
fn pinned_go_sorting() {
    let helpers = helpers();
    assert_eq!(helpers["sorts"].as_array().unwrap().len(), 60);
    for (index, case) in helpers["sorts"].as_array().unwrap().iter().enumerate() {
        let keys = case["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key.as_i64().unwrap())
            .collect::<Vec<_>>();
        let mut indices = (0..keys.len()).collect::<Vec<_>>();
        crate::sort::sort_by(&mut indices, |&first, &second| keys[first] < keys[second]);
        assert_eq!(
            serde_json::to_value(indices).unwrap(),
            case["indices"],
            "sort {index}"
        );
    }
}

#[test]
fn pinned_go_json_ld() {
    let helpers = helpers();
    assert_eq!(helpers["json_ld"].as_array().unwrap().len(), 10);
    for case in helpers["json_ld"].as_array().unwrap() {
        let source = format!(
            "<title>Fallback title</title><script type='application/ld+json'>{}</script>",
            case["input"].as_str().unwrap()
        );
        let tree = Tree::new(parse_html(&source));
        let expected = if case["metadata"].is_null() {
            serde_json::json!({})
        } else {
            case["metadata"].clone()
        };
        assert_eq!(
            serde_json::to_value(metadata::json_ld(&tree)).unwrap(),
            expected,
            "JSON-LD {}",
            case["input"]
        );
    }
}

#[test]
fn pinned_go_time_layouts() {
    let helpers = helpers();
    let cases = helpers["layouts"].as_array().unwrap();
    assert_eq!(cases.len(), 140);
    for case in cases {
        let layout = case["layout"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        match crate::timestamp::parse_layout(layout, input, &crate::LocalTimeZone::utc()) {
            Ok(timestamp) => {
                assert_eq!(case["error"], "", "{layout:?} {input:?}");
                assert_eq!(
                    timestamp.seconds,
                    case["seconds"].as_i64().unwrap(),
                    "{layout:?} {input:?}"
                );
                assert_eq!(
                    timestamp.nanoseconds,
                    case["nanoseconds"].as_u64().unwrap() as u32,
                    "{layout:?} {input:?}"
                );
                assert_eq!(
                    timestamp.zone,
                    case["zone"].as_str().unwrap(),
                    "{layout:?} {input:?}"
                );
                assert_eq!(
                    timestamp.offset,
                    case["offset"].as_i64().unwrap() as i32,
                    "{layout:?} {input:?}"
                );
            }
            Err(error) => assert_eq!(
                error,
                case["error"].as_str().unwrap(),
                "{layout:?} {input:?}"
            ),
        }
    }
}

fn assert_timestamp(case: &Value) {
    let input = case["input"].as_str().unwrap();
    match crate::timestamp::parse_with_local_timezone(input, &crate::LocalTimeZone::utc()) {
        Ok((timestamp, layout)) => {
            assert_eq!(case["layout_error"], "", "{input:?}");
            assert_eq!(
                timestamp.seconds,
                case["seconds"].as_i64().unwrap(),
                "{input:?}"
            );
            assert_eq!(
                timestamp.nanoseconds,
                case["nanoseconds"].as_u64().unwrap() as u32,
                "{input:?}"
            );
            assert_eq!(timestamp.zone, case["zone"].as_str().unwrap(), "{input:?}");
            assert_eq!(
                timestamp.offset,
                case["offset"].as_i64().unwrap() as i32,
                "{input:?}"
            );
            assert_eq!(layout, case["layout"].as_str().unwrap(), "{input:?}");
        }
        Err(error) => assert_eq!(error, case["layout_error"].as_str().unwrap(), "{input:?}"),
    }
}

#[test]
fn pinned_go_numeric_dates() {
    let helpers = helpers();
    let mut count = 0;
    for case in helpers["dates"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        if !input.chars().all(|character| {
            character.is_ascii_digit()
                || matches!(
                    character,
                    '.' | '-' | '/' | ':' | '\u{5e74}' | '\u{6708}' | '\u{65e5}'
                )
        }) {
            continue;
        }
        assert_timestamp(case);
        count += 1;
    }
    assert!(count > 50);
}

#[test]
fn pinned_go_calendar_dates() {
    let helpers = helpers();
    let mut count = 0;
    for case in helpers["dates"].as_array().unwrap() {
        if case["input"].as_str().unwrap().contains(':') || case["layout_error"] != "" {
            continue;
        }
        assert_timestamp(case);
        count += 1;
    }
    assert!(count > 100);
}

#[test]
fn pinned_go_timestamps() {
    let helpers = helpers();
    let cases = helpers["dates"].as_array().unwrap();
    assert_eq!(cases.len(), 817);
    for case in cases {
        assert_timestamp(case);
    }
}

#[test]
fn pinned_go_timestamp_getters() {
    let helpers = helpers();
    let mut article = crate::Article {
        document: crate::parse_dom(""),
        node: None,
        metadata: Default::default(),
        language: String::new(),
    };
    assert!(matches!(
        article.published_time(),
        Err(crate::Error::TimestampMissing)
    ));
    assert!(matches!(
        article.modified_time(),
        Err(crate::Error::TimestampMissing)
    ));
    for case in helpers["dates"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        for field in ["publishedTime", "modifiedTime"] {
            article.metadata.insert(field.to_owned(), input.to_owned());
            let local = crate::LocalTimeZone::utc();
            let result = if field == "publishedTime" {
                article.published_time_with_local_timezone(&local)
            } else {
                article.modified_time_with_local_timezone(&local)
            };
            match result {
                Ok(timestamp) => {
                    assert_eq!(case["error"], "", "{field} {input:?}");
                    assert_eq!(
                        timestamp.seconds,
                        case["seconds"].as_i64().unwrap(),
                        "{field} {input:?}"
                    );
                    assert_eq!(
                        timestamp.nanoseconds,
                        case["nanoseconds"].as_u64().unwrap() as u32,
                        "{field} {input:?}"
                    );
                    assert_eq!(
                        timestamp.zone,
                        case["zone"].as_str().unwrap(),
                        "{field} {input:?}"
                    );
                    assert_eq!(
                        timestamp.offset,
                        case["offset"].as_i64().unwrap() as i32,
                        "{field} {input:?}"
                    );
                }
                Err(error) => {
                    assert_eq!(
                        error.to_string(),
                        case["error"]
                            .as_str()
                            .unwrap()
                            .replacen("publishedTime", field, 1),
                        "{field} {input:?}"
                    );
                    if !input.is_empty() {
                        assert!(std::error::Error::source(&error).is_some());
                    }
                }
            }
        }
    }
}

#[test]
fn pinned_go_timestamp_timezones() {
    use base64::Engine;
    let helpers = helpers();
    let zones = helpers["timezones"].as_array().unwrap();
    assert_eq!(zones.len(), 4);
    for zone in zones {
        let data = base64::engine::general_purpose::STANDARD
            .decode(zone["data"].as_str().unwrap())
            .unwrap();
        let local = crate::LocalTimeZone::from_tzif(&data).unwrap();
        let bundled = crate::LocalTimeZone::from_name(zone["name"].as_str().unwrap()).unwrap();
        let cases = zone["dates"].as_array().unwrap();
        assert_eq!(cases.len(), 817);
        for case in cases {
            for local in [&local, &bundled] {
                let input = case["input"].as_str().unwrap();
                match crate::timestamp::parse_with_local_timezone(input, local) {
                    Ok((timestamp, _)) => {
                        assert_eq!(case["error"], "", "{} {input:?}", zone["name"]);
                        assert_eq!(
                            timestamp.seconds,
                            case["seconds"].as_i64().unwrap(),
                            "{} {input:?}",
                            zone["name"]
                        );
                        assert_eq!(
                            timestamp.nanoseconds,
                            case["nanoseconds"].as_u64().unwrap() as u32,
                            "{} {input:?}",
                            zone["name"]
                        );
                        assert_eq!(
                            timestamp.zone,
                            case["zone"].as_str().unwrap(),
                            "{} {input:?}",
                            zone["name"]
                        );
                        assert_eq!(
                            timestamp.offset,
                            case["offset"].as_i64().unwrap() as i32,
                            "{} {input:?}",
                            zone["name"]
                        );
                    }
                    Err(error) => assert_eq!(
                        error,
                        case["error"].as_str().unwrap(),
                        "{} {input:?}",
                        zone["name"]
                    ),
                }
            }
        }
    }
}

#[test]
#[ignore = "requires tools/go_reference.py --native-timezone on this machine"]
fn native_local_timezone() {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        platform => platform,
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("target/go-native-timezone-{platform}.json"));
    let reference: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(reference["goos"], platform);
    for case in reference["dates"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        match crate::timestamp::parse_with_local_timezone(input, crate::LocalTimeZone::system()) {
            Ok((timestamp, _)) => {
                assert_eq!(case["error"], "", "{input:?}");
                assert_eq!(
                    timestamp.seconds,
                    case["seconds"].as_i64().unwrap(),
                    "{input:?}"
                );
                assert_eq!(
                    timestamp.nanoseconds,
                    case["nanoseconds"].as_u64().unwrap() as u32,
                    "{input:?}"
                );
                assert_eq!(timestamp.zone, case["zone"].as_str().unwrap(), "{input:?}");
                assert_eq!(
                    timestamp.offset,
                    case["offset"].as_i64().unwrap() as i32,
                    "{input:?}"
                );
            }
            Err(error) => assert_eq!(error, case["error"].as_str().unwrap(), "{input:?}"),
        }
    }
}
