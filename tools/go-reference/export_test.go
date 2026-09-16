package readability

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	shtml "html"
	"io"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/markusmobius/go-readabilityV2/internal/re2go"
	"github.com/go-shiori/dom"
	"github.com/itlightning/dateparse"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
	"golang.org/x/net/html/charset"
)

type referenceNode struct {
	Kind      int              `json:"kind"`
	Data      string           `json:"data"`
	Namespace string           `json:"namespace"`
	Attrs     []html.Attribute `json:"attrs"`
	Depth     int              `json:"depth"`
}

type referenceCase struct {
	ID          string            `json:"id"`
	Source      string            `json:"source"`
	URL         string            `json:"url"`
	Profile     string            `json:"profile"`
	Readable    bool              `json:"readable"`
	Prepared    string            `json:"prepared"`
	Cleaned     string            `json:"cleaned"`
	TitleHelper string            `json:"title_helper"`
	Metadata    map[string]string `json:"metadata"`
	Title       string            `json:"title"`
	Byline      string            `json:"byline"`
	Excerpt     string            `json:"excerpt"`
	SiteName    string            `json:"site_name"`
	Image       string            `json:"image"`
	Favicon     string            `json:"favicon"`
	Language    string            `json:"language"`
	Published   string            `json:"published"`
	Modified    string            `json:"modified"`
	HTML        string            `json:"html"`
	Text        string            `json:"text"`
	Tree        []referenceNode   `json:"tree"`
	Error       string            `json:"error"`
}

func referenceParser(profile string) Parser {
	parser := NewParser()
	switch profile {
	case "keep-classes":
		parser.KeepClasses = true
	case "short":
		parser.CharThresholds = 50
	case "no-jsonld":
		parser.DisableJSONLD = true
	case "custom-classes":
		parser.ClassesToPreserve = []string{"keep", "article"}
	case "max-elements":
		parser.MaxElemsToParse = 8
	case "top-one":
		parser.NTopCandidates = 1
	case "custom-video":
		parser.AllowedVideoRegex = regexp.MustCompile(`(?i)example\.org/player`)
	case "no-score-tags":
		parser.TagsToScore = []string{}
	}
	return parser
}

