use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{
    Locale, Message, glossary,
    translate::{self, Failure, Key, Problem},
};

fn answer(f: impl Fn(Locale) -> String) -> BTreeMap<String, String> {
    Locale::ALL.iter().map(|l| (l.tag().to_string(), f(*l))).collect()
}

fn key<'a>(english: &'a str) -> Key<'a> {
    Key { name: "planning_next_moves", english, note: "The status line after a message is sent." }
}

fn blank() -> Message {
    Message { ar: "", ca: "", cs: "", da: "", de: "", el: "", en: "", es: "", es_419: "", fi: "", fr: "", fr_ca: "", he: "", hi: "", hr: "", hu: "", id: "", it: "", ja: "", ko: "", ms: "", nl: "", no: "", pl: "", pt_br: "", pt_pt: "", ro: "", ru: "", sk: "", sv: "", th: "", tr: "", uk: "", vi: "", zh_cn: "", zh_tw: "" }
}

fn reply(map: &BTreeMap<String, String>) -> Value {
    json!({"choices": [{"message": {"content": serde_json::to_string(map).unwrap()}}]})
}

#[test]
fn every_locale_has_a_tag_a_name_and_a_field_in_the_same_order() {
    assert_eq!(Locale::ALL.len(), 36);
    let tags: std::collections::HashSet<_> = Locale::ALL.iter().map(|l| l.tag()).collect();
    assert_eq!(tags.len(), Locale::ALL.len(), "no tag twice");
    let m = Message { ar: "ar", ca: "ca", cs: "cs", da: "da", de: "de", el: "el", en: "en", es: "es", es_419: "es-419", fi: "fi", fr: "fr", fr_ca: "fr-CA", he: "he", hi: "hi", hr: "hr", hu: "hu", id: "id", it: "it", ja: "ja", ko: "ko", ms: "ms", nl: "nl", no: "no", pl: "pl", pt_br: "pt-BR", pt_pt: "pt-PT", ro: "ro", ru: "ru", sk: "sk", sv: "sv", th: "th", tr: "tr", uk: "uk", vi: "vi", zh_cn: "zh-CN", zh_tw: "zh-TW" };
    for locale in Locale::ALL {
        assert_eq!(m.get(*locale), locale.tag(), "{locale:?} reads its own field");
    }
}

#[test]
fn only_arabic_and_hebrew_read_right_to_left() {
    let rtl: Vec<_> = Locale::ALL.iter().filter(|l| l.is_rtl()).map(|l| l.tag()).collect();
    assert_eq!(rtl, ["ar", "he"]);
}

#[test]
fn a_system_tag_finds_the_closest_locale() {
    let cases = [
        ("en_US.UTF-8", Locale::En), ("en-GB", Locale::En), ("fr_CH", Locale::Fr), ("fr-CA", Locale::FrCa), ("fr_CA.UTF-8", Locale::FrCa),
        ("pt", Locale::PtBr), ("pt-PT", Locale::PtPt), ("es-ES", Locale::Es), ("es-MX", Locale::Es419), ("es-419", Locale::Es419), ("es", Locale::Es),
        ("zh", Locale::ZhCn), ("zh-Hans-CN", Locale::ZhCn), ("zh-Hant", Locale::ZhTw), ("zh-HK", Locale::ZhTw), ("zh_TW", Locale::ZhTw),
        ("nb", Locale::No), ("nn-NO", Locale::No), ("iw", Locale::He), ("DE", Locale::De), ("ja_JP.eucJP", Locale::Ja),
    ];
    for (tag, want) in cases {
        assert_eq!(Locale::from_tag(tag), Some(want), "{tag}");
    }
    assert_eq!(Locale::from_tag("xx"), None);
    assert_eq!(Locale::from_tag(""), None);
    assert_eq!(Locale::from_tag("C"), None);
}

#[test]
fn the_first_language_the_app_speaks_wins_and_english_is_the_last_resort() {
    assert_eq!(Locale::resolve(["xx", "ko-KR", "de"]), Locale::Ko);
    assert_eq!(Locale::resolve(["xx", "yy"]), Locale::En);
    assert_eq!(Locale::resolve([]), Locale::En);
}

// One test owns the global, so no other test races it.
#[test]
fn the_current_language_starts_as_english_and_changes() {
    assert_eq!(super::current(), Locale::En);
    let m = Message { en: "Hello", de: "Hallo", ..blank() };
    assert_eq!(super::t(&m), "Hello");
    super::set_current(Locale::De);
    assert_eq!(super::t(&m), "Hallo");
    super::set_current(Locale::En);
}

