use crate::{
    dom::Tree, encoding, extract::grab_article, metadata, parse_dom, postprocess, prepare, Article,
    Document, Error, Options, Url,
};
use std::io::Read;

#[derive(Clone, Debug, Default)]
pub struct Parser {
    pub options: Options,
    language: String,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: Options) -> Self {
        Self {
            options,
            language: String::new(),
        }
    }

    pub fn check_document(&self, document: &Document) -> bool {
        crate::check_document(document)
    }

    pub fn parse(&mut self, input: &str, page_url: Option<&Url>) -> Result<Article, Error> {
        self.parse_tree(&mut parse_dom(input), page_url)
    }

    pub fn parse_reader(
        &mut self,
        mut input: impl Read,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|error| Error::ParseInput(Box::new(Error::Io(error))))?;
        let decoded =
            encoding::decode(&bytes).map_err(|error| Error::ParseInput(Box::new(error)))?;
        self.parse(&decoded, page_url)
    }

    pub fn parse_document(
        &mut self,
        document: &Document,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        self.parse_cloned(|| Tree::new(document.clone()), page_url)
    }

    pub fn parse_dom(&mut self, document: &Tree, page_url: Option<&Url>) -> Result<Article, Error> {
        self.parse_cloned(|| document.clone_for_readability(), page_url)
    }

    pub fn parse_shared_document(
        &mut self,
        document: &impl crate::DomSource,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        self.parse_dom(&document.copy_for_readability(), page_url)
    }

    pub fn parse_dom_and_mutate(
        &mut self,
        document: &mut Tree,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        self.parse_tree(document, page_url)
    }

    pub fn parse_and_mutate(
        &mut self,
        document: &mut Document,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        struct Restore<'input> {
            document: &'input mut Document,
            tree: Tree,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                *self.document =
                    std::mem::replace(&mut self.tree.document, Document { nodes: Vec::new() });
            }
        }
        let tree = Tree::new(std::mem::replace(document, Document { nodes: Vec::new() }));
        let mut restore = Restore { document, tree };
        self.parse_tree(&mut restore.tree, page_url)
    }

    fn prepare_tree(
        &self,
        tree: &mut Tree,
        page_url: Option<&Url>,
    ) -> Result<metadata::Metadata, Error> {
        if self.options.max_elems_to_parse > 0 {
            let count = tree.elements(0).len();
            if count as i64 > self.options.max_elems_to_parse {
                return Err(Error::DocumentTooLarge(count));
            }
        }
        prepare::unwrap_noscript_images(tree);
        let json_ld = if self.options.disable_json_ld {
            metadata::Metadata::new()
        } else {
            metadata::json_ld(tree)
        };
        prepare::remove_scripts(tree);
        prepare::prepare_document(tree);
        Ok(metadata::article_metadata(tree, &json_ld, page_url))
    }

    fn parse_tree(&mut self, tree: &mut Tree, page_url: Option<&Url>) -> Result<Article, Error> {
        let metadata = self.prepare_tree(tree, page_url)?;
        self.extract_article(|| tree.clone_for_readability(), metadata, page_url)
    }

    fn parse_cloned(
        &mut self,
        mut clone: impl FnMut() -> Tree,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        let mut tree = clone();
        let metadata = self.prepare_tree(&mut tree, page_url)?;
        tree.namespaces.fill(crate::Text::new());
        let mut first = Some(tree);
        self.extract_article(
            || {
                first.take().unwrap_or_else(|| {
                    let mut tree = clone();
                    prepare::unwrap_noscript_images(&mut tree);
                    prepare::remove_scripts(&mut tree);
                    prepare::prepare_document(&mut tree);
                    tree.namespaces.fill(crate::Text::new());
                    tree
                })
            },
            metadata,
            page_url,
        )
    }

    fn extract_article(
        &mut self,
        next_tree: impl FnMut() -> Tree,
        mut metadata: metadata::Metadata,
        page_url: Option<&Url>,
    ) -> Result<Article, Error> {
        let title = metadata["title"].clone();
        let mut byline = metadata["byline"].clone();
        let extracted = grab_article(
            next_tree,
            &self.options,
            &title,
            &mut byline,
            &mut self.language,
        );
        metadata.insert("byline".into(), byline);
        let (document, node) = if let Some((mut tree, root)) = extracted {
            postprocess::process(&mut tree, root, &self.options, page_url);
            let node = tree.children_elements(root).first().copied();
            (tree, node)
        } else {
            (Tree::new(Document { nodes: Vec::new() }), None)
        };
        Ok(Article {
            document,
            node,
            metadata,
            language: self.language.clone(),
        })
    }
}

