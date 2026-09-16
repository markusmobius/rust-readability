package main

import "testing"

func TestSnippetScoring(test *testing.T) {
	input := page{
		With:    []string{"article", "missing", "article", "Article"},
		Without: []string{"navigation", "footer"},
	}
	var result counts
	result.evaluate("article navigation", input)
	expected := counts{TruePositives: 2, FalseNegatives: 2, FalsePositives: 1, TrueNegatives: 1}
	if result != expected {
		test.Fatalf("unexpected snippet counts: got %+v, want %+v", result, expected)
	}
	result.evaluate("", input)
	if result.FalseNegatives != 6 || result.TrueNegatives != 3 {
		test.Fatalf("empty output was not scored: %+v", result)
	}
}
