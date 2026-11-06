pub struct Lang {
    pub name: &'static str,
    /// Reverso `from`/`to` code (3 letters, as used by the translation endpoint).
    pub code: &'static str,
    /// Reverso context code (2 letters, as used by context.reverso.net).
    pub short: &'static str,
}

pub const LANGS: &[Lang] = &[
    Lang {
        name: "english",
        code: "eng",
        short: "en",
    },
    Lang {
        name: "russian",
        code: "rus",
        short: "ru",
    },
    Lang {
        name: "ukrainian",
        code: "ukr",
        short: "uk",
    },
    Lang {
        name: "french",
        code: "fra",
        short: "fr",
    },
    Lang {
        name: "german",
        code: "ger",
        short: "de",
    },
    Lang {
        name: "spanish",
        code: "spa",
        short: "es",
    },
    Lang {
        name: "italian",
        code: "ita",
        short: "it",
    },
    Lang {
        name: "portuguese",
        code: "por",
        short: "pt",
    },
    Lang {
        name: "polish",
        code: "pol",
        short: "pl",
    },
    Lang {
        name: "dutch",
        code: "dut",
        short: "nl",
    },
    Lang {
        name: "arabic",
        code: "ara",
        short: "ar",
    },
    Lang {
        name: "hebrew",
        code: "heb",
        short: "he",
    },
    Lang {
        name: "japanese",
        code: "jpn",
        short: "ja",
    },
    Lang {
        name: "turkish",
        code: "tur",
        short: "tr",
    },
    Lang {
        name: "chinese",
        code: "chi",
        short: "zh",
    },
    Lang {
        name: "romanian",
        code: "rum",
        short: "ro",
    },
    Lang {
        name: "swedish",
        code: "swe",
        short: "sv",
    },
];

pub fn find(name: &str) -> Option<usize> {
    LANGS.iter().position(|l| l.name.eq_ignore_ascii_case(name))
}

pub fn is_valid(name: &str) -> bool {
    find(name).is_some()
}

pub fn name(index: usize) -> &'static str {
    LANGS[index].name
}

pub fn code(name: &str) -> Option<&'static str> {
    LANGS
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(name))
        .map(|l| l.code)
}

pub fn short(name: &str) -> Option<&'static str> {
    LANGS
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(name))
        .map(|l| l.short)
}

pub fn index_or(name: &str, fallback: usize) -> usize {
    find(name).unwrap_or(fallback)
}
