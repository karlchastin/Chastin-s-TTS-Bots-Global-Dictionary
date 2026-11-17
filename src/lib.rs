use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

pub mod cleaner;
pub mod formatter;
pub mod slang;

const EMBEDDED_DICTIONARY_JSON: &str = include_str!("../dictionary.json");
const CANONICAL_WINDOWS_DICT_PATH: &str =
    r"C:\Users\NotChztn\Documents\GitHub\Chastin's TTS Bots Global Dictionary\dictionary.json";

#[derive(Clone, Debug)]
pub struct KeyboardSmashConfig {
    pub no_vowel_min_len: usize,
    pub no_vowel_replacement: String,
    pub laugh_min_len: usize,
    pub laugh_required_chars: String,
    pub laugh_allowed_chars: String,
    pub only_a_min_len: usize,
    pub laugh_replacement: String,
}

impl Default for KeyboardSmashConfig {
    fn default() -> Self {
        Self {
            no_vowel_min_len: 10,
            no_vowel_replacement: "GUH-GUH-GUH-GUH".to_string(),
            laugh_min_len: 5,
            laugh_required_chars: "hjk".to_string(),
            laugh_allowed_chars: "ahjkswgzx".to_string(),
            only_a_min_len: 10,
            laugh_replacement: "Hahaha".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LimitsConfig {
    pub huge_number_min_digits: usize,
    pub huge_number_replacement: String,
    pub max_message_len: usize,
    pub repetitive_word_threshold: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            huge_number_min_digits: 10,
            huge_number_replacement: "a huge number".to_string(),
            max_message_len: 300,
            repetitive_word_threshold: 6,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DictionaryData {
    pub words: Vec<[String; 2]>,
    pub phonetic_pre_rules: Vec<[String; 2]>,
    pub phonetic_post_rules: Vec<[String; 2]>,
    pub keyboard_smash: KeyboardSmashConfig,
    pub emoticons: Vec<[String; 2]>,
    pub short_caps_words: Vec<String>,
    pub limits: LimitsConfig,
}

fn parse_pair_array(val: Option<&Value>) -> Vec<[String; 2]> {
    val.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let pair = item.as_array()?;
                    if pair.len() < 2 {
                        return None;
                    }
                    Some([
                        pair[0].as_str()?.to_string(),
                        pair[1].as_str()?.to_string(),
                    ])
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_dictionary_json(raw: &str) -> Option<DictionaryData> {
    let root: Value = serde_json::from_str(raw).ok()?;
    let obj = root.as_object()?;

    let mut smash = KeyboardSmashConfig::default();
    if let Some(ks) = obj.get("keyboard_smash").and_then(|v| v.as_object()) {
        if let Some(v) = ks.get("no_vowel_min_len").and_then(|v| v.as_u64()) {
            smash.no_vowel_min_len = v as usize;
        }
        if let Some(v) = ks.get("no_vowel_replacement").and_then(|v| v.as_str()) {
            smash.no_vowel_replacement = v.to_string();
        }
        if let Some(v) = ks.get("laugh_min_len").and_then(|v| v.as_u64()) {
            smash.laugh_min_len = v as usize;
        }
        if let Some(v) = ks.get("laugh_required_chars").and_then(|v| v.as_str()) {
            smash.laugh_required_chars = v.to_string();
        }
        if let Some(v) = ks.get("laugh_allowed_chars").and_then(|v| v.as_str()) {
            smash.laugh_allowed_chars = v.to_string();
        }
        if let Some(v) = ks.get("only_a_min_len").and_then(|v| v.as_u64()) {
            smash.only_a_min_len = v as usize;
        }
        if let Some(v) = ks.get("laugh_replacement").and_then(|v| v.as_str()) {
            smash.laugh_replacement = v.to_string();
        }
    }

    let mut limits = LimitsConfig::default();
    if let Some(lim) = obj.get("limits").and_then(|v| v.as_object()) {
        if let Some(v) = lim.get("huge_number_min_digits").and_then(|v| v.as_u64()) {
            limits.huge_number_min_digits = v as usize;
        }
        if let Some(v) = lim.get("huge_number_replacement").and_then(|v| v.as_str()) {
            limits.huge_number_replacement = v.to_string();
        }
        if let Some(v) = lim.get("max_message_len").and_then(|v| v.as_u64()) {
            limits.max_message_len = v as usize;
        }
        if let Some(v) = lim.get("repetitive_word_threshold").and_then(|v| v.as_u64()) {
            limits.repetitive_word_threshold = v as usize;
        }
    }

    let short_caps_words = obj
        .get("short_caps_words")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    Some(DictionaryData {
        words: parse_pair_array(obj.get("words")),
        phonetic_pre_rules: parse_pair_array(obj.get("phonetic_pre_rules")),
        phonetic_post_rules: parse_pair_array(obj.get("phonetic_post_rules")),
        keyboard_smash: smash,
        emoticons: parse_pair_array(obj.get("emoticons")),
        short_caps_words,
        limits,
    })
}

pub struct CompiledDictionary {
    pub slang_rules: Vec<(Regex, String)>,
    pub phonetic_pre: Vec<(Regex, String)>,
    pub phonetic_post: Vec<(Regex, String)>,
    pub keyboard_smash: KeyboardSmashConfig,
    pub emoticons: Vec<[String; 2]>,
    pub short_words_re: Option<Regex>,
    pub huge_num_re: Regex,
    pub limits: LimitsConfig,
}

impl CompiledDictionary {
    pub fn compile(data: DictionaryData) -> Self {
        let slang_rules = data
            .words
            .into_iter()
            .filter_map(|[raw_key, replacement]| {
                let pattern = if let Some(raw_re) = raw_key.strip_prefix("regex:") {
                    format!("(?i){raw_re}")
                } else {
                    format!(r"(?i)\b{}\b", regex::escape(raw_key.trim()))
                };
                Regex::new(&pattern).ok().map(|re| (re, replacement))
            })
            .collect();

        let compile_ci = |entries: Vec<[String; 2]>| -> Vec<(Regex, String)> {
            entries
                .into_iter()
                .filter_map(|[pat, repl]| {
                    Regex::new(&format!("(?i){pat}")).ok().map(|re| (re, repl))
                })
                .collect()
        };

        let phonetic_pre = compile_ci(data.phonetic_pre_rules);
        let phonetic_post = compile_ci(data.phonetic_post_rules);

        let short_words_re = if data.short_caps_words.is_empty() {
            None
        } else {
            let escaped: Vec<String> = data
                .short_caps_words
                .iter()
                .map(|w| regex::escape(w.trim()))
                .filter(|w| !w.is_empty())
                .collect();
            if escaped.is_empty() {
                None
            } else {
                Regex::new(&format!(r"\b({})\b", escaped.join("|"))).ok()
            }
        };

        let min_digits = data.limits.huge_number_min_digits.max(1);
        let huge_num_re = Regex::new(&format!(r"\d{{{min_digits},}}"))
            .unwrap_or_else(|_| Regex::new(r"\d{10,}").unwrap());

        Self {
            slang_rules,
            phonetic_pre,
            phonetic_post,
            keyboard_smash: data.keyboard_smash,
            emoticons: data.emoticons,
            short_words_re,
            huge_num_re,
            limits: data.limits,
        }
    }
}

struct CachedDictionary {
    source_path: Option<PathBuf>,
    last_modified: Option<SystemTime>,
    compiled: Arc<CompiledDictionary>,
}

fn resolve_dictionary_path() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("TTS_DICTIONARY_PATH") {
        let p = PathBuf::from(env_path.trim());
        if !env_path.trim().is_empty() && p.exists() {
            return Some(p);
        }
    }

    let candidates = [
        Path::new("dictionary.json"),
        Path::new("../Chastin's TTS Bots Global Dictionary/dictionary.json"),
        Path::new("../../Chastin's TTS Bots Global Dictionary/dictionary.json"),
        Path::new(CANONICAL_WINDOWS_DICT_PATH),
    ];

    for p in candidates {
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

fn resolve_repo_dir(dict_path: &Path) -> Option<PathBuf> {
    let parent = dict_path.parent()?;
    if parent.as_os_str().is_empty() {
        std::env::current_dir().ok()
    } else {
        Some(parent.to_path_buf())
    }
}

fn sync_git_repo_if_available(dict_path: &Path) {
    if cfg!(test) {
        return;
    }
    let Some(repo_dir) = resolve_repo_dir(dict_path) else {
        return;
    };
    if !repo_dir.join(".git").exists() {
        return;
    }

    let remotes_out = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .arg("remote")
        .output();
    let has_remote = matches!(
        remotes_out,
        Ok(out) if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty()
    );
    if !has_remote {
        return;
    }

    let _ = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_dir)
        .args(["pull", "--ff-only", "--quiet"])
        .output();
}

fn load_initial_cache() -> CachedDictionary {
    let source_path = resolve_dictionary_path();
    let mut last_modified = None;
    let mut raw_json = None;

    if let Some(ref path) = source_path {
        sync_git_repo_if_available(path);
        if let Ok(meta) = std::fs::metadata(path) {
            last_modified = meta.modified().ok();
        }
        raw_json = std::fs::read_to_string(path).ok();
    }

    let parsed = raw_json
        .as_deref()
        .and_then(parse_dictionary_json)
        .or_else(|| parse_dictionary_json(EMBEDDED_DICTIONARY_JSON))
        .unwrap_or_default();

    CachedDictionary {
        source_path,
        last_modified,
        compiled: Arc::new(CompiledDictionary::compile(parsed)),
    }
}

lazy_static! {
    static ref DICTIONARY_CACHE: RwLock<CachedDictionary> =
        RwLock::new(load_initial_cache());
}

pub fn sync_and_reload_dictionary() -> Arc<CompiledDictionary> {
    if let Ok(guard) = DICTIONARY_CACHE.read() {
        if let Some(ref path) = guard.source_path {
            sync_git_repo_if_available(path);
        }
    }
    get_dictionary()
}

pub fn get_dictionary() -> Arc<CompiledDictionary> {
    {
        if let Ok(guard) = DICTIONARY_CACHE.read() {
            let needs_reload = match &guard.source_path {
                Some(path) => std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .ok()
                    .map_or(false, |mtime| Some(mtime) != guard.last_modified),
                None => false,
            };
            if !needs_reload {
                return guard.compiled.clone();
            }
        }
    }

    if let Ok(mut guard) = DICTIONARY_CACHE.write() {
        if let Some(ref path) = guard.source_path {
            let disk_mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            if disk_mtime != guard.last_modified {
                if let Ok(raw) = std::fs::read_to_string(path) {
                    if let Some(parsed) = parse_dictionary_json(&raw) {
                        guard.compiled = Arc::new(CompiledDictionary::compile(parsed));
                        guard.last_modified = disk_mtime;
                    }
                }
            }
        }
        return guard.compiled.clone();
    }

    let fallback = parse_dictionary_json(EMBEDDED_DICTIONARY_JSON).unwrap_or_default();
    Arc::new(CompiledDictionary::compile(fallback))
}
