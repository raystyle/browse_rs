//! #67 VM 面单测：执行器（双限额、终态错误、编译错 CTA、serde 过桥
//! 形）。纯语言面（不触 cdp 函数），Session::new() 无连接构造即可——
//! cdp 函数面走真 chrome e2e（browse-core 侧）。

use browse_rune::{Caps, DEFAULT_CAPS, run};

/// 纯脚本执行：值过桥形、json 模块、成功载荷。
#[tokio::test]
async fn pure_script_value_bridge() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    let out = run(
        session,
        r#"
        pub async fn main() {
            let o = json::from_string("{\"a\": 42}")?;
            Ok(o["a"])
        }
        "#,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    assert_eq!(out, serde_json::json!({ "ok": 42 }), "回执形：ok 载荷");
    Ok(())
}

/// 脚本自造错误型（方案：Timeout 属脚本可见失败面）走 Err 载荷。
#[tokio::test]
async fn script_defined_error_payload() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    let out = run(
        session,
        r#"
        struct Timeout;
        pub async fn main() { Err(Timeout) }
        "#,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    assert_eq!(
        out,
        serde_json::json!({ "error": "Timeout" }),
        "脚本错误型回执形（Debug 兜底，json 序列化不了脚本型）"
    );
    Ok(())
}

/// 编译错：CTA 文案与不泄漏 panic。
#[tokio::test]
async fn compile_error_cta() {
    let session = cdp::Session::new();
    let err = run(
        session,
        "pub async fn main() { nosuchfn() }",
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await
    .expect_err("未定义函数应编译红");
    let msg = err.to_string();
    assert!(msg.contains("Rune 编译失败"), "{msg}");
    assert!(msg.contains("下一步"), "{msg}");
}

/// 非 Result 入口（评审一轮 F1）：文案点名入口契约，不误报回传道异常。
#[tokio::test]
async fn non_result_main_gets_entry_form_cta() {
    let session = cdp::Session::new();
    let err = run(
        session,
        "pub async fn main() { 42 }",
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await
    .expect_err("裸值入口应按契约红");
    let msg = err.to_string();
    assert!(msg.contains("未按契约返回 Result"), "{msg}");
    assert!(msg.contains("pub async fn main()"), "{msg}");
}

/// 指令预算击穿：VmError 终态（不重试面）。
#[tokio::test]
async fn budget_cap_is_terminal() {
    let session = cdp::Session::new();
    let err = run(
        session,
        "pub async fn main() { let s = 0; for i in 0..100000 { s += i } Ok(s) }",
        Caps {
            instructions: 1000,
            memory: 1024 * 1024,
        },
        browse_rune::RunOpts::default(),
    )
    .await
    .expect_err("千条指令预算应击穿");
    let msg = err.to_string();
    assert!(msg.contains("终态错误"), "{msg}");
}

/// v0.26.1 探针语义：cdp 面快错（无连接）后探针清空，不残留上一次的
/// 在飞名（超时臂读到残留会误报卡点）。
#[tokio::test]
async fn op_probe_clears_after_fast_error() {
    let session = cdp::Session::new();
    let probe = browse_rune::OpProbe::default();
    let out = run(
        session,
        r#"pub async fn main() { cdp::eval("1").await?; Ok(()) }"#,
        DEFAULT_CAPS,
        browse_rune::RunOpts {
            op_probe: probe.clone(),
            ..Default::default()
        },
    )
    .await;
    // 无连接：eval 快错走 Err(CdpError)，回执是 error 面（或 ok 面皆不
    // 影响断言——关键是 run 正常收尾）
    let _ = out;
    assert_eq!(
        probe.lock().ok().and_then(|g| g.first().copied()),
        None,
        "cdp 面正常退出（含快错）后探针应清空"
    );
}

/// 批次二 r# 裸串：run() 入口重写后脚本可用 r#"…"# 嵌任意引号与反斜杠
/// 的 JS 串（多行保真），值原样出。
#[tokio::test]
async fn raw_string_script_face() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    let out = run(
        session,
        r###"pub async fn main() {
        let js = r#"const o = {a: 1, b: {c: 2}}; const re = /\d+"\w+/g; try { f(); } catch (e) {} const t = `${{deep: 1}}`; return [o.a, `${{x:1}}`.length];"#;
        let lit = r##"含 "# 升级形"##;
        Ok(format!("{}|{}", js.len() > 80, lit))
    }"###,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    assert_eq!(out, serde_json::json!({ "ok": "true|含 \"# 升级形" }));
    Ok(())
}