#[test]
fn a_value_goes_where_its_name_stands_and_an_unknown_name_stays() {
    let fill = super::message::fill;
    assert_eq!(fill("Open {file} in {editor}", &[("file", "a.rs"), ("editor", "Zed")]), "Open a.rs in Zed");
    assert_eq!(fill("{n} files", &[("n", "3")]), "3 files");
    assert_eq!(fill("Hello {who}", &[]), "Hello {who}");
    assert_eq!(fill("Open {", &[]), "Open {");
    assert_eq!(fill("ファイル{n}件", &[("n", "3")]), "ファイル3件");
}

#[test]
fn the_glossary_finds_names_by_whole_word_and_case() {
    let names = |s: &str| glossary::matching(s).iter().map(|t| t.source).collect::<Vec<_>>();
    assert_eq!(names("Waiting for Cursor…"), ["Cursor"]);
    assert_eq!(names("Ask Claude Code to review"), ["Claude", "Claude Code"]);
    assert!(names("Move the cursor").is_empty(), "the cursor you type with is not the editor");
    assert!(names("Cursors blink").is_empty(), "whole words only");
    assert_eq!(names("Push to GitHub"), ["GitHub"]);
}

#[test]
fn names_are_snake_case() {
    for ok in ["a", "planning_next_moves", "tab2"] {
        assert!(translate::valid_name(ok), "{ok}");
    }
    for bad in ["", "Planning", "_a", "a_", "2a", "a-b", "a b"] {
        assert!(!translate::valid_name(bad), "{bad}");
    }
}

#[test]
fn the_prompt_names_every_locale_and_what_the_glossary_says() {
    let (system, user) = translate::prompt(&key("Waiting for Cursor…"));
    for locale in Locale::ALL {
        assert!(system.contains(locale.tag()) && system.contains(locale.name()), "{}", locale.tag());
    }
    assert!(user.contains("Waiting for Cursor…") && user.contains("The status line"));
    assert!(user.contains("\"Cursor\": a name"));
    let (_, plain) = translate::prompt(&key("Planning next moves…"));
    assert!(!plain.contains("Glossary"));
}

#[test]
fn a_good_answer_is_written_as_one_constant_with_every_field() {
    let english = "Open {file}";
    let map = answer(|l| format!("{} {{file}}", l.tag()));
    assert!(translate::check(&key(english), &map).is_empty());
    let source = translate::render(&key(english), &map);
    assert!(source.contains("pub const PLANNING_NEXT_MOVES: Message = Message {"));
    assert!(source.contains("    es_419: \"es-419 {file}\","));
    assert!(source.contains("    zh_tw: \"zh-TW {file}\","));
    assert_eq!(source.matches(",\n").count(), 36, "a line for each locale");
}

