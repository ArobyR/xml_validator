//! Translation loading.
//!
//! The bundled languages (`locales/en.json`, `locales/es.json`) are compiled into
//! the binary. In addition, `locales/*.json` files next to the executable and in
//! the working directory are read at startup, so languages can be added (or keys
//! overridden) without recompiling.

use std::collections::HashMap;

const EN_JSON: &str = include_str!("../locales/en.json");
const ES_JSON: &str = include_str!("../locales/es.json");

#[derive(Debug, Clone)]
pub struct Language {
    pub code: String,
    pub name: String,
    pub strings: HashMap<String, String>,
}

fn parse(text: &str) -> HashMap<String, String> {
    serde_json::from_str(text).unwrap_or_default()
}

fn display_name(strings: &HashMap<String, String>, code: &str) -> String {
    strings
        .get("_name")
        .cloned()
        .unwrap_or_else(|| code.to_string())
}

fn locale_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("locales"));
        }
    }
    dirs.push(std::path::PathBuf::from("locales"));
    dirs
}

fn load_languages() -> Vec<Language> {
    let mut languages: HashMap<String, Language> = HashMap::new();
    for (code, json) in [("en", EN_JSON), ("es", ES_JSON)] {
        let strings = parse(json);
        let name = display_name(&strings, code);
        languages.insert(
            code.to_string(),
            Language {
                code: code.to_string(),
                name,
                strings,
            },
        );
    }

    for dir in locale_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(code) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
            else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let strings = parse(&text);
            if strings.is_empty() {
                continue;
            }
            if let Some(existing) = languages.get_mut(&code) {
                existing
                    .strings
                    .extend(strings.iter().map(|(k, v)| (k.clone(), v.clone())));
                existing.name = display_name(&existing.strings, &code);
            } else {
                let name = display_name(&strings, &code);
                languages.insert(
                    code.clone(),
                    Language {
                        code,
                        name,
                        strings,
                    },
                );
            }
        }
    }

    let mut list: Vec<Language> = languages.into_values().collect();
    list.sort_by(|a, b| a.code.cmp(&b.code));
    list
}

pub struct I18n {
    languages: Vec<Language>,
    current: usize,
    english: HashMap<String, String>,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new("en")
    }
}

impl I18n {
    pub fn new(preferred: &str) -> Self {
        let languages = load_languages();
        let current = languages
            .iter()
            .position(|l| l.code == preferred)
            .unwrap_or(0);
        let english = languages
            .iter()
            .find(|l| l.code == "en")
            .map(|l| l.strings.clone())
            .unwrap_or_default();
        Self {
            languages,
            current,
            english,
        }
    }

    pub fn name(&self) -> &str {
        &self.languages[self.current].name
    }

    pub fn index(&self) -> usize {
        self.current
    }

    pub fn names(&self) -> Vec<String> {
        self.languages.iter().map(|l| l.name.clone()).collect()
    }

    pub fn select(&mut self, index: usize) {
        if index < self.languages.len() {
            self.current = index;
        }
    }

    /// Looks up a key in the current language, falling back to English and then
    /// to the key itself.
    pub fn translate(&self, key: &str) -> String {
        self.languages[self.current]
            .strings
            .get(key)
            .or_else(|| self.english.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    /// Like [`I18n::translate`], substituting `{name}` placeholders.
    pub fn translate_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.translate(key);
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_language_is_english() {
        let i18n = I18n::default();
        assert_eq!(i18n.name(), "English");
        assert_eq!(i18n.translate("button.validate"), "Validate");
    }

    #[test]
    fn switching_to_spanish() {
        let mut i18n = I18n::default();
        let names = i18n.names();
        let index = names.iter().position(|n| n == "Español").expect("es");
        i18n.select(index);
        assert_eq!(i18n.name(), "Español");
        assert_eq!(i18n.translate("button.validate"), "Validar");
    }

    #[test]
    fn unknown_keys_return_the_key() {
        let i18n = I18n::default();
        assert_eq!(i18n.translate("does.not.exist"), "does.not.exist");
    }

    #[test]
    fn argument_substitution() {
        let i18n = I18n::default();
        assert_eq!(
            i18n.translate_args("report.errors", &[("n", "3")]),
            "Errors found: 3"
        );
    }
}