func TestRustExportReference(test *testing.T) {
	if runtime.Version() != "go1.27.1" {
		test.Fatal("reference requires Go 1.27.1")
	}
	output, err := os.Create(os.Getenv("RUST_READABILITY_OUTPUT"))
	if err != nil {
		test.Fatal(err)
	}
	defer output.Close()
	encoder := json.NewEncoder(output)
	encode := func(id, source, base, profile string) {
		parser := referenceParser(profile)
		document, err := html.Parse(strings.NewReader(source))
		if err != nil {
			test.Fatal(err)
		}
		var pageURL *url.URL
		if base != "" {
			pageURL, err = url.Parse(base)
			if err != nil {
				test.Fatal(err)
			}
		}
		result := referenceCase{ID: id, Source: source, URL: base, Profile: profile, Readable: parser.CheckDocument(document), Tree: []referenceNode{}}
		parser.doc = dom.Clone(document, true)
		parser.documentURI = pageURL
		parser.unwrapNoscriptImages(parser.doc)
		var jsonLD map[string]string
		if !parser.DisableJSONLD {
			jsonLD = parser.getJSONLD()
		}
		parser.removeScripts(parser.doc)
		parser.prepDocument()
		result.Prepared = dom.OuterHTML(parser.doc)
		result.TitleHelper = parser.getArticleTitle()
		result.Metadata = parser.getArticleMetadata(jsonLD)
		cleaned := dom.Clone(parser.doc, true)
		parser.flags = flags{stripUnlikelys: true, useWeightClasses: true, cleanConditionally: true}
		if body := getElementByTagName(cleaned, "body"); body != nil {
			parser.prepArticle(body)
		}
		result.Cleaned = dom.OuterHTML(cleaned)
		before := dom.OuterHTML(document)
		article, err := parser.ParseDocument(document, pageURL)
		if before != dom.OuterHTML(document) {
			test.Fatal("reference mutated caller input", id)
		}
		if err != nil {
			result.Error = err.Error()
		}
		result.Title, result.Byline, result.Excerpt = article.Title(), article.Byline(), article.Excerpt()
		result.SiteName, result.Image, result.Favicon = article.SiteName(), article.ImageURL(), article.Favicon()
		result.Language, result.Published, result.Modified = article.Language(), article.publishedTime, article.modifiedTime
		if article.Node != nil {
			var htmlBuffer, textBuffer bytes.Buffer
			if err := article.RenderHTML(&htmlBuffer); err != nil {
				test.Fatal(err)
			}
			if err := article.RenderText(&textBuffer); err != nil {
				test.Fatal(err)
			}
			result.HTML, result.Text = htmlBuffer.String(), textBuffer.String()
			var visit func(*html.Node, int)
			visit = func(node *html.Node, depth int) {
				result.Tree = append(result.Tree, referenceNode{int(node.Type), node.Data, node.Namespace, node.Attr, depth})
				for child := node.FirstChild; child != nil; child = child.NextSibling {
					visit(child, depth+1)
				}
			}
			visit(article.Node, 0)
		}
		if err := encoder.Encode(result); err != nil {
			test.Fatal(err)
		}
	}
	count := 0
	if os.Getenv("RUST_READABILITY_CORPUS") != "" {
		entries, err := os.ReadDir("test-pages")
		if err != nil {
			test.Fatal(err)
		}
		for _, entry := range entries {
			if !entry.IsDir() {
				continue
			}
			source, err := os.ReadFile(filepath.Join("test-pages", entry.Name(), "source.html"))
			if err != nil {
				test.Fatal(err)
			}
			encode(entry.Name(), string(source), "http://fakehost/test/page.html", "default")
			count++
		}
	} else {
		paragraph := strings.Repeat("An article sentence, with useful context and another detail. ", 12)
		for index, source := range []string{
			"", "<p>Short text.</p>", "plain text without markup",
			"<p>" + strings.Repeat("x", 540) + "</p>", "<p>" + strings.Repeat("x", 541) + "</p>",
			"<p>" + strings.Repeat("\u5b57", 181) + "</p>",
			"<li><p>" + paragraph + "</p></li>",
			"<p hidden>" + paragraph + "</p>",
			"<div>" + paragraph + "<br><br>" + paragraph + "</div>",
			"<html lang='fr'><head><title>Example article | Publisher</title></head><body><div class='article keep' dir='rtl'><h1>Example article</h1><p>" + paragraph + "</p><p>" + paragraph + "</p></div></body></html>",
			"<html><head><title>Example</title><meta property='og:title' content='OG title'><meta name='author' content='Ada Example'><meta property='og:description' content='A summary'><meta property='og:site_name' content='News'><meta property='og:image' content='/photo.jpg'><link rel='icon' href='/icon-64x64.png'></head><body><article><p>" + paragraph + "</p></article></body></html>",
			`<html><head><script type="application/ld+json">{"@context":"https://schema.org","@type":"NewsArticle","headline":"Structured article","author":[{"name":"Ada Example"},{"name":"Bob Example"}],"datePublished":"2020-01-02T03:04:05Z","publisher":{"name":"Publisher"},"image":{"url":"/photo.jpg"}}</script></head><body><article><p>` + paragraph + "</p></article></body></html>",
			"<html><head><title>News</title></head><body><header>Menu</header><div class='byline'>Ada Example</div><div class='content'><p>" + paragraph + "</p><img data-src='/large.jpg' src='tiny.gif'><noscript><img src='/photo.jpg'></noscript><p>" + paragraph + "</p></div><aside class='sidebar'><p>Buy this product</p></aside></body></html>",
			"<article><p>" + paragraph + "<a href='../more?q=1&amp;b=2'>more</a><a href='javascript:alert(1)'>label</a></p><figure><img src='/photo.jpg' srcset='/one.jpg 1x, /two.jpg 2x'><figcaption>Caption</figcaption></figure></article>",
			"<article><p>" + paragraph + "</p><table><caption>Data table</caption><tr><th>One<th>Two<tr><td>A<td>B</table><table><tr><td><p>Layout text</p></table></article>",
			"<article><p>" + paragraph + "</p><iframe src='https://www.youtube.com/embed/1'></iframe><iframe src='https://example.org/player'></iframe><object data='/advert.swf'></object></article>",
			"<article><h2>Heading</h2><p>" + paragraph + "</p><ul><li>First</li><li>Second</li></ul><pre> one\n<br>two<br><br>three </pre></article>",
			"<article><p style='visibility:hidden'>" + paragraph + "</p><p aria-hidden='true' class='fallback-image'>" + paragraph + "</p><div class='comment'><p>" + paragraph + "</p></div></article>",
			"<article><p>" + paragraph + "</p><svg><a xlink:href='/symbol' xml:lang='fr'><text>SVG text</text></a></svg><math><mtext>x+y</mtext></math></article>",
		} {
			for _, profile := range []string{"default", "keep-classes", "short", "no-jsonld", "custom-classes", "max-elements", "top-one", "custom-video", "no-score-tags"} {
				encode(fmt.Sprintf("synthetic-%02d-%s", index, profile), source, "https://example.org/news/page.html", profile)
				count++
			}
		}
	}
	test.Logf("Exported %d exact Go cases", count)
}

