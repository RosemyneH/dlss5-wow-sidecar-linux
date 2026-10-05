const KNOWN_TAGS: &[&str] = &[
    "en", "ru", "es", "de", "fr", "tr", "ar", "zh", "ja", "ko",
];

pub fn parse_language_tag(tag: &str) -> bool {
    KNOWN_TAGS.contains(&tag)
}
