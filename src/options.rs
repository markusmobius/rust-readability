use regex::Regex;

#[derive(Clone, Debug)]
pub struct Options {
    pub max_elems_to_parse: i64,
    pub n_top_candidates: i64,
    pub char_thresholds: i64,
    pub classes_to_preserve: Vec<String>,
    pub keep_classes: bool,
    pub tags_to_score: Vec<String>,
    pub disable_json_ld: bool,
    pub allowed_video_regex: Option<Regex>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_elems_to_parse: 0,
            n_top_candidates: 5,
            char_thresholds: 500,
            classes_to_preserve: vec!["page".into()],
            keep_classes: false,
            tags_to_score: ["section", "h2", "h3", "h4", "h5", "h6", "p", "td", "pre"]
                .map(String::from)
                .into(),
            disable_json_ld: false,
            allowed_video_regex: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Options;

    #[test]
    fn defaults_match_readeck_v2() {
        let options = Options::default();
        assert_eq!(options.max_elems_to_parse, 0);
        assert_eq!(options.n_top_candidates, 5);
        assert_eq!(options.char_thresholds, 500);
        assert_eq!(options.classes_to_preserve, ["page"]);
        assert_eq!(
            options.tags_to_score,
            ["section", "h2", "h3", "h4", "h5", "h6", "p", "td", "pre"]
        );
        assert!(!options.keep_classes);
        assert!(!options.disable_json_ld);
        assert!(options.allowed_video_regex.is_none());
    }
}
