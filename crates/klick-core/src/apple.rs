//! Названия моделей iPhone по коду модели. iOS отдаёт приложению только код (`hw.machine`, например
//! `iPhone18,3`), и в панели подписок устройство выглядело как «iPhone18,3» вместо «iPhone 17».
//! Названия латиницей: модель уходит панели заголовком, а в нём только ASCII.

/// «iPhone 17» по `iPhone18,3`; неизвестная модель (новая или iPad) — `None`, тогда показываем код.
pub fn model_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "iPhone8,1" => "iPhone 6s",
        "iPhone8,2" => "iPhone 6s Plus",
        "iPhone8,4" => "iPhone SE",
        "iPhone9,1" | "iPhone9,3" => "iPhone 7",
        "iPhone9,2" | "iPhone9,4" => "iPhone 7 Plus",
        "iPhone10,1" | "iPhone10,4" => "iPhone 8",
        "iPhone10,2" | "iPhone10,5" => "iPhone 8 Plus",
        "iPhone10,3" | "iPhone10,6" => "iPhone X",
        "iPhone11,2" => "iPhone XS",
        "iPhone11,4" | "iPhone11,6" => "iPhone XS Max",
        "iPhone11,8" => "iPhone XR",
        "iPhone12,1" => "iPhone 11",
        "iPhone12,3" => "iPhone 11 Pro",
        "iPhone12,5" => "iPhone 11 Pro Max",
        "iPhone12,8" => "iPhone SE (2nd gen)",
        "iPhone13,1" => "iPhone 12 mini",
        "iPhone13,2" => "iPhone 12",
        "iPhone13,3" => "iPhone 12 Pro",
        "iPhone13,4" => "iPhone 12 Pro Max",
        "iPhone14,2" => "iPhone 13 Pro",
        "iPhone14,3" => "iPhone 13 Pro Max",
        "iPhone14,4" => "iPhone 13 mini",
        "iPhone14,5" => "iPhone 13",
        "iPhone14,6" => "iPhone SE (3rd gen)",
        "iPhone14,7" => "iPhone 14",
        "iPhone14,8" => "iPhone 14 Plus",
        "iPhone15,2" => "iPhone 14 Pro",
        "iPhone15,3" => "iPhone 14 Pro Max",
        "iPhone15,4" => "iPhone 15",
        "iPhone15,5" => "iPhone 15 Plus",
        "iPhone16,1" => "iPhone 15 Pro",
        "iPhone16,2" => "iPhone 15 Pro Max",
        "iPhone17,1" => "iPhone 16 Pro",
        "iPhone17,2" => "iPhone 16 Pro Max",
        "iPhone17,3" => "iPhone 16",
        "iPhone17,4" => "iPhone 16 Plus",
        "iPhone17,5" => "iPhone 16e",
        "iPhone18,1" => "iPhone 17 Pro",
        "iPhone18,2" => "iPhone 17 Pro Max",
        "iPhone18,3" => "iPhone 17",
        "iPhone18,4" => "iPhone Air",
        _ => return None,
    })
}

/// Что показать: название, а если модель незнакомая — сам код.
pub fn model_label(code: &str) -> String {
    model_name(code).map_or_else(|| code.to_string(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_become_names() {
        assert_eq!(model_label("iPhone18,3"), "iPhone 17");
        assert_eq!(model_label("iPhone17,1"), "iPhone 16 Pro");
        assert_eq!(model_label("iPhone12,8"), "iPhone SE (2nd gen)");
        // Незнакомое — как есть: новый iPhone или iPad всё равно узнаваем по коду.
        assert_eq!(model_label("iPhone99,1"), "iPhone99,1");
        assert_eq!(model_label("iPad16,3"), "iPad16,3");
    }

    #[test]
    fn names_are_ascii_for_the_header() {
        for major in 8..=18 {
            for minor in 1..=8 {
                if let Some(n) = model_name(&format!("iPhone{major},{minor}")) {
                    assert!(n.is_ascii(), "{n}");
                }
            }
        }
    }
}
