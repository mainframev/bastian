use std::path::PathBuf;

use super::theme::with_theme;
use super::*;

fn env(pairs: &[(&str, &str)]) -> EnvMap {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn precedence_is_default_file_env_flag() {
    let file = || Config {
        theme: Some("light".into()),
        width: Some(80),
        ..Config::default()
    };
    let none = Config::default();

    let r = resolve(Config::default(), &env(&[]), none.clone()).unwrap();
    assert_eq!(
        r.theme,
        Setting {
            value: "auto".to_string(),
            source: Source::Default
        }
    );
    assert_eq!(r.width.value, 100);
    assert_eq!(r.background.value, PageBackground::Theme);

    let r = resolve(file(), &env(&[]), none.clone()).unwrap();
    assert_eq!(
        r.theme,
        Setting {
            value: "light".to_string(),
            source: Source::File
        }
    );

    let e = env(&[("BASTIAN_THEME", "dark"), ("BASTIAN_WIDTH", "120")]);
    let r = resolve(file(), &e, none).unwrap();
    assert_eq!(
        r.theme,
        Setting {
            value: "dark".to_string(),
            source: Source::Env
        }
    );
    assert_eq!(
        r.width,
        Setting {
            value: 120,
            source: Source::Env
        }
    );

    let cli = Config {
        theme: Some("light".into()),
        width: Some(60),
        confirm: Some(Confirm::Never),
        line_numbers: Some(true),
        ..Config::default()
    };
    let r = resolve(file(), &e, cli).unwrap();
    assert_eq!(
        r.theme,
        Setting {
            value: "light".to_string(),
            source: Source::Flag
        }
    );
    assert_eq!(
        r.width,
        Setting {
            value: 60,
            source: Source::Flag
        }
    );
    assert_eq!(r.confirm.value, Confirm::Never);
    assert!(r.line_numbers.value);
}

#[test]
fn locate_prefers_flag_then_env() {
    let lookup = |k: &str| (k == "HOME").then(|| "/h".to_string());
    assert_eq!(
        locate(Some("~/f.toml"), Some("/e.toml"), &lookup),
        Some((PathBuf::from("/h/f.toml"), true))
    );
    assert_eq!(
        locate(None, Some("/e.toml"), &lookup),
        Some((PathBuf::from("/e.toml"), true))
    );
    assert!(matches!(
        locate(None, Some(""), &lookup),
        Some((_, false)) | None
    ));
}

#[test]
fn explicit_missing_config_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("none.toml");
    assert!(load_at(&missing, true).is_err());
    assert_eq!(load_at(&missing, false).unwrap(), Config::default());
}

#[test]
fn template_parses_and_describe_shows_sources() {
    assert_eq!(
        toml::from_str::<Config>(TEMPLATE).unwrap(),
        Config::default()
    );
    let file = Config {
        width: Some(80),
        ..Config::default()
    };
    let r = resolve(file, &env(&[("BASTIAN_THEME", "light")]), Config::default()).unwrap();
    let text = describe(&r, None);
    assert!(text.contains("theme = \"light\""), "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("theme") && l.ends_with("# environment")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("width = 80") && l.ends_with("# config file")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("pager = false") && l.ends_with("# default")),
        "{text}"
    );
    // The output is itself valid config.
    let stripped: String = text
        .lines()
        .map(|l| format!("{}\n", l.split('#').next().unwrap()))
        .collect();
    assert!(toml::from_str::<Config>(&stripped).is_ok(), "{stripped}");
}

#[test]
fn env_values_are_parsed_and_validated() {
    let e = env(&[
        ("BASTIAN_CONFIRM", "NEVER"),
        ("BASTIAN_LINE_NUMBERS", "yes"),
        ("BASTIAN_THEME", ""),
        ("BASTIAN_SELECTOR", "Arrow"),
        ("BASTIAN_LIST_LAYOUT", "compact"),
        ("BASTIAN_EXTENSIONS", "md, txt,,"),
        ("BASTIAN_SORT", "name"),
        ("BASTIAN_BACKGROUND", "Terminal"),
    ]);
    let r = resolve(Config::default(), &e, Config::default()).unwrap();
    assert_eq!(
        r.confirm,
        Setting {
            value: Confirm::Never,
            source: Source::Env
        }
    );
    assert_eq!(
        r.line_numbers,
        Setting {
            value: true,
            source: Source::Env
        }
    );
    assert_eq!(r.selector.value, Selector::Arrow);
    assert_eq!(r.list_layout.value, ListLayout::Compact);
    assert_eq!(r.extensions.value, ["md", "txt"]);
    assert_eq!(r.sort.value, SortBy::Name);
    assert_eq!(r.background.value, PageBackground::Terminal);
    // Empty means unset.
    assert_eq!(r.theme.source, Source::Default);

    let err = resolve(
        Config::default(),
        &env(&[("BASTIAN_SELECTOR", "star")]),
        Config::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("expected bar, thick, block, arrow or highlight"),
        "{err}"
    );

    let err = resolve(
        Config::default(),
        &env(&[("BASTIAN_WIDTH", "wide")]),
        Config::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("BASTIAN_WIDTH") && err.contains("wide"),
        "{err}"
    );
}

