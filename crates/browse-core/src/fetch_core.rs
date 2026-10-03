//! fetch 的纯函数面（REQ-028 单源）：HTTP 腿的正文粗抽与升级判定、页内
//! 抽取 JS 构造、markdown 转换。何处分流：CLI `browse fetch`（零 daemon
//! 快路径）与方言全局 `fetch(url, opts?)`（daemon 侧，rn 经 dialect 桥
//! 直达）两道编排共用本面；两道编排各自持腿逻辑（HTTP 客户端与引擎腿
//! 拼装不同：CLI 引擎腿走 /eval、全局直调本进程 session），属库内双轨
//! （ADR-0009 同款注记，漂移由各自测试锁）。

use serde_json::{Value, json};

/// 升级判定的三条件（#50）：正文空、墙词、少于 20 词。
/// 纯函数供单测锁。
pub fn needs_upgrade(body_text: &str) -> Option<&'static str> {
    let t = body_text.trim();
    if t.is_empty() {
        return Some("empty");
    }
    const WALLS: [&str; 8] = [
        "just a moment",
        "attention required",
        "checking your browser",
        "verify you are human",
        "enable javascript",
        "access denied",
        "unusual traffic",
        "captcha",
    ];
    let low = t.to_lowercase();
    if WALLS.iter().any(|w| low.contains(w)) {
        return Some("wall-word");
    }
    let words = low.split_whitespace().count();
    if words < 20 {
        return Some("thin-content");
    }
    None
}

/// HTML 粗抽正文文本（HTTP 腿的降级面）：剥 script/style 与标签，压空白。
///
/// 标记匹配对 `html` 本体做 ASCII 不敏感比较（全量审 H1 修）：不构造
/// 整串 `to_lowercase` 再共用字节下标——U+0130（İ）/U+1E9E（ẞ）等小写化
/// 改变字节长度的字符会让下标在 lower 里落进字符中间直接 panic（土耳其
/// 语页实测崩）。HTML 标记名恒 ASCII，ASCII 等值即足。
pub fn strip_html(html: &str) -> String {
    fn starts_with_tag(s: &str, tag: &str) -> bool {
        let b = s.as_bytes();
        let t = tag.as_bytes();
        b.len() >= t.len() && b.iter().zip(t).all(|(x, y)| x.to_ascii_lowercase() == *y)
    }
    fn find_ascii(haystack: &str, needle: &str) -> Option<usize> {
        let hb = haystack.as_bytes();
        let nb = needle.as_bytes();
        // 短串守卫（全量审二轮 F-1）：haystack 短于 needle 时裸切越界 panic
        if hb.len() < nb.len() {
            return None;
        }
        (0..=hb.len() - nb.len()).find(|&i| {
            hb[i..i + nb.len()]
                .iter()
                .zip(nb)
                .all(|(x, y)| x.to_ascii_lowercase() == *y)
        })
    }
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut skip_until: Option<&str> = None;
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let rest = &html[i..];
        if let Some(end) = skip_until {
            if let Some(pos) = find_ascii(rest, end) {
                i += pos + end.len();
                skip_until = None;
                in_tag = false;
                continue;
            }
            break;
        }
        if !in_tag && starts_with_tag(rest, "<script") {
            skip_until = Some("</script>");
            i += 7;
            continue;
        }
        if !in_tag && starts_with_tag(rest, "<style") {
            skip_until = Some("</style>");
            i += 6;
            continue;
        }
        if !in_tag && starts_with_tag(rest, "<title") {
            skip_until = Some("</title>");
            i += 6;
            continue;
        }
        let ch = rest.chars().next().unwrap_or(' ');
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
        i += ch.len_utf8();
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 标题抽取（HTTP 腿）：开标记 ASCII 不敏感、闭标记大小写敏感（原行为），
/// 全程在 `html` 本体做（同 H1/F-1 纪律）。
pub fn title_of(html: &str) -> String {
    fn find_ascii(haystack: &str, needle: &str) -> Option<usize> {
        let hb = haystack.as_bytes();
        let nb = needle.as_bytes();
        if hb.len() < nb.len() {
            return None;
        }
        (0..=hb.len() - nb.len()).find(|&i| {
            hb[i..i + nb.len()]
                .iter()
                .zip(nb)
                .all(|(x, y)| x.to_ascii_lowercase() == *y)
        })
    }
    find_ascii(html, "<title")
        .and_then(|p| html[p..].find('>').map(|q| p + q + 1))
        .and_then(|start| html[start..].find("</title>").map(|e| (start, e)))
        .map(|(s, e)| html[s..s + e].trim().to_string())
        .unwrap_or_default()
}

/// 引擎腿的页内抽取 JS（主世界 returnByValue 求值体）：标题、正文
/// innerText，`need_html` 时连渲染后的 `documentElement.outerHTML`。
pub fn extract_js(need_html: bool) -> &'static str {
    if need_html {
        r#"JSON.stringify({title: document.title, text: (document.body ? document.body.innerText : '').trim(), html: document.documentElement.outerHTML})"#
    } else {
        r#"JSON.stringify({title: document.title, text: (document.body ? document.body.innerText : '').trim()})"#
    }
}

