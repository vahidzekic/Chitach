//! Port of bas-celik `localization` package.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language { SrLatin, SrCyrillic, En, Ru }

const CYRILLIC: [char; 60] = [
    'А', 'Б', 'В', 'Г', 'Д', 'Ђ', 'Е', 'Ж', 'З', 'И', 'Ј', 'К', 'Л', 'Љ', 'М', 'Н', 'Њ', 'О', 'П', 'Р', 'С', 'Т', 'Ћ', 'У', 'Ф', 'Х', 'Ц', 'Ч', 'Џ', 'Ш',
    'а', 'б', 'в', 'г', 'д', 'ђ', 'е', 'ж', 'з', 'и', 'ј', 'к', 'л', 'љ', 'м', 'н', 'њ', 'о', 'п', 'р', 'с', 'т', 'ћ', 'у', 'ф', 'х', 'ц', 'ч', 'џ', 'ш',
];
const LATIN: [&str; 60] = [
    "A", "B", "V", "G", "D", "Đ", "E", "Ž", "Z", "I", "J", "K", "L", "Lj", "M", "N", "Nj", "O", "P", "R", "S", "T", "Ć", "U", "F", "H", "C", "Č", "Dž", "Š",
    "a", "b", "v", "g", "d", "đ", "e", "ž", "z", "i", "j", "k", "l", "lj", "m", "n", "nj", "o", "p", "r", "s", "t", "ć", "u", "f", "h", "c", "č", "dž", "š",
];

pub fn cyrillic_to_latin(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match CYRILLIC.iter().position(|&x| x == c) {
            Some(i) => out.push_str(LATIN[i]),
            None => out.push(c),
        }
    }
    out
}

pub fn format_yes_no(v: bool, lang: Language) -> &'static str {
    match (lang, v) {
        (Language::SrLatin, true) => "Da",
        (Language::SrLatin, false) => "Ne",
        (Language::SrCyrillic, true) => "Да",
        (Language::SrCyrillic, false) => "Не",
        (Language::Ru, true) => "Да",
        (Language::Ru, false) => "Нет",
        (Language::En, true) => "Yes",
        (Language::En, false) => "No",
    }
}

/// `DDMMYYYY` -> `DD.MM.YYYY.`; year starting with `0` -> `Nije dostupan`.
pub fn format_date(s: &mut String) {
    let c: Vec<char> = s.chars().collect();
    if c.len() != 8 {
        return;
    }
    if c[4] == '0' {
        *s = "Nije dostupan".into();
        return;
    }
    *s = format!("{}{}.{}{}.{}{}{}{}.", c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]);
}

/// `YYYYMMDD` -> `DD.MM.YYYY`.
pub fn format_date_ymd(s: &mut String) {
    let c: Vec<char> = s.chars().collect();
    if c.len() != 8 {
        return;
    }
    *s = format!("{}{}.{}{}.{}{}{}{}", c[6], c[7], c[4], c[5], c[0], c[1], c[2], c[3]);
}

/// Join non-empty strings with ", ".
pub fn join_with_comma(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        for (i, e) in [("23051987", "23.05.1987."), ("01010000", "Nije dostupan"), ("123", "123"), ("", "")] {
            let mut s = i.to_string();
            format_date(&mut s);
            assert_eq!(s, e);
        }
        for (i, e) in [("19870523", "23.05.1987"), ("123", "123"), ("", "")] {
            let mut s = i.to_string();
            format_date_ymd(&mut s);
            assert_eq!(s, e);
        }
    }

    #[test]
    fn yes_no() {
        assert_eq!(format_yes_no(true, Language::SrLatin), "Da");
        assert_eq!(format_yes_no(false, Language::Ru), "Нет");
        assert_eq!(format_yes_no(true, Language::En), "Yes");
    }

    #[test]
    fn join() {
        assert_eq!(join_with_comma(&[""]), "");
        assert_eq!(join_with_comma(&["aaa", "", "bb"]), "aaa, bb");
        assert_eq!(join_with_comma(&["", "", "a", "b", "c"]), "a, b, c");
    }

    #[test]
    fn translit() {
        assert_eq!(cyrillic_to_latin("Љубомир Џаковић"), "Ljubomir Džaković");
    }
}
