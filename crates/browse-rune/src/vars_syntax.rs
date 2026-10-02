//! 模板语法选择（总台令「{{ 与 jinja 不冲突」批）：四面（rune 源模板
//! 道、方言/--js 片段模板道、js_lib 库体、cdp::render 运行时）同源的
//! 定界形态。缺省 Jinja 经典形 `{{ }}`；ERB 形 `<%= %>` opt-in——`<%`
//! 邻接在合法 JS 中不存在（`a<%b` 即 `a < (%b)`，JS 无 `%` 一元），模
//! 板工程五十年实证的 JS 免撞定界。入口双轨：件首行指令
//! `// browse-vars: erb`（库件自描述，优先）与 CLI `--vars-syntax erb`
//! 旗标（调用方）；同值同效。

/// 模板定界形态。**缺省 ERB**（总台令「默认开启 erb」，0.30.3）：`<%`
/// 邻接在合法 JS 中不存在，`{{ }}`（format! 转义/对象字面量相邻）整类
/// 撞面消；存量 `{{ }}` 件走件首行 `// browse-vars: classic` 回退。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VarsSyntax {
    /// Jinja 经典形：`{{ }}` / `{% %}` / `{# #}`（存量件回退道）。
    Classic,
    /// ERB 形（缺省）：`<%= %>` 槽 / `<% %>` 块 / `<%# %>` 注释。
    #[default]
    Erb,
}

impl VarsSyntax {
    /// 从旗标或请求字段的外部名解析（`classic`/`erb`；未知值回落经典并
    /// 由调用方文案把关）。
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "classic" | "jinja" => Some(Self::Classic),
            "erb" => Some(Self::Erb),
            _ => None,
        }
    }

    /// 外部名。
    pub fn name(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Erb => "erb",
        }
    }
}

/// 件首行指令：`// browse-vars: erb`（或 `classic`）。返回 None 表示无
/// 指令（走缺省/旗标）。指令行本身保持原样进渲染（注释惰性化道会圈它，
/// 对 Jinja 无害）。
pub fn directive_of(source: &str) -> Option<VarsSyntax> {
    let first = source.lines().next()?.trim();
    let rest = first.strip_prefix("// browse-vars:")?;
    VarsSyntax::parse(rest)
}

/// 按形态取 Jinja 引擎（ADR-0009 同配置：Strict 加零 autoescape；两形
/// 各一份 OnceLock）。
pub fn env_for(syntax: VarsSyntax) -> &'static minijinja::Environment<'static> {
    fn build(syntax: VarsSyntax) -> minijinja::Environment<'static> {
        let mut env = minijinja::Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
        if syntax == VarsSyntax::Erb {
            env.set_syntax(
                minijinja::syntax::SyntaxConfig::builder()
                    .block_delimiters("<%", "%>")
                    .variable_delimiters("<%=", "%>")
                    .comment_delimiters("<%#", "%>")
                    .build()
                    .expect("ERB 定界合法"),
            );
        }
        env
    }
    static CLASSIC: std::sync::OnceLock<minijinja::Environment<'static>> =
        std::sync::OnceLock::new();
    static ERB: std::sync::OnceLock<minijinja::Environment<'static>> = std::sync::OnceLock::new();
    match syntax {
        VarsSyntax::Classic => CLASSIC.get_or_init(|| build(VarsSyntax::Classic)),
        VarsSyntax::Erb => ERB.get_or_init(|| build(VarsSyntax::Erb)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_parse() {
        assert_eq!(
            directive_of("// browse-vars: erb\nrest"),
            Some(VarsSyntax::Erb)
        );
        assert_eq!(
            directive_of("// browse-vars: classic\nx"),
            Some(VarsSyntax::Classic)
        );
        assert_eq!(directive_of("pub async fn main() {}"), None);
        assert_eq!(directive_of("// browse-vars: bogus\nx"), None);
    }

    #[test]
    fn erb_default_renders_and_classic_stays() {
        assert_eq!(VarsSyntax::default(), VarsSyntax::Erb, "缺省 ERB");
        let e = env_for(VarsSyntax::default());
        let out = e.render_str("<%= q %> {{ not_a_slot }}", minijinja::context! {q => "v"});
        assert_eq!(out.unwrap(), "v {{ not_a_slot }}");
        let c = env_for(VarsSyntax::Classic);
        let out2 = c.render_str("{{ q }}", minijinja::context! {q => "v"});
        assert_eq!(out2.unwrap(), "v");
        // 块定界：<% if false %> 死分支丢弃、<% for %> 复制
        let out3 = e.render_str(
            "<% if false %>DEAD<% endif %>return [<% for c in list %><%= c %><% endfor %>]",
            minijinja::context! {list => "ab"},
        );
        assert_eq!(out3.unwrap(), "return [ab]");
    }
}