func TestRustExportHTML(test *testing.T) {
	output, err := os.Create(os.Getenv("RUST_READABILITY_OUTPUT"))
	if err != nil {
		test.Fatal(err)
	}
	defer output.Close()
	encoder := json.NewEncoder(output)
	count := 0
	root := os.Getenv("RUST_READABILITY_HTML_TESTDATA")
	err = filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() || !strings.HasSuffix(path, ".dat") {
			return nil
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		var source []string
		collect := false
		index := 0
		for _, line := range strings.Split(string(data), "\n") {
			if line == "#data" {
				collect = true
				source = nil
				continue
			}
			if line != "#errors" {
				if collect {
					source = append(source, line)
				}
				continue
			}
			if !collect {
				continue
			}
			collect = false
			input := strings.Join(source, "\n")
			document, err := html.Parse(strings.NewReader(input))
			if err != nil {
				return err
			}
			var nodes []referenceNode
			var atoms []string
			var visit func(*html.Node, int)
			visit = func(node *html.Node, depth int) {
				nodes = append(nodes, referenceNode{int(node.Type), node.Data, node.Namespace, node.Attr, depth})
				atoms = append(atoms, node.DataAtom.String())
				for child := node.FirstChild; child != nil; child = child.NextSibling {
					visit(child, depth+1)
				}
			}
			visit(document, 0)
			relative, err := filepath.Rel(root, path)
			if err != nil {
				return err
			}
			if err := encoder.Encode(map[string]any{"id": fmt.Sprintf("%s:%d", filepath.ToSlash(relative), index), "source": input, "tree": nodes, "atoms": atoms, "html": dom.OuterHTML(document)}); err != nil {
				return err
			}
			index++
			count++
		}
		return nil
	})
	if err != nil {
		test.Fatal(err)
	}
	test.Logf("Exported %d default-document HTML parser cases", count)
}

