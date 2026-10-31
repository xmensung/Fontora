//! 拼音检索键：把「思源黑体」变成 `siyuanheiti` / `syht`，让搜索框收拼音。
//!
//! 设计取舍：
//! 1. **多音字全展开**，但设上限。字体名里最常见的多音字是「行」（xing / hang），
//!    「华文行楷」要能被 `hangkai` 命中，所以不能只留第一个读音。展开是笛卡尔积，
//!    名字越长越容易爆炸，所以超过 [`MAX_VARIANTS`] 就退回「只取第一读音」。
//! 2. **英文按词切**，不按字母切。`Source Han Sans SC` 的首字母键是 `shss`，
//!    而不是 16 个字母各取首字母的乱七八糟结果。
//! 3. 只输出小写 ASCII 键；含音调的、非拉丁的、标点的一律不进键。

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::pinyin_data::TABLE;

/// 一个名字最多展开多少种读音组合。超过就不展开多音字。
const MAX_VARIANTS: usize = 24;

/// 一个名字最多返回多少个检索键。
const MAX_KEYS: usize = 48;

fn table() -> &'static HashMap<char, &'static str> {
    static CELL: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut map = HashMap::with_capacity(21_000);
        for line in TABLE.lines() {
            let mut parts = line.split(' ');
            let (Some(ch), Some(read)) = (parts.next(), parts.next()) else {
                continue;
            };
            // 一行只有一个汉字 + 一串读音，取第一个字符即可。
            if let Some(c) = ch.chars().next() {
                map.insert(c, read);
            }
        }
        map
    })
}

/// 读音串（`"xing,hang,heng"`）→ 读音列表。
///
/// 只取前两个。pypinyin 给单字列读音时会把很偏的古音一起列出来
/// （「体」还会给 `cui`、「思」还给 `sai`），全展开只会让 `sc`、`syhc`
/// 这种键到处误命中，正事没干成先添噪。
fn readings(c: char) -> Option<Vec<&'static str>> {
    table().get(&c).map(|raw| raw.split(',').take(2).collect())
}

/// 拼音检索用的一个单位：汉字是「多个读音选一」，英文词是「就是它自己」。
enum Token {
    Han(Vec<&'static str>),
    Word(String),
}

/// 名字 → 检索键。返回值已去重、已小写，顺序稳定。
pub fn keys(name: &str) -> Vec<String> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut word = String::new();
    let mut plain = String::new();

    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            word.push(c.to_ascii_lowercase());
            plain.push(c.to_ascii_lowercase());
            continue;
        }
        if !word.is_empty() {
            tokens.push(Token::Word(std::mem::take(&mut word)));
        }
        if let Some(list) = readings(c) {
            // `plain` 只收 ASCII，汉字跳过，但名字里汉字的位置也要断开英文词。
            tokens.push(Token::Han(list));
        } else if c.is_alphanumeric() {
            // 非汉字、非 ASCII 的字符（假名等）当分隔符，不参与拼音。
        }
    }
    if !word.is_empty() {
        tokens.push(Token::Word(word));
    }

    if tokens.is_empty() {
        return Vec::new();
    }

    let combos: usize = tokens
        .iter()
        .map(|t| match t {
            Token::Han(list) => list.len(),
            Token::Word(_) => 1,
        })
        .product();
    let wide = combos <= MAX_VARIANTS;

    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut push = |key: String| {
        if key.len() >= 2 && seen.insert(key.clone()) && out.len() < MAX_KEYS {
            out.push(key);
        }
    };

    // 全拼与首字母：按每个 token 的选定读音拼起来。
    let mut idx = vec![0usize; tokens.len()];
    loop {
        let mut full = String::new();
        let mut initials = String::new();
        for (i, token) in tokens.iter().enumerate() {
            let piece = match token {
                Token::Han(list) => list.get(if wide { idx[i] } else { 0 }).copied().unwrap_or(""),
                Token::Word(w) => w.as_str(),
            };
            full.push_str(piece);
            if let Some(first) = piece.chars().next() {
                initials.push(first);
            }
        }
        push(full);
        if tokens.len() >= 2 {
            push(initials);
        }

        if !wide {
            break;
        }
        // 进位：多音字依次换读音。
        let mut carry = true;
        for i in (0..tokens.len()).rev() {
            if !carry {
                break;
            }
            let len = match &tokens[i] {
                Token::Han(list) => list.len(),
                Token::Word(_) => 1,
            };
            idx[i] += 1;
            if idx[i] < len {
                carry = false;
            } else {
                idx[i] = 0;
            }
        }
        if carry {
            break;
        }
    }

    // 名字本身就是纯英文时，整串（去空格）也是一个键。
    push(plain);
    out
}

#[cfg(test)]
mod tests {
    use super::keys;

    fn has(name: &str, key: &str) -> bool {
        keys(name).iter().any(|k| k == key)
    }

    #[test]
    fn chinese_name_gives_full_pinyin_and_initials() {
        assert!(has("思源黑体", "siyuanheiti"));
        assert!(has("思源黑体", "syht"));
    }

    #[test]
    fn polyphones_are_expanded() {
        // 行楷读 hang，不是 xing —— 这一条是多音字展开的全部理由。
        assert!(has("华文行楷", "huawenhangkai"));
        assert!(has("华文行楷", "huawenxingkai"));
        assert!(has("华文行楷", "hwhk"));
    }

    #[test]
    fn latin_words_are_split_by_word() {
        assert!(has("Source Han Sans SC", "sourcehansanssc"));
        assert!(has("Source Han Sans SC", "shss"));
        // 不该出现「逐字母取首字母」的怪键。
        assert!(!has("Source Han Sans SC", "shssc"));
    }

    #[test]
    fn ascii_only_name_keeps_its_own_spelling() {
        assert!(has("Helvetica Neue", "helveticaneue"));
    }

    #[test]
    fn long_polyphonic_name_stays_bounded() {
        let name = "长长长长长长长长长长长长长长长长";
        assert!(keys(name).len() <= super::MAX_KEYS);
    }

    #[test]
    fn empty_and_symbol_only_names_give_nothing() {
        assert!(keys("").is_empty());
        assert!(keys("—— ——").is_empty());
    }

    #[test]
    fn mixed_name_covers_both_halves() {
        // 汉字 + 拉丁后缀：全拼是接在一起的，前缀匹配交给调用方（`includes`）。
        assert!(keys("思源黑体 SC").iter().any(|k| k.contains("siyuanheiti")));
        // 后缀本身也是一个键，搜 `sc` 能命中。
        assert!(has("思源黑体 SC", "sc"));
    }
}