pub fn from_html(input: &str, page_url: Option<&Url>) -> Result<Article, Error> {
    Parser::new().parse(input, page_url)
}

pub fn from_document(document: &Document, page_url: Option<&Url>) -> Result<Article, Error> {
    Parser::new().parse_document(document, page_url)
}

pub fn from_reader(input: impl Read, page_url: Option<&Url>) -> Result<Article, Error> {
    Parser::new().parse_reader(input, page_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_document_imports_once_across_retries() {
        use crate::DomSource;

        struct Source {
            document: crate::Dom,
            copies: std::cell::Cell<usize>,
        }

        impl DomSource for Source {
            fn node_count(&self) -> usize {
                self.document.node_count()
            }
            fn kind(&self, node: crate::NodeId) -> crate::Kind {
                self.document.kind(node)
            }
            fn tag(&self, node: crate::NodeId) -> &str {
                self.document.tag(node)
            }
            fn data(&self, node: crate::NodeId) -> &str {
                self.document.data(node)
            }
            fn parent(&self, node: crate::NodeId) -> Option<crate::NodeId> {
                self.document.parent(node)
            }
            fn children(&self, node: crate::NodeId) -> &[crate::NodeId] {
                self.document.children(node)
            }
            fn attribute_count(&self, node: crate::NodeId) -> usize {
                self.document.attribute_count(node)
            }
            fn attribute(&self, node: crate::NodeId, attribute: usize) -> (&str, &str, &str) {
                self.document.attribute(node, attribute)
            }
            fn copy_for_readability(&self) -> crate::Dom {
                self.copies.set(self.copies.get() + 1);
                self.document.clone()
            }
        }

        let source = Source {
            document: parse_dom("<title>Short article</title><article><p>A short article with useful evidence, details, and context.</p></article>"),
            copies: std::cell::Cell::new(0),
        };
        let snapshot = source.document.clone();
        let options = Options {
            char_thresholds: i64::MAX,
            ..Default::default()
        };
        let expected = Parser::with_options(options.clone())
            .parse_dom(&source.document, None)
            .unwrap();
        let actual = Parser::with_options(options)
            .parse_shared_document(&source, None)
            .unwrap();
        assert_eq!(source.copies.get(), 1);
        assert_eq!(actual.document, expected.document);
        assert_eq!(actual.node, expected.node);
        assert_eq!(actual.metadata, expected.metadata);
        assert_eq!(actual.language, expected.language);
        assert_eq!(source.document, snapshot);
    }

    #[test]
    fn shared_document_preserves_reader_decoding_and_input() {
        let mut source = b"<html lang='fr'><meta charset='windows-1252'><title>Shared document</title><body><article><p>".to_vec();
        for _ in 0..20 {
            source.extend_from_slice(
                b"An article with caf\xe9, useful details, and several complete sentences. ",
            );
        }
        source.extend_from_slice(b"</p></article></body></html>");
        let document = crate::parse_bytes(&source).unwrap();
        let snapshot = document.clone();
        let expected = Parser::new().parse_reader(source.as_slice(), None).unwrap();
        for _ in 0..2 {
            let actual = Parser::new()
                .parse_shared_document(&document, None)
                .unwrap();
            assert_eq!(actual.text().unwrap(), expected.text().unwrap());
            assert_eq!(actual.html().unwrap(), expected.html().unwrap());
            assert_eq!(actual.metadata, expected.metadata);
            assert_eq!(actual.language, expected.language);
            assert_eq!(document, snapshot);
        }
    }

    #[test]
    fn owned_parse_preserves_retries_and_input() {
        let source = parse_dom(&format!(
            "<html lang='en'><title>Article title</title><body><script>removed()</script><style>p {{color:red}}</style><article><font>heading</font><img><noscript><img src='photo.jpg'></noscript><p>{}</p><br><br>trailing text<svg><title>image title</title></svg></article></body></html>",
            "An article sentence, with useful context and another detail. ".repeat(12),
        ));
        let original = source.clone();
        for char_thresholds in [0, 500, i64::MAX] {
            for disable_json_ld in [false, true] {
                let options = Options {
                    char_thresholds,
                    disable_json_ld,
                    ..Options::default()
                };
                let mut prepared = source.clone_for_readability();
                let expected = Parser::with_options(options.clone())
                    .parse_dom_and_mutate(&mut prepared, None)
                    .unwrap();
                let actual = Parser::with_options(options.clone())
                    .parse_dom(&source, None)
                    .unwrap();
                assert_eq!(actual.document, expected.document);
                assert_eq!(actual.node, expected.node);
                assert_eq!(actual.metadata, expected.metadata);
                assert_eq!(actual.language, expected.language);
                assert_eq!(source, original);

                let mut prepared = source.document.clone();
                let expected = Parser::with_options(options.clone())
                    .parse_and_mutate(&mut prepared, None)
                    .unwrap();
                let actual = Parser::with_options(options)
                    .parse_document(&source.document, None)
                    .unwrap();
                assert_eq!(actual.document, expected.document);
                assert_eq!(actual.node, expected.node);
                assert_eq!(actual.metadata, expected.metadata);
                assert_eq!(actual.language, expected.language);
                assert_eq!(source, original);
            }
        }
    }

    #[test]
    fn readability_check_is_independent_of_parser_options() {
        let parser = Parser::with_options(Options {
            max_elems_to_parse: 1,
            n_top_candidates: -1,
            char_thresholds: i64::MAX,
            ..Options::default()
        });
        for (source, expected) in [
            ("Short text".to_owned(), false),
            (
                "An article sentence, with useful context. ".repeat(30),
                true,
            ),
        ] {
            let document = parse_dom(&format!("<article><p>{source}</p></article>"));
            assert_eq!(parser.check_document(&document), expected);
        }
    }

    #[test]
    fn reader_normalization_is_separate_from_dom_parsing() {
        let text =
            "This is a detailed article, with context and supporting explanations. ".repeat(12);
        let source = format!(
            "<article><p>{text} e\u{301} e\u{301} e\u{301} e\u{301} co\u{ad}operate.</p></article>"
        );
        let direct = from_html(&source, None).unwrap().text().unwrap();
        let decoded = from_reader(source.as_bytes(), None)
            .unwrap()
            .text()
            .unwrap();
        assert!(direct.contains("e\u{301}"));
        assert!(direct.contains("co\u{ad}operate"));
        assert!(decoded.contains("\u{e9} \u{e9} \u{e9} \u{e9}"));
        assert!(decoded.contains("cooperate"));
    }

    #[test]
    fn reader_errors_keep_the_go_prefix() {
        struct FailedReader;
        impl Read for FailedReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("reader failure"))
            }
        }
        assert_eq!(
            from_reader(FailedReader, None).unwrap_err().to_string(),
            "failed to parse input: reader failure"
        );
    }

    #[test]
    fn mutating_document_is_restored_after_unwind() {
        let source = format!(
            "<script>removed()</script><article><font>heading</font><p>{}</p></article>",
            "An article sentence, with useful context and another detail. ".repeat(12)
        );
        let mut native = parse_dom(&source);
        let mut shared = native.document.clone();
        let original = shared.clone();
        let options = Options {
            n_top_candidates: -1,
            ..Options::default()
        };
        let native_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Parser::with_options(options.clone()).parse_dom_and_mutate(&mut native, None)
        }));
        let shared_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Parser::with_options(options).parse_and_mutate(&mut shared, None)
        }));
        assert!(native_panic.is_err());
        assert!(shared_panic.is_err());
        assert_eq!(shared, native.document);
        assert_ne!(shared, original);
    }
}
