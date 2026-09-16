package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"net/url"
	"os"
	"runtime"
	"strings"
	"time"

	readability "github.com/markusmobius/go-readabilityV2"
	"golang.org/x/net/html"
)

type page struct {
	File    string   `json:"file"`
	URL     string   `json:"url"`
	HTML    string   `json:"html"`
	With    []string `json:"with"`
	Without []string `json:"without"`
}

type counts struct {
	TruePositives  int `json:"true_positives"`
	FalseNegatives int `json:"false_negatives"`
	FalsePositives int `json:"false_positives"`
	TrueNegatives  int `json:"true_negatives"`
}

func (result *counts) evaluate(text string, input page) {
	for _, snippet := range input.With {
		if text != "" && strings.Contains(text, snippet) {
			result.TruePositives++
		} else {
			result.FalseNegatives++
		}
	}
	for _, snippet := range input.Without {
		if text != "" && strings.Contains(text, snippet) {
			result.FalsePositives++
		} else {
			result.TrueNegatives++
		}
	}
}

type request struct {
	Outputs bool `json:"outputs"`
}

type output struct {
	File     string `json:"file"`
	Text     string `json:"text"`
	HTML     string `json:"html"`
	Title    string `json:"title"`
	Byline   string `json:"byline"`
	Excerpt  string `json:"excerpt"`
	SiteName string `json:"site_name"`
	ImageURL string `json:"image_url"`
	Favicon  string `json:"favicon"`
	Language string `json:"language"`
	Error    string `json:"error"`
}

type response struct {
	ElapsedNS int64    `json:"elapsed_ns"`
	Counts    counts   `json:"counts"`
	Errors    []string `json:"errors"`
	Outputs   []output `json:"outputs"`
}

func run(input string) error {
	runtime.GOMAXPROCS(1)
	file, err := os.Open(input)
	if err != nil {
		return err
	}
	var pages []page
	err = json.NewDecoder(file).Decode(&pages)
	file.Close()
	if err != nil {
		return err
	}
	documents := make([]*html.Node, len(pages))
	urls := make([]*url.URL, len(pages))
	for index, input := range pages {
		documents[index], err = html.Parse(strings.NewReader(input.HTML))
		if err != nil {
			return fmt.Errorf("%s: %w", input.File, err)
		}
		urls[index], err = url.ParseRequestURI(input.URL)
		if err != nil {
			return fmt.Errorf("%s: %w", input.File, err)
		}
	}
	reader := json.NewDecoder(os.Stdin)
	writer := bufio.NewWriter(os.Stdout)
	encoder := json.NewEncoder(writer)
	if err := encoder.Encode(map[string]int{"ready": len(pages)}); err != nil {
		return err
	}
	if err := writer.Flush(); err != nil {
		return err
	}
	for {
		var requested request
		if err := reader.Decode(&requested); err == io.EOF {
			return nil
		} else if err != nil {
			return err
		}
		result := response{Errors: []string{}, Outputs: []output{}}
		started := time.Now()
		for index, input := range pages {
			article, err := readability.FromDocument(documents[index], urls[index])
			var text strings.Builder
			if err == nil {
				err = article.RenderText(&text)
			}
			message := ""
			if err != nil {
				message = err.Error()
				text.Reset()
				result.Errors = append(result.Errors, fmt.Sprintf("%s: %s", input.File, message))
			}
			result.Counts.evaluate(text.String(), input)
			if requested.Outputs {
				var rendered strings.Builder
				if article.Node != nil {
					if err := article.RenderHTML(&rendered); err != nil {
						return fmt.Errorf("%s: %w", input.File, err)
					}
				}
				result.Outputs = append(result.Outputs, output{
					File: input.File, Text: text.String(), HTML: rendered.String(),
					Title: article.Title(), Byline: article.Byline(), Excerpt: article.Excerpt(),
					SiteName: article.SiteName(), ImageURL: article.ImageURL(), Favicon: article.Favicon(),
					Language: article.Language(), Error: message,
				})
			}
		}
		result.ElapsedNS = time.Since(started).Nanoseconds()
		if err := encoder.Encode(result); err != nil {
			return err
		}
		if err := writer.Flush(); err != nil {
			return err
		}
	}
}

func main() {
	if len(os.Args) != 2 {
		panic("usage: benchmark CORPUS_JSON")
	}
	if err := run(os.Args[1]); err != nil {
		panic(err)
	}
}
