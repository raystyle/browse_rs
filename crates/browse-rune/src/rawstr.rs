//! r# 裸串预处理（战役台裸串研究单定谳 2026-10-02，批次二落地）：Rune
//! 上游无 r#"…"# 形（rune-rs #537 自 2023-05 在册零动作，#802 重复件已
//! 关），browse 侧在编译前把 r# 裸串重写为转义普通串：任意引号与反
//! 斜杠原样直达 `cdp::js`/`cdp::eval`，多行保真（换行原样入串，编译诊
//! 断行号不漂移）。
//!
//! 管道序（ADR-0010 增补）：CLI 模板道先 [`extract_raw_strings`] 把 r#
//! 区与**注释区**换占位标识符，Jinja 只渲染余下源（r# 区与注释区天然免
//! 渲染，`{{`/`{%`/字面 raw 标签原样——批次三①注释惰性化），渲染毕
//! [`subst_raw_strings`] 回填；免渲染道（无 --vars、管道源码、实参）由
//! [`rewrite_raw_strings`] 在 `run()` 入口兜底（注释占位逐字回填，幂等
//! 空转）。两道单实现在本 crate，CLI 直接依赖防双轨。
//!
//! 词法器是单遍状态机（近似面）：跳过 `//` 与 `/* */` 注释、普通串、
//! 模板串（`${}` 插值内递归按表达式扫）、char 字面量，只重写表达式位
//! 的 r# 记号；非 ASCII 标识符紧贴 r# 的罕见形可能误判，报错即回退普
//! 通串写法。形与 Rust 全同：`r` + N 个 `#` + `"` 开、`"` + N 个 `#`
//! 闭（N 大于等于 1），内容含 `"#` 时升 N。

/// r# 词法错：一行 CTA 形（带行号与下一步）。
#[derive(Debug, Clone)]
pub struct RawStrError {
    /// 人读错误描述。
    pub message: String,
}

impl std::fmt::Display for RawStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RawStrError {}

