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
    let err = run(session, "pub async fn main() { nosuchfn() }", DEFAULT_CAPS)
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
    let err = run(session, "pub async fn main() { 42 }", DEFAULT_CAPS)
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
    )
    .await
    .expect_err("千条指令预算应击穿");
    let msg = err.to_string();
    assert!(msg.contains("终态错误"), "{msg}");
}