#[test]
fn parses_all_fields() {
    let c: Config = toml::from_str(
        r#"
            theme = "light"
            width = 80
            confirm = "never"
            line_numbers = true
            color = "never"
            pager = true
            mouse = true
            selector = "highlight"
            list_layout = "compact"
            sort = "created"
            dates = false
            date_format = "%Y-%m-%d"
            extensions = ["md", "txt"]
            ignore = ["drafts/"]
            image_height = 12
            tab_width = 2
            hyperlinks = false
            scrollbar = false
            remember_position = false
            editor = "nvim -p"
            [interpreters]
            ruby = ["ruby", "-e"]
            fish = []
            "#,
    )
    .unwrap();
    assert_eq!(c.theme, Some("light".into()));
    assert_eq!(c.width, Some(80));
    assert_eq!(c.confirm, Some(Confirm::Never));
    assert_eq!(c.line_numbers, Some(true));
    assert_eq!(c.color, Some(ColorMode::Never));
    assert_eq!(c.pager, Some(true));
    assert_eq!(c.mouse, Some(true));
    assert_eq!(c.selector, Some(Selector::Highlight));
    assert_eq!(c.list_layout, Some(ListLayout::Compact));
    assert_eq!(c.sort, Some(SortBy::Created));
    assert_eq!(c.dates, Some(false));
    assert_eq!(c.date_format.as_deref(), Some("%Y-%m-%d"));
    assert_eq!(c.extensions, Some(vec!["md".into(), "txt".into()]));
    assert_eq!(c.ignore, Some(vec!["drafts/".into()]));
    assert_eq!(c.image_height, Some(12));
    assert_eq!(c.tab_width, Some(2));
    assert_eq!(c.hyperlinks, Some(false));
    assert_eq!(c.scrollbar, Some(false));
    assert_eq!(c.remember_position, Some(false));
    assert_eq!(c.editor.as_deref(), Some("nvim -p"));
    assert_eq!(c.interpreters["ruby"], ["ruby", "-e"]);
    let fish = &c.interpreters["fish"];
    assert!(fish.is_empty(), "{fish:?}");
}

#[test]
fn empty_is_default() {
    assert_eq!(toml::from_str::<Config>("").unwrap(), Config::default());
}

#[test]
fn rejects_unknown_keys() {
    let err = toml::from_str::<Config>("colour = \"red\"").unwrap_err();
    assert!(err.to_string().contains("colour"), "{err}");
}

#[test]
fn load_reports_path_on_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("config.toml");
    std::fs::write(&p, "width = \"wide\"").unwrap();
    let err = format!("{:#}", load_from(&p).unwrap_err());
    assert!(err.contains("config.toml"), "{err}");
}

#[test]
fn theme_name_follows_auto_dark_and_light() {
    let r = |theme: &str| {
        resolve(
            Config {
                theme: Some(theme.into()),
                dark_theme: Some("nord".into()),
                ..Config::default()
            },
            &env(&[]),
            Config::default(),
        )
        .unwrap()
    };
    assert_eq!(theme_name(&r("auto"), false), "nord");
    assert_eq!(theme_name(&r("auto"), true), "rose-pine-dawn");
    assert_eq!(theme_name(&r("dark"), true), "nord");
    assert_eq!(theme_name(&r("light"), false), "rose-pine-dawn");
    assert_eq!(theme_name(&r("dracula"), true), "dracula");
}

#[test]
fn unknown_themes_name_their_source() {
    let exists = |n: &str| ["rose-pine", "rose-pine-dawn", "nord"].contains(&n);
    let check = |file: Config, e: &[(&str, &str)], cli: Config| {
        check_themes(&resolve(file, &env(e), cli).unwrap(), exists)
    };
    assert!(check(Config::default(), &[], Config::default()).is_ok());
    let flag = Config {
        theme: Some("nope".into()),
        ..Config::default()
    };
    let err = check(Config::default(), &[], flag).unwrap_err().to_string();
    assert!(
        err.contains("unknown theme \"nope\" (from --theme)"),
        "{err}"
    );
    let err = check(
        Config::default(),
        &[("BASTIAN_LIGHT_THEME", "x")],
        Config::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("(from BASTIAN_LIGHT_THEME)"), "{err}");
    let file = Config {
        dark_theme: Some("y".into()),
        ..Config::default()
    };
    let err = check(file, &[], Config::default()).unwrap_err().to_string();
    assert!(
        err.contains("(from `dark_theme` in the config file)"),
        "{err}"
    );
}

#[test]
fn saving_a_theme_keeps_the_rest_of_the_file() {
    assert_eq!(with_theme("", "nord"), "theme = \"nord\"\n");
    assert_eq!(
        with_theme("# mine\n\nwidth = 80\n[interpreters]\ntheme = []\n", "nord"),
        "# mine\n\ntheme = \"nord\"\nwidth = 80\n[interpreters]\ntheme = []\n"
    );
    assert_eq!(
        with_theme(
            "width = 80\ntheme = \"auto\"  # follow the terminal\n",
            "dracula"
        ),
        "width = 80\ntheme = \"dracula\"  # follow the terminal\n"
    );
    // The commented template line stays a comment.
    let t = with_theme(TEMPLATE, "nord");
    assert!(t.contains("# theme = \"auto\""), "{t}");
    let c: Config = toml::from_str(&t).unwrap();
    assert_eq!(c.theme.as_deref(), Some("nord"));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub/config.toml");
    save_theme(&path, "kanagawa").unwrap();
    save_theme(&path, "nord").unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "theme = \"nord\"\n"
    );
}