/// 占位标识符前缀（后接区序号）：合法 Rune 标识符，Jinja 惰性文本。
const PLACEHOLDER_PREFIX: &str = "__BROWSE_RN_RAW_";

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// 内容转义为普通串体：只转反斜杠、双引号与 CR；换行/Tab/UTF-8 原样
/// （多行保真，行号不漂）。
fn escape_body(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for c in content.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// 抽出源内全部 r# 裸串区与注释区：每处记号替换为占位标识符
/// `__BROWSE_RN_RAW_n__`（n 是区序号），返回（占位后的源，各区文本列
/// 表——r# 区是转义普通串字面量含首尾引号，注释区是逐字原文含定界
/// 符）。模板道用：占位源交 Jinja 渲染（两类区都免渲染），渲染毕按序
/// 回填 [`subst_raw_strings`]。
///
/// # Errors
///
/// r# 裸串未闭合、或源已含占位前缀（标识符冲撞）时报带行号 CTA。
pub fn extract_raw_strings(src: &str) -> Result<(String, Vec<String>), RawStrError> {
    extract_with(src, Family::Rune)
}

/// 抽出 JS 族源（方言片段与 `--js` 小程序、js_lib 库体）的注释区（G-1
/// 对齐批）：与 Rune 族同道占位抽取回填，但**只认注释**——r# 是 Rune
/// 语法不在 JS 族重写（字符串里的 `{{ }}` 照旧参与渲染，注释整段惰性）
/// ；正则字面量内的 `//` 不误判（启发式：前一显著字符是运算/开括号类
/// 或行首时 `/` 起正则，正则内 `\` 转义与 `[...]` 字符类吞 `/`）。
///
/// # Errors
///
/// 源已含占位前缀（标识符冲撞）时报 CTA。
pub fn extract_comment_regions(src: &str) -> Result<(String, Vec<String>), RawStrError> {
    extract_with(src, Family::Js)
}

/// 词法族：Rune（r# 加注释）或 Js（仅注释，带正则感知）。
#[derive(Clone, Copy, PartialEq)]
enum Family {
    Rune,
    Js,
}

fn extract_with(src: &str, family: Family) -> Result<(String, Vec<String>), RawStrError> {
    if src.contains(PLACEHOLDER_PREFIX) {
        let what = if family == Family::Rune {
            "r# 预处理"
        } else {
            "注释占位"
        };
        return Err(RawStrError {
            message: format!(
                "源码已含占位前缀 {PLACEHOLDER_PREFIX}（与 {what} 冲撞）；下一步：改用别的标识符名"
            ),
        });
    }
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut literals: Vec<String> = Vec::new();
    // 状态栈：Normal/TplExpr 是表达式面（识别记号），Str/Char/Tpl 是字面
    // 量面（整段拷贝）。Tpl 内遇 ${} 入 TplExpr，其内按表达式扫（含嵌套）。
    #[derive(Clone, Copy, PartialEq)]
    enum Ctx {
        Normal,
        Str,
        Char,
        Tpl,
        TplExpr,
        /// JS 族正则字面量（G-1 对齐批）：`\` 转义与 `[...]` 字符类内 `/`
        /// 不闭合；闭合 `/` 后回表达式面。
        Regex,
    }
    let mut stack: Vec<Ctx> = vec![Ctx::Normal];
    let mut i = 0usize;
    let mut line = 1u32;
    // JS 族正则启发式的前一显著面：上一非空白字节 + 上一标识符（关键字
    // 后可起正则：return /x/ 等）
    let mut prev_sig: Option<u8> = None;
    let mut last_word = String::new();
    while i < bytes.len() {
        // 栈底恒有 Normal（只压不弹到底），防御形取值免 panic 文档面
        let ctx = stack.last().copied().unwrap_or(Ctx::Normal);
        let b = bytes[i];
        match ctx {
            Ctx::Str => {
                if b == b'\\' && i + 1 < bytes.len() {
                    // 转义对整段拷贝（后随字节按 UTF-8 长度推进，防逐字节
                    // as char 的 mojibake——评审 F1）
                    let pair = 1 + utf8_len(bytes[i + 1]);
                    push_bytes(&mut out, bytes, i, pair);
                    i += pair;
                    continue;
                }
                if b == b'"' {
                    stack.pop();
                    prev_sig = Some(b);
                    last_word.clear();
                }
                let ch_len = utf8_len(b);
                push_bytes(&mut out, bytes, i, ch_len);
                i += ch_len;
            }
            Ctx::Char => {
                if b == b'\\' && i + 1 < bytes.len() {
                    let pair = 1 + utf8_len(bytes[i + 1]);
                    push_bytes(&mut out, bytes, i, pair);
                    i += pair;
                    continue;
                }
                if b == b'\'' {
                    stack.pop();
                    prev_sig = Some(b);
                    last_word.clear();
                }
                let ch_len = utf8_len(b);
                push_bytes(&mut out, bytes, i, ch_len);
                i += ch_len;
            }
            Ctx::Tpl => {
                if b == b'\\' && i + 1 < bytes.len() {
                    let pair = 1 + utf8_len(bytes[i + 1]);
                    push_bytes(&mut out, bytes, i, pair);
                    i += pair;
                    continue;
                }
                if b == b'`' {
                    stack.pop();
                    prev_sig = Some(b);
                    last_word.clear();
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                if b == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                    stack.push(Ctx::TplExpr);
                    out.push_str("${");
                    i += 2;
                    prev_sig = Some(b'{');
                    last_word.clear();
                    continue;
                }
                let ch_len = utf8_len(b);
                push_bytes(&mut out, bytes, i, ch_len);
                i += ch_len;
            }
            Ctx::Regex => {
                if b == b'\\' && i + 1 < bytes.len() {
                    let pair = 1 + utf8_len(bytes[i + 1]);
                    push_bytes(&mut out, bytes, i, pair);
                    i += pair;
                    continue;
                }
                if b == b'[' {
                    // 字符类：到 ] 为止（内的 / 不闭合）
                    let start = i;
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j] != b']' {
                        j += utf8_len(bytes[j]);
                    }
                    let end = (j + 1).min(bytes.len());
                    push_bytes(&mut out, bytes, start, end - start);
                    i = end;
                    continue;
                }
                if b == b'/' {
                    stack.pop();
                    prev_sig = Some(b'/');
                }
                let ch_len = utf8_len(b);
                push_bytes(&mut out, bytes, i, ch_len);
                i += ch_len;
            }
            Ctx::Normal | Ctx::TplExpr => {
                // 行注释（表达式面内到行尾）：整段占位抽取（批次三①：注释
                // 区 Jinja 惰性化，字面回填行数不变），换行留在源内
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    let start = i;
                    let end = src[start..]
                        .find('\n')
                        .map(|o| start + o)
                        .unwrap_or(bytes.len());
                    out.push_str(&format!("{PLACEHOLDER_PREFIX}{}__", literals.len()));
                    literals.push(src[start..end].to_string());
                    i = end;
                    prev_sig = Some(b'\n');
                    last_word.clear();
                    continue;
                }
                // 块注释（不嵌套）：整段占位抽取（含定界符逐字回填；未闭合
                // 到 EOF 也整段，无词法错面）
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    let start = i;
                    let close = src[start + 2..].find("*/").map(|o| start + 2 + o);
                    let end = close.map(|c| c + 2).unwrap_or(bytes.len());
                    line += src[start..end].matches('\n').count() as u32;
                    out.push_str(&format!("{PLACEHOLDER_PREFIX}{}__", literals.len()));
                    literals.push(src[start..end].to_string());
                    i = end;
                    prev_sig = Some(b'\n');
                    last_word.clear();
                    continue;
                }
                // JS 族正则字面量起点（启发式：前一显著面属运算/开括号类
                // 或关键字后或行首；两注释形已在上优先截获）
                if family == Family::Js && b == b'/' && regex_allowed(prev_sig, &last_word) {
                    stack.push(Ctx::Regex);
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                if b == b'"' {
                    stack.push(Ctx::Str);
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                if b == b'\'' {
                    stack.push(Ctx::Char);
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                if b == b'`' {
                    stack.push(Ctx::Tpl);
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                if b == b'{' && ctx == Ctx::TplExpr {
                    // TplExpr 内的花括号交由下方字符面处理（深度由闭括号平衡）
                    out.push(b as char);
                    i += 1;
                    prev_sig = Some(b);
                    last_word.clear();
                    // 压一层表达式上下文以平衡
                    stack.push(Ctx::TplExpr);
                    continue;
                }
                if b == b'}' && ctx == Ctx::TplExpr {
                    stack.pop();
                    out.push(b as char);
                    i += 1;
                    prev_sig = Some(b);
                    last_word.clear();
                    continue;
                }
                // 标识符 run：整段消费；Rune 族 run 恰为 "r" 且后随 #+"
                // 时是裸串（Js 族不认 r#）
                if is_word(b) {
                    let start = i;
                    while i < bytes.len() && is_word(bytes[i]) {
                        i += 1;
                    }
                    let word = &src[start..i];
                    last_word = word.to_string();
                    prev_sig = Some(bytes[i - 1]);
                    if family == Family::Rune && word == "r" && i < bytes.len() && bytes[i] == b'#'
                    {
                        let mut hashes = 0usize;
                        let mut j = i;
                        while j < bytes.len() && bytes[j] == b'#' {
                            hashes += 1;
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b'"' && hashes >= 1 {
                            // 找闭引号 + N 个 #（行号取记号起行，报错指开处）
                            let start_line = line;
                            let content_start = j + 1;
                            let mut k = content_start;
                            let mut close: Option<usize> = None;
                            while k < bytes.len() {
                                if bytes[k] == b'"'
                                    && k + hashes < bytes.len()
                                    && (0..hashes).all(|o| bytes[k + 1 + o] == b'#')
                                {
                                    close = Some(k);
                                    break;
                                }
                                if bytes[k] == b'\n' {
                                    line += 1;
                                }
                                k += utf8_len(bytes[k]);
                            }
                            let Some(close_at) = close else {
                                return Err(RawStrError {
                                    message: format!(
                                        "r# 裸串未闭合（起于第 {start_line} 行）；下一步：补闭引号加同数 #，内容含 \"# 时升 r## 形"
                                    ),
                                });
                            };
                            let content = &src[content_start..close_at];
                            out.push_str(&format!("{PLACEHOLDER_PREFIX}{}__", literals.len()));
                            literals.push(format!("\"{}\"", escape_body(content)));
                            i = close_at + 1 + hashes;
                            continue;
                        }
                    }
                    push_bytes(&mut out, bytes, start, i - start);
                    continue;
                }
                // 其余（含 TplExpr 的 `{`、Normal 的花括号、`#{` 对象字面
                // 量、运算符、空白）原样；多字节按 UTF-8 整段
                let ch_len = utf8_len(b);
                push_bytes(&mut out, bytes, i, ch_len);
                if b == b'\n' {
                    line += 1;
                }
                if !b.is_ascii_whitespace() {
                    prev_sig = Some(b);
                    last_word.clear();
                }
                i += ch_len;
            }
        }
    }
    Ok((out, literals))
}

/// JS 正则起点启发式：前一显著字节属运算/开括号类、或前一 token 是
/// 关键字（return/case 等）、或尚无显著前文（行首/起始）。
fn regex_allowed(prev_sig: Option<u8>, last_word: &str) -> bool {
    match prev_sig {
        None => true,
        Some(b) => {
            // 评审 G-1：+ 与 - 不入集（++/-- 后的 / 判除法，防吞同行注
            // 释；代价是 = +/re/ 的病理性一元正则前缀判除法，输出中性）
            matches!(
                b,
                b'(' | b','
                    | b'='
                    | b':'
                    | b'['
                    | b'!'
                    | b'&'
                    | b'|'
                    | b'?'
                    | b'{'
                    | b'}'
                    | b';'
                    | b'*'
                    | b'%'
                    | b'~'
                    | b'^'
                    | b'<'
                    | b'>'
                    | b'/'
                    | b'\n'
            ) || matches!(
                last_word,
                "return"
                    | "typeof"
                    | "case"
                    | "in"
                    | "of"
                    | "new"
                    | "delete"
                    | "void"
                    | "instanceof"
                    | "do"
                    | "else"
            )
        }
    }
}

/// 渲染后回填：占位源里第 n 个占位换 literals 第 n 项的转义普通串字面量。
///
/// # Errors
///
/// 占位缺失（渲染方吃掉了占位文本）时报 CTA。
pub fn subst_raw_strings(rendered: &str, literals: &[String]) -> Result<String, RawStrError> {
    let mut out = rendered.to_string();
    for (n, lit) in literals.iter().enumerate() {
        let ph = format!("{PLACEHOLDER_PREFIX}{n}__");
        // 评审 F-1（0.30.0 对齐轮）：Jinja 控制流会丢弃（死分支 0 次）或
        // 复制（循环体 N 次）占位区间——0 次静默略过（区间被控制流丢
        // 弃）、出现即全量替换（每次复制都回填原文）；注入面由渲染后的
        // 残留前缀检查兜
        if !out.contains(&ph) {
            continue;
        }
        out = out.replace(&ph, lit);
    }
    if out.contains(PLACEHOLDER_PREFIX) {
        return Err(RawStrError {
            message: format!(
                "渲染产物残留占位前缀 {PLACEHOLDER_PREFIX}（值注入面）；下一步：值避开该前缀"
            ),
        });
    }
    Ok(out)
}

/// 全量重写（免渲染道兜底）：源内全部 r# 裸串直接换转义普通串。
///
/// # Errors
///
/// 同 [`extract_raw_strings`]。
pub fn rewrite_raw_strings(src: &str) -> Result<String, RawStrError> {
    let (placed, literals) = extract_raw_strings(src)?;
    subst_raw_strings(&placed, &literals)
}

/// 字节首字节对应的 UTF-8 字符长度（ASCII 记 1；截断的 continuation 字
/// 节也记 1 防死循环，源是合法 UTF-8 时不会出现）。
fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 1,
    }
}

/// 按 UTF-8 整段拷贝字节（源是合法 UTF-8，切片边界安全）。
fn push_bytes(out: &mut String, bytes: &[u8], start: usize, len: usize) {
    out.push_str(std::str::from_utf8(&bytes[start..start + len]).unwrap_or("\u{fffd}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_quotes_and_backslashes_verbatim() {
        let out = rewrite_raw_strings(
            r##"fn f() { let s = r#"try { f(); } catch (e) {} const t = `${{x:1}}`;"#; s }"##,
        )
        .unwrap();
        assert_eq!(
            out,
            "fn f() { let s = \"try { f(); } catch (e) {} const t = `${{x:1}}`;\"; s }"
        );
    }

    #[test]
    fn multiline_preserved_line_count() {
        let src = "let a = 1;\nlet s = r#\"line'1\"\nline\\2\nline3\"#;\nlet b = 2;";
        let out = rewrite_raw_strings(src).unwrap();
        assert_eq!(out.lines().count(), src.lines().count(), "行数不漂");
        // 内容引号转义、反斜杠翻倍、换行原样
        assert!(out.contains("\"line'1\\\"\nline\\\\2\nline3\""), "{out}");
    }

    #[test]
    fn escalated_hashes_terminate_only_at_n() {
        // r## 形：内容含 "# 不误闭
        let src = r###"let s = r##"a "# b"##; s"###;
        let out = rewrite_raw_strings(src).unwrap();
        assert_eq!(out, "let s = \"a \\\"# b\"; s");
    }

    #[test]
    fn unterminated_reports_line() {
        let err = rewrite_raw_strings("let s = r#\"abc\n").unwrap_err();
        assert!(err.message.contains("第 1 行"), "{}", err.message);
        assert!(err.message.contains("r##"), "{}", err.message);
    }

    #[test]
    fn comment_and_string_text_untouched() {
        // 注释里的 r# 文本与普通串里的 r# 文本都不重写
        let src = r##"// r#"comment"# not raw
let t = "r#\"inside\"#";
let s = r#"real"#; s"##;
        let out = rewrite_raw_strings(src).unwrap();
        assert!(out.contains("// r#\"comment\"# not raw"), "{out}");
        assert!(out.contains("\"r#\\\"inside\\\"#\""), "{out}");
        assert!(out.contains("\"real\""), "{out}");
    }

    #[test]
    fn template_expr_scans_nested_literals() {
        // 模板串插值内含普通串与 r# 裸串（合法表达式位）；r# 区整段重写
        let src = r##"let t = `v=${f("x") + r#"y"z"#}`; t"##;
        let out = rewrite_raw_strings(src).unwrap();
        assert!(out.contains("${f(\"x\") + \"y\\\"z\"}"), "{out}");
    }

    #[test]
    fn object_literal_and_braces_not_confused() {
        let src = r##"let o = #{k: "v"}; let s = r#"#{not-an-object}"#; o"##;
        let out = rewrite_raw_strings(src).unwrap();
        assert!(out.contains("#{k: \"v\"}"), "{out}");
        assert!(out.contains("\"#{not-an-object}\""), "{out}");
    }

    #[test]
    fn extract_then_subst_roundtrip() {
        let src = r##"let q = "{{ x }}"; let s = r#"keep {{ x }} verbatim"#; q"##;
        let (placed, literals) = extract_raw_strings(src).unwrap();
        assert!(placed.contains("{{ x }}"), "{placed}");
        assert!(!placed.contains("verbatim"), "{placed}");
        // 模拟 Jinja 渲染：只动占位源里的占位外文本
        let rendered = placed.replace("{{ x }}", "SUBST");
        let out = subst_raw_strings(&rendered, &literals).unwrap();
        assert!(out.contains("\"SUBST\""), "{out}");
        assert!(out.contains("keep {{ x }} verbatim"), "{out}");
    }

    #[test]
    fn identifier_prefix_r_is_not_raw() {
        let src = "let x = foor#\"not raw\"#;";
        // foor 后的 r#" 属于词法歧义面：整段 run 消费后 # " 原样拷贝
        let out = rewrite_raw_strings(src).unwrap();
        assert_eq!(out, src, "标识符 run 内不误判");
    }

    #[test]
    fn placeholder_prefix_collision_errors() {
        let err = rewrite_raw_strings("let x = __BROWSE_RN_RAW_0__;").unwrap_err();
        assert!(err.message.contains("占位前缀"), "{}", err.message);
    }

    /// 评审 F1 回归锁：普通串/char/模板串/行注释里的非 ASCII 必须整段
    /// UTF-8 拷贝（逐字节 as char 是 mojibake 静默变形）。
    #[test]
    fn non_ascii_text_verbatim_in_all_faces() {
        let src = "// 中文注释\nlet a = \"中文ABC\";\nlet c = '中';\nlet t = `值=${1}文`;\nlet r = r#\"中文裸串\"#;\nlet tail = \"尾串\"";
        let out = rewrite_raw_strings(src).unwrap();
        assert!(out.contains("// 中文注释"), "{out}");
        assert!(out.contains("\"中文ABC\""), "{out}");
        assert!(out.contains("'中'"), "{out}");
        assert!(out.contains("`值=${1}文`"), "{out}");
        assert!(out.contains("\"中文裸串\""), "{out}");
        assert!(out.contains("\"尾串\""), "{out}");
    }

    /// 评审 F-1（0.30.0 对齐轮）：占位复制（循环体）合法全量替换；残留
    /// 前缀（值注入面）仍拦；死分支丢弃由 0 次静默略过（client 级测覆
    /// 盖真 Jinja 面）。
    #[test]
    fn subst_replaces_replicas_and_rejects_residual() {
        let (placed, literals) = extract_raw_strings("let s = r#\"x\"#;").unwrap();
        // 循环体复制：两次出现都回填
        let looped = placed.replace(
            "__BROWSE_RN_RAW_0__",
            "__BROWSE_RN_RAW_0__ __BROWSE_RN_RAW_0__",
        );
        let out = subst_raw_strings(&looped, &literals).unwrap();
        assert_eq!(out, "let s = \"x\" \"x\";", "{out}");
        // 残留臂：占位 0 被丢弃，渲染注入了异序号前缀
        let residual = placed.replace("let s =", "let extra = __BROWSE_RN_RAW_7__; let s =");
        let err = subst_raw_strings(&residual, &literals).unwrap_err();
        assert!(err.message.contains("残留"), "{}", err.message);
    }

    /// 评审 G-1（0.30.0 对齐轮）：`++`/`--` 后的 `/` 判除法，行尾注释仍
    /// 被抽出（启发式收紧钉）。
    #[test]
    fn js_family_postfix_increment_then_division_keeps_comment() {
        let src = "let a = b++ / c; // note {{x}}";
        let (placed, literals) = extract_comment_regions(src).unwrap();
        assert!(!placed.contains("note"), "注释应被抽出：{placed}");
        let out = subst_raw_strings(&placed, &literals).unwrap();
        assert_eq!(out, src);
    }

    /// 批次三①：注释区 Jinja 惰性化——注释内的 `{{ }}`/`{% raw %}` 字面
    /// 标签/`r#` 形文本都不进渲染，逐字回填；普通串照旧渲染。
    #[test]
    fn comments_are_jinja_inert() {
        let src = "// 注释 {{ q }} 与 {% raw %} 与 r#\"x\"#\nlet s = \"{{ q }}\";";
        let (placed, literals) = extract_raw_strings(src).unwrap();
        // 注释整段被占位（渲染器看不到），普通串的 {{ q }} 可见
        assert!(!placed.contains("注释"), "{placed}");
        assert!(placed.contains("{{ q }}"), "{placed}");
        let rendered = placed.replace("{{ q }}", "SUBST");
        let out = subst_raw_strings(&rendered, &literals).unwrap();
        assert!(
            out.contains("// 注释 {{ q }} 与 {% raw %} 与 r#\"x\"#\n"),
            "{out}"
        );
        assert!(out.contains("\"SUBST\""), "{out}");
        // 行数不变（换行留在源内）
        assert_eq!(out.lines().count(), src.lines().count());
    }

    /// 批次三① 边界：EOF 无换行的行注释与未闭合块注释整段惰性回填。
    #[test]
    fn comment_edges_eof_and_unterminated_block() {
        let src = "let a = 1; // 尾注释无换行 {{ x }}";
        let (placed, literals) = extract_raw_strings(src).unwrap();
        assert!(!placed.contains("尾注释"), "{placed}");
        let out = subst_raw_strings(&placed, &literals).unwrap();
        assert_eq!(out, src, "逐字回填");

        let src2 = "let a = 1; /* 未闭合到 EOF {{ y }}";
        let (placed2, literals2) = extract_raw_strings(src2).unwrap();
        assert!(!placed2.contains("未闭合"), "{placed2}");
        let out2 = subst_raw_strings(&placed2, &literals2).unwrap();
        assert_eq!(out2, src2, "未闭合块注释整段回填");
    }

    /// G-1 对齐批：JS 族（方言/--js/js_lib）注释惰性、字符串照旧渲染、
    /// r# 不重写。
    #[test]
    fn js_family_comments_inert_strings_active() {
        let src = r##"// 注释 {{ nope }} 与 {% raw %} 与 r#"x"#
const a = "串内 {{ q }} 渲染";
const b = '单引串 {{ q }} 同渲染';
/* 块注释 {{ nope2 }} */"##;
        let (placed, literals) = extract_comment_regions(src).unwrap();
        assert!(!placed.contains("nope"), "注释占位后不可见：{placed}");
        assert!(placed.contains("{{ q }}"), "字符串槽可见：{placed}");
        let rendered = placed.replace("{{ q }}", "SUBST");
        let out = subst_raw_strings(&rendered, &literals).unwrap();
        assert!(
            out.contains("// 注释 {{ nope }} 与 {% raw %} 与 r#\"x\"#\n"),
            "{out}"
        );
        assert!(out.contains("\"串内 SUBST 渲染\""), "{out}");
        assert!(out.contains("'单引串 SUBST 同渲染'"), "{out}");
        assert!(out.contains("/* 块注释 {{ nope2 }} */"), "{out}");
    }

    /// G-1 对齐批：正则字面量内的 `//` 不误判注释；除法上下文不起正则。
    #[test]
    fn js_family_regex_and_division() {
        let src = r#"const re = /https:\/\/x\/{{q}}/; const d = a / b; // 真注释 {{ nope }}"#;
        let (placed, literals) = extract_comment_regions(src).unwrap();
        // 正则整体未被当注释（其后的分号与除法在 placed 里），行尾真注释占位
        assert!(!placed.contains("真注释"), "{placed}");
        assert!(placed.contains("a / b"), "除法保留：{placed}");
        let rendered = placed.replace("{{q}}", "Q");
        let out = subst_raw_strings(&rendered, &literals).unwrap();
        assert!(
            out.contains("/https:\\/\\/x\\/Q/"),
            "正则内 {{q}} 仍渲染（字符串/正则不惰性）：{out}"
        );
        assert!(out.contains("// 真注释 {{ nope }}"), "{out}");
    }

    /// G-1 对齐批：模板串插值内的注释与正则（表达式面同规），模板文本
    /// 里的槽照旧渲染（字符串面活）。
    #[test]
    fn js_family_template_expr_scans() {
        let src = "const t = `v=${f(/a\\//b/) /* c {{ nope }} */} {{ q }}`;";
        let (placed, literals) = extract_comment_regions(src).unwrap();
        assert!(!placed.contains("nope"), "{placed}");
        let out = subst_raw_strings(&placed.replace("{{ q }}", "S"), &literals).unwrap();
        assert!(out.contains("/* c {{ nope }} */"), "{out}");
        assert!(out.contains("} S`;"), "模板文本槽渲染：{out}");
    }
}