func TestRustExportHelpers(test *testing.T) {
	previousLocation := time.Local
	time.Local = time.UTC
	test.Cleanup(func() { time.Local = previousLocation })
	result := make(map[string]any)
	var readers []map[string]any
	addReader := func(input []byte) {
		record := map[string]any{"input": input, "html": "", "error": ""}
		document, err := dom.Parse(bytes.NewReader(input))
		if err != nil {
			record["error"] = err.Error()
		} else {
			record["html"] = dom.OuterHTML(dom.Clone(document, true))
		}
		readers = append(readers, record)
	}
	for _, sample := range []struct{ label, text string }{
		{"utf-8", "An article, with useful context and supporting details."},
		{"windows-1252", "Le caf\u00e9 fran\u00e7ais est situ\u00e9 pr\u00e8s du march\u00e9."},
		{"windows-1251", "\u042d\u0442\u043e \u043f\u0440\u0438\u043c\u0435\u0440 \u0440\u0443\u0441\u0441\u043a\u043e\u0433\u043e \u0442\u0435\u043a\u0441\u0442\u0430 \u0434\u043b\u044f \u0441\u0442\u0430\u0442\u044c\u0438."},
		{"koi8-r", "\u042d\u0442\u043e \u043f\u0440\u0438\u043c\u0435\u0440 \u0440\u0443\u0441\u0441\u043a\u043e\u0433\u043e \u0442\u0435\u043a\u0441\u0442\u0430."},
		{"shift_jis", "\u65e5\u672c\u8a9e\u306e\u6587\u7ae0\u3092\u8aad\u3080\u305f\u3081\u306e\u30c6\u30b9\u30c8\u3067\u3059\u3002"},
		{"euc-jp", "\u65e5\u672c\u8a9e\u306e\u6587\u7ae0\u3092\u8aad\u3080\u305f\u3081\u306e\u30c6\u30b9\u30c8\u3067\u3059\u3002"},
		{"iso-2022-jp", "\u65e5\u672c\u8a9e\u306e\u6587\u7ae0\u3092\u8aad\u3080\u305f\u3081\u306e\u30c6\u30b9\u30c8\u3067\u3059\u3002"},
		{"gb18030", "\u8fd9\u662f\u4e00\u4e2a\u4e2d\u6587\u6587\u7ae0\u7684\u6d4b\u8bd5\u3002"},
		{"big5", "\u9019\u662f\u4e00\u500b\u4e2d\u6587\u6587\u7ae0\u7684\u6e2c\u8a66\u3002"},
		{"euc-kr", "\ud55c\uad6d\uc5b4 \uae30\uc0ac\ub97c \uc77d\uae30 \uc704\ud55c \ud14c\uc2a4\ud2b8\uc785\ub2c8\ub2e4."},
		{"utf-16le", "An article with \u00e9 and \u5b57."},
		{"utf-16be", "An article with \u00e9 and \u5b57."},
	} {
		encoding, _ := charset.Lookup(sample.label)
		input, err := encoding.NewEncoder().Bytes([]byte("<html><head><title>Reader</title></head><body><p>" + strings.Repeat(sample.text+" ", 25) + "</p></body></html>"))
		if err != nil {
			test.Fatal(sample.label, err)
		}
		addReader(input)
	}
	for _, text := range []string{"", "plain ASCII", "<p>e\u0301 co\u00adoperate e\u0301 e\u0301 e\u0301</p>", "<p>a" + strings.Repeat("\u0301", 70) + "</p>", "<p>a" + strings.Repeat("\u0301", 25) + "\u00ad" + strings.Repeat("\u0301", 25) + "</p>"} {
		addReader([]byte(text))
	}
	for value := 0; value < 256; value++ {
		addReader([]byte{byte(value)})
	}
	result["readers"] = readers
	var urlCases []map[string]any
	for _, input := range []string{
		"", "*", "relative", "/absolute", "//host/path", "///path", "?q", "#frag", ":missing", "a:b", "1:a", "a_b:c",
		"http://example.org/path", "https://user:pass@example.org:443/a%2fb?q#frag", "HTTP:/a", "http:opaque", "http://",
		"http://host/a?", "http://host/a??", "http://host/a b", "http://host/a%20b", "http://host/%zz", "http://host/%", "http://host/%f",
		"http://host/#%zz", "http://host/\npath", "http://user name@host/x", "http://u%zz@host/x", "http://host:abc/x", "http://host:80:90/x",
		"postgres://host:80,other:90/x", "http://a[b]/", "http://[::1", "http://[::1]:abc", "http://[::1]/", "http://[::ffff:192.0.2.1]/",
		"http://[fe80::1%25zone]/", "http://[fe80::1%25]/", "http://[fe80::1%25z%20x]/", "http://[fe80::1%25z%0Ax]/",
		"http://[bad]/", "http://[]/", "http://[127.0.0.1]/", "http://[1:2:3]/", "http://[1::2::3]/", "http://[1:2:3:4:5:6:7:8:9]/",
		"http://[12345::]/", "http://[::ffff:01.2.3.4]/", "http://[::ffff:256.2.3.4]/", "http://[::ffff:1.2.3]/",
		"http://host%20name/x", "http://h ost/x", "http://host/%C3%A9#x%20y", "http://host/\u00e9#\u00a0", "http://u@bad host/",
	} {
		for _, request := range []bool{false, true} {
			var parsed *url.URL
			var err error
			if request { parsed, err = url.ParseRequestURI(input) } else { parsed, err = url.Parse(input) }
			record := map[string]any{"input": input, "request": request, "error": ""}
			if err != nil { record["error"] = err.Error() } else {
				record["rendered"], record["host"], record["hostname"] = parsed.String(), parsed.Host, parsed.Hostname()
				record["scheme"], record["opaque"], record["path"], record["raw_path"] = parsed.Scheme, parsed.Opaque, []byte(parsed.Path), parsed.RawPath
				record["query"], record["force_query"] = parsed.RawQuery, parsed.ForceQuery
				record["fragment"], record["raw_fragment"] = []byte(parsed.Fragment), parsed.RawFragment
			}
			urlCases = append(urlCases, record)
		}
	}
	result["urls"] = urlCases
	var scalarCases []map[string]any
	for _, word := range []string{"hid", "hidden", "d-none", "author", "article", "comment", "main", "share", "hidhidden", "HIDDEN"} {
		for _, prefix := range []string{"", " ", "x", "-", "\n", "\u00a0", "\x00"} {
			for _, suffix := range []string{"", " ", "x", "-", "\n", "\u00a0", "\x00"} {
				input := prefix + word + suffix
				scalarCases = append(scalarCases, map[string]any{"input": input, "negative": re2go.IsNegativeClass(input), "positive": re2go.IsPositiveClass(input), "byline": re2go.IsByline(input), "normalize": normalizeWhitespace(input)})
			}
		}
	}
	result["strings"] = scalarCases
	var entities []map[string]string
	for _, input := range []string{"&amp;lt; &notit; &NotEqualTilde;", "&#x; &#; &#x &#0; &#55296;", "&#4294967361; &#2147483648;", "&notin &notin; &notit; &CounterClockwiseContourIntegral;", "&#1 &#12 &#x1 &#x12", "&amp= &copyx; &unknown;", "&#9999999999999999999999999999999999999;"} {
		entities = append(entities, map[string]string{"input": input, "output": shtml.UnescapeString(input)})
	}
	result["entities"] = entities
	var sorts []map[string]any
	for _, length := range []int{0, 1, 7, 12, 13, 49, 50, 99, 257, 1024} {
		for mode := 0; mode < 6; mode++ {
			keys, indices := make([]int, length), make([]int, length)
			for index := range keys {
				indices[index] = index
				switch mode {
				case 0:
					keys[index] = index
				case 1:
					keys[index] = length - index
				case 2:
					keys[index] = 0
				case 3:
					keys[index] = (index*37 + index/7) % 11
				case 4:
					keys[index] = min(index, length-index)
				case 5:
					keys[index] = index % 3
				}
			}
			sort.Slice(indices, func(first, second int) bool { return keys[indices[first]] < keys[indices[second]] })
			sorts = append(sorts, map[string]any{"keys": keys, "indices": indices})
		}
	}
	result["sorts"] = sorts
	var jsonCases []map[string]any
	for _, input := range []string{
		`{"@context":"https://schema.org","@type":"Article","description":"\ud800"}`,
		`{"@context":"https://schema.org","@type":"Article","description":"\udc00"}`,
		`{"@context":"https://schema.org","@type":"Article","description":"\ud83d\ude00"}`,
		`{"@context":"https://schema.org","@type":"Article","description":"\ud800x"}`,
		`{"@context":"https://schema.org","@type":"Article","description":"\ud800\ud800"}`,
		`{"@context":"https://schema.org","@type":"Article","name":"one","name":"two"}`,
		`{"@context":{"@vocab":"http://schema.org/"},"@graph":[{"@type":"Article","headline":"from graph"}]}`,
		`{"@context":"https://schema.org","@type":null,"@graph":[{"@type":"Article","headline":"ignored"}]}`,
		`{"@context":"https://schema.org","@type":"Article","extra":` + strings.Repeat("[", 150) + "0" + strings.Repeat("]", 150) + `,"headline":"deep JSON"}`,
		`{"@context":"https://schema.org","@type":"Article","description":"bad\xescape"}`,
	} {
		parser := NewParser()
		parser.doc, _ = html.Parse(strings.NewReader(`<title>Fallback title</title><script type="application/ld+json">` + input + `</script>`))
		jsonCases = append(jsonCases, map[string]any{"input": input, "metadata": parser.getJSONLD()})
	}
	result["json_ld"] = jsonCases
	paragraph := strings.Repeat("An article sentence, with useful context and another detail. ", 12)
	apiSources := []string{
		"<html lang='fr'><head><title>First article</title></head><body><article><font class='keep'>" + paragraph + "</font><p>" + paragraph + "</p></article></body></html>",
		"<html><head><title>Second article</title></head><body><div class='byline'>Ada Example</div><article><p>" + paragraph + "</p><div>" + paragraph + "<br><br>" + paragraph + "</div></article></body></html>",
		"", "<html lang=''><body><article><p>" + paragraph + "</p></article></body></html>",
		"<html lang='de'><body><article><p>" + paragraph + "</p><svg><a xlink:href='/x'><text>label</text></a></svg></article></body></html>",
	}
	var apiCases []map[string]any
	for _, mode := range []string{"clone", "mutate", "repeat", "body", "article"} {
		parser := NewParser()
		for _, source := range apiSources {
			document, err := html.Parse(strings.NewReader(source))
			if err != nil {
				test.Fatal(err)
			}
			if mode == "body" || mode == "article" {
				document = getElementByTagName(document, mode)
				if document == nil {
					continue
				}
			}
			for pass := 0; pass < 2; pass++ {
				if mode != "repeat" && pass > 0 {
					break
				}
				before := dom.OuterHTML(document)
				var article Article
				if mode == "clone" || mode == "body" || mode == "article" {
					article, err = parser.ParseDocument(document, nil)
				} else {
					article, err = parser.ParseAndMutate(document, nil)
				}
				var htmlBuffer, textBuffer bytes.Buffer
				htmlErr, textErr := article.RenderHTML(&htmlBuffer), article.RenderText(&textBuffer)
				record := map[string]any{"mode": mode, "source": source, "pass": pass, "before": before, "after": dom.OuterHTML(document), "title": article.Title(), "byline": article.Byline(), "language": article.Language(), "excerpt": article.Excerpt(), "html": htmlBuffer.String(), "text": textBuffer.String(), "error": "", "html_error": "", "text_error": ""}
				if err != nil {
					record["error"] = err.Error()
				}
				if htmlErr != nil {
					record["html_error"] = htmlErr.Error()
				}
				if textErr != nil {
					record["text_error"] = textErr.Error()
				}
				apiCases = append(apiCases, record)
			}
		}
	}
	result["parser_api"] = apiCases
	var renderingCases []map[string]any
	roots := []*html.Node{}
	for _, data := range []string{"", "plain text", "<&>\"'\r", "-->", "<!--nested-->", "<!-->", "<!--->", "--!>", "<script>"} {
		for _, kind := range []html.NodeType{html.TextNode, html.CommentNode, html.DoctypeNode} {
			roots = append(roots, &html.Node{Type: kind, Data: data})
		}
	}
	for _, public := range []string{"", "identifier", "a>b", "a\"b", "a'\"b"} {
		for _, system := range []string{"", "identifier", "a>b", "a\"b", "a'\"b"} {
			roots = append(roots, &html.Node{Type: html.DoctypeNode, Data: "html", Attr: []html.Attribute{{Key: "public", Val: public}, {Key: "system", Val: system}}})
		}
	}
	for _, tag := range []string{"div", "img", "br", "pre", "listing", "textarea", "script", "style", "noscript", "plaintext"} {
		for _, childKind := range []html.NodeType{html.TextNode, html.CommentNode, html.ElementNode} {
			root := &html.Node{Type: html.ElementNode, Data: tag, Attr: []html.Attribute{{Namespace: "xml", Key: "lang", Val: "<&>\"'\r"}}}
			child := &html.Node{Type: childKind, Data: "\n<&>\"'\r"}
			root.AppendChild(child)
			child.AppendChild(&html.Node{Type: html.TextNode, Data: "child text"})
			roots = append(roots, root)
		}
	}
	for _, namespace := range []string{"", "svg", "math"} {
		for _, tag := range []string{"div", "script", "noscript", "plaintext", "foreignObject", "desc", "title", "annotation-xml"} {
			for _, encoding := range []string{"", "text/html", "APPLICATION/XHTML+XML"} {
				root := &html.Node{Type: html.ElementNode, Data: tag, DataAtom: atom.Lookup([]byte(tag)), Namespace: namespace, Attr: []html.Attribute{{Key: "encoding", Val: encoding}}}
				child := &html.Node{Type: html.ElementNode, Data: "script", DataAtom: atom.Script}
				child.AppendChild(&html.Node{Type: html.TextNode, Data: "<&>\"'\r"})
				root.AppendChild(child)
				root.AppendChild(&html.Node{Type: html.TextNode, Data: "<&>\"'\r"})
				roots = append(roots, root)
			}
		}
	}
	for _, root := range roots {
		var output bytes.Buffer
		err := (Article{Node: root}).RenderHTML(&output)
		var nodes []referenceNode
		var atoms []string
		var visit func(*html.Node, int)
		visit = func(node *html.Node, depth int) {
			nodes = append(nodes, referenceNode{int(node.Type), node.Data, node.Namespace, node.Attr, depth})
			atoms = append(atoms, node.DataAtom.String())
			for child := node.FirstChild; child != nil; child = child.NextSibling {
				visit(child, depth+1)
			}
		}
		visit(root, 0)
		record := map[string]any{"tree": nodes, "atoms": atoms, "html": output.String(), "error": ""}
		if err != nil {
			record["error"] = err.Error()
		}
		renderingCases = append(renderingCases, record)
	}
	result["render_nodes"] = renderingCases
	var parsingCases []map[string]any
	for _, source := range []string{
		"", "text", "<!DOCTYPE html PUBLIC 'a>b' 'x>y'><p>body", "<!--&amp;--><!-->--><!--a-->b",
		"<svg><script>&lt;a>&amp;b</script><style>&lt;z</style></svg>",
		"<svg><foreignObject><script>&lt;&amp;</script></foreignObject><desc><style>&lt;&amp;</style></desc></svg>",
		"<math><annotation-xml encoding='text/html'><script>&lt;&amp;</script></annotation-xml></math>",
		"<math><annotation-xml encoding='application/xml'><script>&lt;&amp;</script></annotation-xml></math>",
		"<svg><foreignObject><table><script>&lt;&amp;</script></table></foreignObject></svg>",
		"<math><mtext><table><style>&lt;&amp;</style></table></mtext></math>",
		"<p><b z=1 a=2><b a=2 z=1><b z=1 a=2><b a=2 z=1>one</p>two",
		"<p><a z=1 a=2><table><a a=2 z=1>one</table>two", "<table><b z=1 a=2>one<tr><td>two</b>three",
		"<template><p>one<template><div>two</template>three</template>",
		"<table><template><tr><td>one</template></table>", "<template><html lang=fr><head><title>one</title><body>two</template>",
		"<select><div><option>one</option></div></select>", "<select><button><selectedcontent></selectedcontent></button><option>one</select>",
		"<table><select><option>one<tr><td>two", "<select><hr><option>one<option>two</select>",
		"<pre>\nfirst\r\nsecond\x00third</pre>", "<svg><image xlink:href='/x' xmlns:xlink='x' xml:lang='fr' viewbox='0 0 1 1'></svg>",
		"<math definitionurl='x'><mi>a<mglyph>z</mglyph><b>b</mi></math>",
		"<p><nobr z=1 a=2><nobr a=2 z=1>nested</p>end", "<noscript><p>one</noscript><p>two",
		"<p><image src=x><keygen><isindex prompt='x'><param><source><track>", "<listing>\ntext</listing><textarea>\ntext</textarea>",
		"<p>one<plaintext><b>literal</p>text", "<svg><title><p>one</p></title></svg>", "<p a=1 a=2 A=3 b=4>B",
		"<!DOCTYPE &gt; PUBLIC \"&gt;\" \"&gt;\">", "<svg xml:base xml:lang xml:space xml:baaah definitionurl>",
		"<math><template><mo><template>", "<math><template><mo><template><p>ignored</p></template></mo></math><p>ignored",
		"<svg><foreignObject><template><p>ignored</p></template></foreignObject></svg><p>ignored",
		"<svg><foreignObject><p>inside</p></foreignObject></svg><template><p>outside</p></template>",
		"<!DOCTYPE html PUBLIC '' ''><p>a<table><tr><td>b", "<!DOCTYPE html SYSTEM ''><p>a<table><tr><td>b",
		"<!DOCTYPE h&#116;ml><p>a<table><tr><td>b", "<!DOCTYPE HTML><p>a<table><tr><td>b",
		"<!DOCTYPE html PUBLIC \"&quot;x\" \"tail\"><p>a<table><tr><td>b",
		"<!DOCTYPE html PUBLIC '&apos;x' 'tail'><p>a<table><tr><td>b",
		"<!DOCTYPE html SYSTEM \"&quot;x\"><p>a<table><tr><td>b", "<!DOCTYPE html PUBLIC '&gt;'>",
	} {
		document, err := html.Parse(strings.NewReader(source))
		if err != nil {
			test.Fatal(err)
		}
		var nodes []referenceNode
		var atoms []string
		var visit func(*html.Node, int)
		visit = func(node *html.Node, depth int) {
			nodes = append(nodes, referenceNode{int(node.Type), node.Data, node.Namespace, node.Attr, depth})
			atoms = append(atoms, node.DataAtom.String())
			for child := node.FirstChild; child != nil; child = child.NextSibling {
				visit(child, depth+1)
			}
		}
		visit(document, 0)
		parsingCases = append(parsingCases, map[string]any{"source": source, "tree": nodes, "atoms": atoms, "html": dom.OuterHTML(document)})
	}
	result["parse_nodes"] = parsingCases
	atomSource, err := parser.ParseFile(token.NewFileSet(), os.Getenv("RUST_READABILITY_ATOM_SOURCE"), nil, 0)
	if err != nil {
		test.Fatal(err)
	}
	var atomNames []string
	ast.Inspect(atomSource, func(node ast.Node) bool {
		declaration, ok := node.(*ast.ValueSpec)
		if !ok {
			return true
		}
		kind, ok := declaration.Type.(*ast.Ident)
		if !ok || kind.Name != "Atom" {
			return true
		}
		for _, expression := range declaration.Values {
			literal, ok := expression.(*ast.BasicLit)
			if !ok {
				test.Fatal("unexpected atom value")
			}
			value, err := strconv.ParseUint(literal.Value, 0, 32)
			if err != nil {
				test.Fatal(err)
			}
			atomNames = append(atomNames, atom.Atom(value).String())
		}
		return true
	})
	sort.Strings(atomNames)
	atomJSON, err := json.MarshalIndent(atomNames, "", "  ")
	if err != nil {
		test.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("RUST_READABILITY_ATOMS"), append(atomJSON, '\n'), 0600); err != nil {
		test.Fatal(err)
	}
	dateSource, err := parser.ParseFile(token.NewFileSet(), os.Getenv("RUST_READABILITY_DATE_TESTS"), nil, 0)
	if err != nil {
		test.Fatal(err)
	}
	dateInputs := []string{"", "2020-01-02T03:04:05Z", "31/03/2023", "garbage", strings.Repeat("x", 79)}
	ast.Inspect(dateSource, func(node ast.Node) bool {
		pair, ok := node.(*ast.KeyValueExpr)
		if !ok {
			return true
		}
		key, ok := pair.Key.(*ast.Ident)
		if !ok || key.Name != "in" {
			return true
		}
		value, ok := pair.Value.(*ast.BasicLit)
		if !ok || value.Kind != token.STRING {
			return true
		}
		input, err := strconv.Unquote(value.Value)
		if err != nil {
			test.Fatal(err)
		}
		dateInputs = append(dateInputs, input)
		return true
	})
	var dates []map[string]any
	for _, input := range dateInputs {
		article := Article{publishedTime: input}
		timestamp, err := article.PublishedTime()
		zone, offset := timestamp.Zone()
		layout, layoutErr := dateparse.ParseFormat(input)
		record := map[string]any{"input": input, "seconds": timestamp.Unix(), "nanoseconds": timestamp.Nanosecond(), "zone": zone, "offset": offset, "error": "", "layout": layout, "layout_error": ""}
		if err != nil {
			record["error"] = err.Error()
		}
		if layoutErr != nil {
			record["layout_error"] = layoutErr.Error()
		}
		dates = append(dates, record)
	}
	result["dates"] = dates
	zoneArchive, err := zip.OpenReader(filepath.Join(runtime.GOROOT(), "lib", "time", "zoneinfo.zip"))
	if err != nil {
		test.Fatal(err)
	}
	defer zoneArchive.Close()
	var timezones []map[string]any
	for _, zoneName := range []string{"America/New_York", "Europe/Berlin", "Australia/Sydney", "Asia/Kathmandu"} {
		var data []byte
		for _, entry := range zoneArchive.File {
			if entry.Name != zoneName {
				continue
			}
			reader, err := entry.Open()
			if err != nil {
				test.Fatal(err)
			}
			data, err = io.ReadAll(reader)
			reader.Close()
			if err != nil {
				test.Fatal(err)
			}
		}
		location, err := time.LoadLocationFromTZData(zoneName, data)
		if err != nil {
			test.Fatal(err)
		}
		time.Local = location
		var records []map[string]any
		for _, input := range dateInputs {
			timestamp, err := dateparse.ParseAny(input)
			zone, offset := timestamp.Zone()
			record := map[string]any{"input": input, "seconds": timestamp.Unix(), "nanoseconds": timestamp.Nanosecond(), "zone": zone, "offset": offset, "error": ""}
			if err != nil {
				record["error"] = err.Error()
			}
			records = append(records, record)
		}
		timezones = append(timezones, map[string]any{"name": zoneName, "data": data, "dates": records})
	}
	time.Local = time.UTC
	result["timezones"] = timezones
	abbreviationSource, err := parser.ParseFile(token.NewFileSet(), filepath.Join(runtime.GOROOT(), "src", "time", "zoneinfo_abbrs_windows.go"), nil, 0)
	if err != nil {
		test.Fatal(err)
	}
	abbreviations := make(map[string][]string)
	ast.Inspect(abbreviationSource, func(node ast.Node) bool {
		pair, ok := node.(*ast.KeyValueExpr)
		if !ok {
			return true
		}
		key := pair.Key.(*ast.BasicLit)
		name, err := strconv.Unquote(key.Value)
		if err != nil {
			test.Fatal(err)
		}
		for _, entry := range pair.Value.(*ast.CompositeLit).Elts {
			value, err := strconv.Unquote(entry.(*ast.BasicLit).Value)
			if err != nil {
				test.Fatal(err)
			}
			abbreviations[name] = append(abbreviations[name], value)
		}
		return true
	})
	abbreviationJSON, err := json.MarshalIndent(abbreviations, "", "  ")
	if err != nil {
		test.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("RUST_READABILITY_ABBREVIATIONS"), append(abbreviationJSON, '\n'), 0600); err != nil {
		test.Fatal(err)
	}
	if destination := os.Getenv("RUST_READABILITY_NATIVE_TIMEZONE"); destination != "" {
		time.Local = previousLocation
		var records []map[string]any
		for _, input := range append(append([]string{}, dateInputs...), "1900-01-02 03:04:05 EST", "1900-07-02 03:04:05 EDT", "9999-07-02 03:04:05 EDT", "9999-01-02 03:04:05 EST") {
			timestamp, err := dateparse.ParseAny(input)
			zone, offset := timestamp.Zone()
			record := map[string]any{"input": input, "seconds": timestamp.Unix(), "nanoseconds": timestamp.Nanosecond(), "zone": zone, "offset": offset, "error": ""}
			if err != nil {
				record["error"] = err.Error()
			}
			records = append(records, record)
		}
		data, err := json.Marshal(map[string]any{"goos": runtime.GOOS, "year": time.Now().UTC().Year(), "dates": records})
		if err != nil {
			test.Fatal(err)
		}
		if err := os.WriteFile(destination, append(data, '\n'), 0600); err != nil {
			test.Fatal(err)
		}
		time.Local = time.UTC
	}
	var layouts []map[string]any
	for _, layout := range []string{"2006-01-02T15:04:05Z07:00", "2006-01-02", "01/02/2006", "January 2, 2006", "Jan 2, 2006 3:4:5pm", "Mon, 02 Jan 2006 15:04:05 MST", "2006-01-02 15:04:05.000 -0700", "2006", "060102", "20060102150405"} {
		for _, input := range []string{"2020-01-02T03:04:05Z", "2020-02-30", "2020-01-02", "03/31/2023", "31/03/2023", "January 2, 2020", "Jan 2, 2020 3:4:5pm", "Thu, 02 Jan 2020 03:04:05 XYZ", "2020-01-02 03:04:05.123 +0530", "2020", "200102", "20200102030405", "", "garbage"} {
			timestamp, err := time.Parse(layout, input)
			zone, offset := timestamp.Zone()
			record := map[string]any{"layout": layout, "input": input, "seconds": timestamp.Unix(), "nanoseconds": timestamp.Nanosecond(), "zone": zone, "offset": offset, "error": ""}
			if err != nil {
				record["error"] = err.Error()
			}
			layouts = append(layouts, record)
		}
	}
	result["layouts"] = layouts
	output, err := os.Create(os.Getenv("RUST_READABILITY_HELPERS"))
	if err != nil {
		test.Fatal(err)
	}
	defer output.Close()
	if err := json.NewEncoder(output).Encode(result); err != nil {
		test.Fatal(err)
	}
	test.Logf("Exported %d readers, %d scalar cases, %d entities, %d sorts, %d JSON-LD cases, %d timestamps, %d layouts", len(readers), len(scalarCases), len(entities), len(sorts), len(jsonCases), len(dates), len(layouts))
}