/// markdown 转换（REQ-028，mdka）：字符串转换不可失败，空串安全。
pub fn to_markdown(html: &str) -> String {
    mdka::html_to_markdown(html)
}

/// fetch 回执的可选字段拼装（两道编排共用）：html 字段（渲染形）与
/// markdown 字段（对本腿 html 转换）。
pub fn attach_optional_fields(r: &mut Value, html_src: Option<&str>, markdown: bool) {
    if let Some(h) = html_src {
        r["html"] = json!(h);
    }
    if markdown {
        let src = html_src.unwrap_or_default();
        r["markdown"] = json!(to_markdown(src));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_conditions() {
        assert_eq!(needs_upgrade(""), Some("empty"));
        assert_eq!(needs_upgrade("   "), Some("empty"));
        assert_eq!(
            needs_upgrade("Please wait while we are checking your browser"),
            Some("wall-word")
        );
        assert_eq!(
            needs_upgrade("Access Denied. You are blocked."),
            Some("wall-word")
        );
        // 少于 20 词
        assert_eq!(needs_upgrade("short page"), Some("thin-content"));
        // 健康正文（20 词以上、无墙词）不升级
        let healthy = "word ".repeat(25);
        assert_eq!(needs_upgrade(&healthy), None);
    }

    #[test]
    fn strip_html_basic() {
        let html = "<html><head><title>T</title><style>.a{}</style></head>\
                    <body><script>var x=1;</script><h1>Head</h1><p>Body text here</p></body></html>";
        let t = strip_html(html);
        assert_eq!(t, "HeadBody text here");
    }

    #[test]
    fn markdown_conversion_smoke() {
        let md = to_markdown("<h1>Hello</h1>\n<p>world <b>b</b></p>");
        assert!(md.contains("# Hello"), "{md}");
        assert!(md.contains("world"), "{md}");
        assert!(md.contains("**b**") || md.contains("b"), "{md}");
        // 空与短串零崩（同 F-1 纪律；mdka 空串回空串）
        assert_eq!(to_markdown(""), "");
        let _ = to_markdown("x");
    }

    /// 二轮 F-1 回归锁：短串与未闭合标记不越界 panic（空 html 的标题
    /// 抽取、未闭合 script/style/title 的正文抽取，旧扫描器饱和下标
    /// 实测崩）。
    #[test]
    fn strip_html_short_and_unclosed_inputs_no_panic() {
        assert_eq!(strip_html(""), "");
        assert_eq!(strip_html("abc"), "abc");
        assert_eq!(strip_html("<script>"), "");
        assert_eq!(strip_html("<title>"), "");
        assert_eq!(strip_html("<script src=x"), "");
        assert_eq!(strip_html("<style>"), "");
        // 等价标题抽取面：空串与短串不 panic、无标题得空串
        assert_eq!(title_of(""), "");
        assert_eq!(title_of("abc"), "");
        // 开标记 ASCII 不敏感命中；闭标记查找本大小写敏感（原行为保持）
        assert_eq!(title_of("<TITLE>t</title>"), "t");
        assert_eq!(title_of("<TITLE>T</TITLE>"), "");
    }
}