#[test]
fn a_string_with_quotes_and_newlines_is_written_as_valid_rust() {
    let map = answer(|_| "Say \"hi\"\nthen\\go".to_string());
    let source = translate::render(&key("x"), &map);
    assert!(source.contains(r#"ar: "Say \"hi\"\nthen\\go","#));
}

#[test]
fn an_answer_is_refused_for_what_it_gets_wrong() {
    let english = "Open {file} in Cursor";
    let good = |l: Locale| format!("{} {{file}} Cursor", l.tag());
    let mut map = answer(good);
    assert!(translate::check(&key(english), &map).is_empty());

    map.remove("de");
    map.insert("fr".into(), "Ouvrir dans Cursor".into());
    map.insert("ja".into(), "{file} をカーソルで開く".into());
    map.insert("ko".into(), "  ".into());
    map.insert("xx".into(), "?".into());
    map.insert("it".into(), "Apri {archivo} in Cursor".into());
    let problems = translate::check(&key(english), &map);
    assert!(problems.contains(&Problem::Missing("de")));
    assert!(problems.contains(&Problem::Unknown("xx".into())));
    assert!(problems.contains(&Problem::Empty("ko")));
    assert!(problems.contains(&Problem::Name { tag: "ja", term: "Cursor" }));
    assert!(problems.contains(&Problem::Placeholders { tag: "fr", want: vec!["file".into()], got: vec![] }));
    assert!(problems.contains(&Problem::Placeholders { tag: "it", want: vec!["file".into()], got: vec!["archivo".into()] }));
}

#[test]
fn a_reply_may_come_in_a_code_fence() {
    assert_eq!(translate::parse("```json\n{\"en\": \"a\"}\n```").unwrap()["en"], "a");
    assert_eq!(translate::parse("{\"en\": \"a\"}").unwrap()["en"], "a");
    assert!(matches!(translate::parse("sorry"), Err(Problem::NotJson(_))));
    assert!(matches!(translate::parse("[1]"), Err(Problem::NotJson(_))));
    assert!(matches!(translate::parse("{\"en\": 1}"), Err(Problem::NotJson(_))));
}

#[test]
fn a_request_asks_the_translating_model_for_json() {
    let body = translate::request_body("sys", "user", &[]);
    assert_eq!(body["model"], "openai/gpt-6-luna");
    assert_eq!(body["response_format"]["type"], "json_object");
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    let again = translate::request_body("sys", "user", &[Problem::Missing("de")]);
    let last = again["messages"][2]["content"].as_str().unwrap();
    assert!(last.contains("\"de\" is missing"), "{last}");
}

#[test]
fn a_good_first_answer_is_one_request() {
    let map = answer(|l| format!("{} Planning", l.tag()));
    let mut sent = 0;
    let source = translate::translate(&key("Planning"), &mut |_| {
        sent += 1;
        Ok(reply(&map))
    })
    .unwrap();
    assert_eq!(sent, 1);
    assert!(source.contains("pub const PLANNING_NEXT_MOVES"));
}

#[test]
fn a_bad_answer_goes_back_once_with_its_problems_and_a_fixed_one_is_taken() {
    let mut bad = answer(|l| format!("{} Planning", l.tag()));
    bad.remove("sv");
    let good = answer(|l| format!("{} Planning", l.tag()));
    let mut bodies = Vec::new();
    let source = translate::translate(&key("Planning"), &mut |body| {
        bodies.push(body.clone());
        Ok(reply(if bodies.len() == 1 { &bad } else { &good }))
    })
    .unwrap();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[1]["messages"][2]["content"].as_str().unwrap().contains("\"sv\" is missing"));
    assert!(source.contains("sv: \"sv Planning\""));
}

#[test]
fn two_bad_answers_are_a_failure_and_nothing_is_written() {
    let mut bad = answer(|l| format!("{} Planning", l.tag()));
    bad.remove("sv");
    let mut sent = 0;
    let failure = translate::translate(&key("Planning"), &mut |_| {
        sent += 1;
        Ok(reply(&bad))
    })
    .unwrap_err();
    assert_eq!(sent, 2, "one retry and no more");
    assert_eq!(failure, Failure::Answer(vec![Problem::Missing("sv")]));
}

#[test]
fn a_request_that_fails_or_a_response_with_no_reply_is_a_failure() {
    let failure = translate::translate(&key("x"), &mut |_| Err("401".into())).unwrap_err();
    assert_eq!(failure, Failure::Send("401".into()));
    let failure = translate::translate(&key("x"), &mut |_| Ok(json!({"error": {}}))).unwrap_err();
    assert_eq!(failure, Failure::NoReply);
    let bad = Key { name: "Bad Name", english: "x", note: "" };
    assert_eq!(translate::translate(&bad, &mut |_| unreachable!()).unwrap_err(), Failure::Name);
}

#[test]
fn a_script_is_written_as_itself_and_only_what_cannot_be_seen_is_escaped() {
    let map = answer(|_| "हिन्दी ไทย\u{200f}\u{7}".to_string());
    let source = translate::render(&key("x"), &map);
    assert!(source.contains("ar: \"हिन्दी ไทย\\u{200f}\\u{7}\","), "{source}");
}

#[test]
fn the_environment_names_the_language_in_the_order_the_c_library_reads_it() {
    let env = |pairs: &'static [(&str, &str)]| move |name: &str| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string());
    let read = super::system::from_env;
    assert_eq!(read(env(&[("LANG", "fr_FR.UTF-8")])), ["fr_FR.UTF-8"]);
    assert_eq!(read(env(&[("LANGUAGE", "de:en"), ("LANG", "fr_FR.UTF-8")])), ["de", "en", "fr_FR.UTF-8"]);
    assert_eq!(read(env(&[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "fr_FR.UTF-8")])), ["ja_JP.UTF-8"], "LC_ALL settles it");
    assert_eq!(read(env(&[("ATELIER_LANG", "ko"), ("LANG", "fr_FR.UTF-8")])), ["ko"], "a language asked for wins");
    assert_eq!(read(env(&[("LANG", "")])), Vec::<String>::new());
    assert_eq!(Locale::resolve(["C", "pt_BR.UTF-8"]), Locale::PtBr, "C names no language, so the next one counts");
}

#[test]
fn a_macs_language_list_is_read_in_order() {
    let out = "(\n    \"fr-FR\",\n    \"en-US\",\n    \"zh-Hans-CN\"\n)\n";
    assert_eq!(super::system::parse_apple_languages(out), ["fr-FR", "en-US", "zh-Hans-CN"]);
    assert!(super::system::parse_apple_languages("(\n)\n").is_empty());
}