/// #70 补名腿端到端：缺失实例函数的终态错误带方法名（哈希保留）。
#[tokio::test]
async fn missing_fn_error_carries_name() {
    let session = cdp::Session::new();
    let err = run(
        session,
        r#"pub async fn main() { let s = String::from("x"); Ok(s.to_string()) }"#,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await
    .expect_err("to_string 缺席应终态红");
    let msg = err.to_string();
    assert!(
        msg.contains("to_string（哈希 0x871c12da43b176d2）"),
        "{msg}"
    );
}

/// 评审 F1 回归锁（run() 全道）：非 ASCII 串载荷经 r# 预处理入口后
/// 原样（v0.26 语义零回归）。
#[tokio::test]
async fn non_ascii_string_face() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    let out = run(
        session,
        "pub async fn main() { Ok(\"中文ABC 标题\") }",
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    assert_eq!(out, serde_json::json!({ "ok": "中文ABC 标题" }));
    Ok(())
}

/// 三形态批：render 的 Jinja 面（Strict 未填报 op、default 过滤器通行）
/// 与 js_wrapper 的转义纪律（args JSON 字面量注入，单双引号反引号原样）。
#[tokio::test]
async fn render_and_js_wrapper_face() -> anyhow::Result<()> {
    use browse_rune::run;
    let session = cdp::Session::new();
    let out = run(
        session,
        r##"pub async fn main() {
        let t = "f(<%= q %>, <%= n | default(7) %>)";
        cdp::render(t, #{"q": "he said \"hi\""})
    }"##,
        browse_rune::DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    assert_eq!(
        out,
        serde_json::json!({ "ok": "f(he said \"hi\", 7)" }),
        "render 过桥形（含双引号值原样直插）"
    );

    let session = cdp::Session::new();
    let err = run(
        session,
        r#"pub async fn main() { cdp::render("<%= missing %>", #{}) }"#,
        browse_rune::DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    let op = err
        .pointer("/error/op")
        .and_then(|o| o.as_str())
        .unwrap_or("");
    assert_eq!(op, "render", "CdpError 降形保 op/message：{err}");
    Ok(())
}

/// 通用命令通道 raw（无连接进程内态）：空串等价空对象（预检过、走到
/// 连接检查），快错面是 op=raw 的 CdpError——证明函数注册可达。
#[tokio::test]
async fn raw_reachable_and_empty_params() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    let out = run(
        session,
        r#"pub async fn main() { cdp::raw("Network.enable", "").await?; Ok(()) }"#,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    let op = out
        .pointer("/error/op")
        .and_then(|o| o.as_str())
        .unwrap_or("");
    let msg = out
        .pointer("/error/message")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    assert_eq!(op, "raw", "无连接应走 CdpError(op=raw)：{out}");
    assert!(msg.contains("Not connected"), "快错面是连接错：{out}");
    Ok(())
}

/// raw 参数串预检：坏 JSON 与非对象形都在发命令前拦（与上一件的
/// Not connected 面可区分——预检先于连接检查）。
#[tokio::test]
async fn raw_params_preflight() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    for (bad, want) in [
        ("{oops", "不是合法 JSON"),
        ("[1, 2]", "对象"),
        ("42", "对象"),
    ] {
        let src = format!(
            "pub async fn main() {{ cdp::raw(\"X.y\", {:?}).await?; Ok(()) }}",
            bad
        );
        let out = run(
            session.clone(),
            &src,
            DEFAULT_CAPS,
            browse_rune::RunOpts::default(),
        )
        .await?;
        let msg = out
            .pointer("/error/message")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        assert!(
            msg.contains(want) && !msg.contains("Not connected"),
            "params={bad} 应在预检拦「{want}」而非连接面：{out}"
        );
    }
    Ok(())
}

/// 设计注记实测锚（raw 的 params 取 JSON 串形而非 rune 对象）：对象形
/// 本身可行——嵌套对象/数组/标量与 null（经 `()` 单位值）都过桥；选串形
/// 是协议文档参数形原样粘贴（r# 免转义，与 wait_event 任意 method 名对
/// 偶）加避开外部类型的序列化错面，不是对象形坏了。
#[tokio::test]
async fn raw_string_params_rationale_probe() -> anyhow::Result<()> {
    let session = cdp::Session::new();
    let out = run(
        session.clone(),
        r#"pub async fn main() {
        let v = #{"x": 1, "arr": [1, "a", true], "o": #{"k": 2}};
        Ok(json::to_string(v)?)
    }"#,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    let s = out.pointer("/ok").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        s.contains("\"x\":1")
            && s.contains("\"arr\":[1,\"a\",true]")
            && s.contains("\"o\":{\"k\":2}"),
        "嵌套组合对象字面量应可序列化：{out}"
    );
    // null 面：() 单位值过桥即 JSON null（对象形并非表达不了 null）
    let out = run(
        session,
        r#"pub async fn main() {
        let v = #{"v": ()};
        Ok(format!("{:?}", json::to_string(v)))
    }"#,
        DEFAULT_CAPS,
        browse_rune::RunOpts::default(),
    )
    .await?;
    let s2 = out.pointer("/ok").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        s2.contains("\\\"v\\\":null"),
        "() 应过桥为 null（设计注记的实测锚）：{out}"
    );
    Ok(())
}
