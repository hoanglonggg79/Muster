use std::collections::BTreeMap;
use std::path::Path;

const LEGACY_SECTION: &str = "RBX Alt Manager";
const LEGACY_SECTION_REPLACEMENT: &str = "Roblox Account Manager";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IniFile {
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl IniFile {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut current: Option<String> = None;

        for raw_line in text.lines() {
            let line = raw_line.trim();

            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if let Some(inner) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                let name = normalize_section_name(inner.trim());
                sections.entry(name.clone()).or_default();
                current = Some(name);
                continue;
            }

            let Some(section) = current.as_ref() else {
                continue;
            };

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();

            if key.is_empty() || value.is_empty() {
                continue;
            }

            sections
                .entry(section.clone())
                .or_default()
                .insert(key.to_owned(), value.to_owned());
        }

        Self { sections }
    }

    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    #[must_use]
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(section)
            .and_then(|entries| entries.get(key))
            .map(String::as_str)
    }

    #[must_use]
    pub fn section(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.sections.get(name)
    }

    pub fn sections(&self) -> impl Iterator<Item = (&str, &BTreeMap<String, String>)> {
        self.sections
            .iter()
            .map(|(name, entries)| (name.as_str(), entries))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.values().all(BTreeMap::is_empty)
    }

    #[must_use]
    pub fn bool_or(&self, section: &str, key: &str, fallback: bool) -> bool {
        match self.get(section, key) {
            Some(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => fallback,
            },
            None => fallback,
        }
    }

    #[must_use]
    pub fn int_or<T: std::str::FromStr>(&self, section: &str, key: &str, fallback: T) -> T {
        self.get(section, key)
            .and_then(|raw| raw.trim().parse::<T>().ok())
            .unwrap_or(fallback)
    }

    #[must_use]
    pub fn float_or<T: std::str::FromStr>(&self, section: &str, key: &str, fallback: T) -> T {
        self.get(section, key)
            .and_then(|raw| raw.trim().parse::<T>().ok())
            .unwrap_or(fallback)
    }

    #[must_use]
    pub fn str_or(&self, section: &str, key: &str, fallback: &str) -> String {
        self.get(section, key)
            .map_or_else(|| fallback.to_owned(), str::to_owned)
    }

    #[must_use]
    pub fn opt_str(&self, section: &str, key: &str) -> Option<String> {
        self.get(section, key)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }
}

fn normalize_section_name(name: &str) -> String {
    if name == LEGACY_SECTION {
        LEGACY_SECTION_REPLACEMENT.to_owned()
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# comment bang dau thang
; comment bang cham phay
[General]
SavePasswords=true
ScaleFonts = true

[WebServer]
WebServerPort=7963
Password=abc123

[RBX Alt Manager]
Theme=dark
";

    #[test]
    fn parses_sections_and_keys() {
        let ini = IniFile::parse(SAMPLE);
        assert_eq!(ini.get("General", "SavePasswords"), Some("true"));
        assert_eq!(ini.get("General", "ScaleFonts"), Some("true"));
        assert_eq!(ini.get("WebServer", "WebServerPort"), Some("7963"));
    }

    #[test]
    fn a_numeric_looking_password_stays_a_string() {
        let ini = IniFile::parse(SAMPLE);
        assert_eq!(ini.get("WebServer", "Password"), Some("abc123"));
    }

    #[test]
    fn the_legacy_section_name_is_renamed() {
        let ini = IniFile::parse(SAMPLE);
        assert_eq!(ini.get("Roblox Account Manager", "Theme"), Some("dark"));
        assert_eq!(ini.get("RBX Alt Manager", "Theme"), None);
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let ini = IniFile::parse(SAMPLE);
        assert_eq!(ini.section("General").map(BTreeMap::len), Some(2));
    }

    #[test]
    fn an_empty_value_drops_the_line_like_ram_does() {
        let ini = IniFile::parse("[General]\nSavePasswords=\nScaleFonts=true\n");
        assert_eq!(ini.get("General", "SavePasswords"), None);
        assert_eq!(ini.get("General", "ScaleFonts"), Some("true"));
    }

    #[test]
    fn a_key_outside_any_section_is_ignored() {
        let ini = IniFile::parse("Orphan=1\n[General]\nSavePasswords=true\n");
        assert_eq!(ini.get("General", "Orphan"), None);
        assert_eq!(ini.get("General", "SavePasswords"), Some("true"));
    }

    #[test]
    fn a_value_containing_equals_signs_is_kept_whole() {
        let ini = IniFile::parse("[General]\nQuery=a=b=c\n");
        assert_eq!(ini.get("General", "Query"), Some("a=b=c"));
    }

    #[test]
    fn the_last_duplicate_key_wins() {
        let ini = IniFile::parse("[General]\nA=1\nA=2\n");
        assert_eq!(ini.get("General", "A"), Some("2"));
    }

    #[test]
    fn a_file_without_a_trailing_newline_still_parses() {
        let ini = IniFile::parse("[General]\nSavePasswords=true");
        assert_eq!(ini.get("General", "SavePasswords"), Some("true"));
    }

    #[test]
    fn typed_readers_fall_back_when_the_value_is_missing_or_broken() {
        let ini = IniFile::parse("[General]\nBad=true\nDelay=khong-phai-so\n");

        assert!(
            ini.bool_or("General", "Bad", false),
            "true phai doc ra true"
        );
        assert!(
            ini.bool_or("General", "Thieu", true),
            "thieu thi lay fallback"
        );
        assert_eq!(ini.int_or::<i32>("General", "Delay", 8), 8);
        assert_eq!(ini.int_or::<i32>("General", "Thieu", 8), 8);
        assert_eq!(ini.str_or("General", "Thieu", "mac-dinh"), "mac-dinh");
        assert_eq!(ini.opt_str("General", "Thieu"), None);
    }

    #[test]
    fn bool_reader_accepts_1_and_0() {
        let ini = IniFile::parse("[General]\nA=1\nB=0\nC=True\nD=FALSE\n");
        assert!(ini.bool_or("General", "A", false));
        assert!(!ini.bool_or("General", "B", true));
        assert!(ini.bool_or("General", "C", false));
        assert!(!ini.bool_or("General", "D", true));
    }

    #[test]
    fn an_empty_file_is_empty() {
        assert!(IniFile::parse("").is_empty());
        assert!(IniFile::parse("# chi co comment\n").is_empty());
        assert!(!IniFile::parse(SAMPLE).is_empty());
    }
}
