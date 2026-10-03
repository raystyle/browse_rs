//! 一次性只读抓取（#50/#56）：HTTP 优先，三条件升级引擎，markdown 直出。
//!
//! - HTTP 腿：reqwest 直取（零浏览器成本），判升级三条件：正文空、命中
//!   墙词、正文少于 20 词。
//! - 引擎腿：经 daemon POST /eval 走 goto 加页内抽取（启发式标题加段落，
//!   非 Readability；v1 口径已在 surface 披露）。
//! - 单源纪律（REQ-028）：纯函数（粗抽/升级判定/标题/markdown 转换）在
//!   `browse_core::fetch_core`，与方言全局 `fetch(url, opts?)`（rn 经
//!   dialect 桥直达）共用；本文件的腿编排（CLI 进程内 reqwest + /eval
//!   引擎腿）与全局的 daemon 侧编排双轨（ADR-0009 注记形，零 daemon 快
//!   路径是 CLI 既有契约）。
//! - 域名层点名（#56）：两腿回执都按 URL 做 goto 同口径匹配，命中
//!   workspace 站点知识附 domain_skills 与 hint；不依赖引擎（引擎腿
//!   daemon 侧 goto 的点名回执被抽取串替换，CLI 侧补点）。

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use serde_json::json;

use browse_core::fetch_core;

/// 抓取入口：HTTP 优先，命中升级条件转引擎腿。
///
/// # Errors
///
/// 两腿全失败（HTTP 网络错且引擎腿也失败）；引擎腿的 goto/抽取错误原样
/// 上抛。
pub async fn fetch(url: &str, dom_html: bool, markdown: bool, timeout_s: u64) -> Result<Value> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_s))
        .user_agent(concat!("browse/", env!("CARGO_PKG_VERSION")))
        .build()?;
    // 渲染 DOM 形（--html）：渲染后的 HTML 需要 JS 执行，恒走引擎腿直取
    // documentElement.outerHTML（不经 HTTP 先试——raw body 不是渲染面）
    if dom_html {
        return fetch_engine_leg(url, "dom-html", true, markdown, None, timeout_s, &client).await;
    }
    let http = client.get(url).send().await;
    let (status, html) = match http {
        Ok(r) => {
            let s = r.status().as_u16();
            match r.text().await {
                Ok(t) => (s, t),
                Err(_) => (s, String::new()),
            }
        }
        Err(_) => (0, String::new()),
    };
    let title = fetch_core::title_of(&html);
    let text = fetch_core::strip_html(&html);
    let reason = if status == 0 {
        Some("http-error")
    } else {
        fetch_core::needs_upgrade(&text)
    };
    match reason {
        None => {
            let mut r = json!({
                "url": url, "via": "http", "status": status,
                "title": title,
                "text": text,
                "bytes": html.len(),
            });
            // markdown 形（--markdown，mdka 真转换）：转的是本腿手上的 html
            //（HTTP 直通腿是 raw body；升级腿是渲染后的 DOM）
            if markdown {
                r["markdown"] = json!(fetch_core::to_markdown(&html));
            }
            add_domain_skills(url, &mut r);
            Ok(r)
        }
        Some(why) => {
            return fetch_engine_leg(
                url,
                why,
                markdown,
                markdown,
                Some(status),
                timeout_s,
                &client,
            )
            .await;
        }
    }
}

/// 引擎腿（统一实现：三条件升级与 --html 渲染形同源）：goto 后页内抽取
/// 标题/正文，`need_html` 时连 `documentElement.outerHTML`（渲染后的 DOM
/// HTML）一起取；`markdown` 置位时对该 html 做 mdka 转换回 `markdown`
/// 字段。`reason` 是引擎腿成因（三条件升级名或 `dom-html`），`http_status`
/// 是 HTTP 先试腿的状态（直取渲染形没有先试，None）。
async fn fetch_engine_leg(
    url: &str,
    reason: &str,
    need_html: bool,
    markdown: bool,
    http_status: Option<u16>,
    timeout_s: u64,
    client: &reqwest::Client,
) -> Result<Value> {
    let extract = fetch_core::extract_js(need_html);
    let code = format!(
        "await goto({url}, {{timeout: 20}}); return (await session.Runtime.evaluate({{expression: {expr}, returnByValue: true}})).result.value",
        url = serde_json::to_string(url)?,
        expr = serde_json::to_string(extract)?,
    );
    // G1：复用 CLI 求值面的 daemon 发现（含 BROWSE_NAME 派生端口），
    // 不只认 BROWSE_PORT 缺省；懒拉起同构（#49）：daemon 不在则
    // 自动拉起，冷启动 fetch 不再要求先手工 browse up
    crate::client::ensure_daemon_with_env(&crate::client::skills_passthrough_env()).await?;
    let bind = crate::client::daemon_bind();
    let resp = client
        .post(format!("http://{bind}/eval"))
        .json(&serde_json::json!({ "code": code }))
        .timeout(std::time::Duration::from_secs(timeout_s + 60))
        .send()
        .await
        .map_err(|e| {
            // CTA 指日志（#49 评审随记下批项）：daemon 在跑却不可达
            // 多为重启窗口或端口不符，日志比「browse up」更对症
            anyhow!(
                "引擎腿 daemon 不可达（{e}）；下一步：看日志 {}（tail）或 browse status 对端口",
                crate::client::state_dir().join("daemon.log").display()
            )
        })?;
    let v: Value = resp.json().await?;
    // #60：引擎腿信封的引擎上下文告警（与 eval 道同款渲染）
    if let Some(ctx) = v.get("engineContext") {
        crate::client::render_engine_context(ctx);
    }
    if v.get("ok") != Some(&json!(true)) {
        bail!(
            "引擎腿失败：{}",
            v.get("error").and_then(Value::as_str).unwrap_or("?")
        );
    }
    let inner = v
        .get("value")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or(json!({}));
    let mut r = json!({
        "url": url, "via": "engine",
        "title": inner.get("title"),
        "text": inner.get("text"),
    });
    // 腿成因与 HTTP 先试状态只在该信息存在时入册（--html 直取形没有）
    if reason != "dom-html" {
        r["upgradedFrom"] = json!(reason);
    }
    if let Some(st) = http_status {
        r["httpStatus"] = json!(st);
    }
    if need_html {
        r["html"] = inner.get("html").cloned().unwrap_or(Value::Null);
    }
    if markdown {
        let html_src = inner
            .get("html")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();
        r["markdown"] = json!(fetch_core::to_markdown(&html_src));
    }
    add_domain_skills(url, &mut r);
    Ok(r)
}

/// 域名层点名（#56，两腿共用缝）：URL 与 workspace 目录取
/// [`browse_core::skills::url_domain_fields`]，未命中或关闭零插入。
fn add_domain_skills(url: &str, r: &mut Value) {
    // #63 多根序（自定义仓盖默认仓）
    let roots = browse_core::paths::workspace_roots();
    for (k, v) in browse_core::skills::url_domain_fields_multi(&roots, url) {
        if let Some(obj) = r.as_object_mut() {
            obj.insert(k.to_string(), v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #56 域名点名两腿接线：命中时两腿回执形都附 domain_skills 与
    /// hint；未命中（data: URL 无域名段）键集与现状逐字节一致（G4 钉
    /// 进回归网的验收第一条）。
    #[test]
    fn domain_skills_wiring_both_legs() {
        let _env_guard = crate::TEST_ENV_LOCK.lock().unwrap();
        let ws = std::env::temp_dir().join(format!("browse-fetch-sk56-{}", std::process::id()));
        let seg = ws.join("domain-skills").join("x");
        std::fs::create_dir_all(&seg).unwrap();
        std::fs::write(seg.join("notes.md"), "# x").unwrap();
        let saved = std::env::var_os("BROWSE_WORKSPACE");
        // SAFETY: TEST_ENV_LOCK 窗内独占改 env，块尾按原值还原
        unsafe { std::env::set_var("BROWSE_WORKSPACE", &ws) };
        // HTTP 腿回执形：命中附两键
        let mut http = json!({
            "url": "https://www.x.com/a", "via": "http", "status": 200,
            "title": "t", "text": "body", "bytes": 42,
        });
        add_domain_skills("https://www.x.com/a", &mut http);
        assert_eq!(
            http.get("domain_skills"),
            Some(&json!(["notes.md"])),
            "{http}"
        );
        assert!(
            http.get("domain_skills_hint")
                .and_then(Value::as_str)
                .is_some_and(|h| h == "browse workspace site x"),
            "{http}"
        );
        // 引擎腿回执形：同口径命中
        let mut engine = json!({
            "url": "https://www.x.com/a", "via": "engine",
            "upgradedFrom": "thin-content", "httpStatus": 200,
            "title": "t", "text": "body",
        });
        add_domain_skills("https://www.x.com/a", &mut engine);
        assert_eq!(
            engine.get("domain_skills"),
            Some(&json!(["notes.md"])),
            "{engine}"
        );
        // 未命中：键集恰为现状六键（逐字节一致的等价断言，BTreeMap 键序）
        let mut miss = json!({
            "url": "data:text/html,x", "via": "http", "status": 200,
            "title": "t", "text": "body", "bytes": 42,
        });
        add_domain_skills("data:text/html,x", &mut miss);
        let mut keys: Vec<&str> = miss
            .as_object()
            .expect("对象回执")
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["bytes", "status", "text", "title", "url", "via"],
            "未命中零新增键: {miss}"
        );
        // SAFETY: 同一锁窗内按原值还原
        unsafe {
            match &saved {
                Some(v) => std::env::set_var("BROWSE_WORKSPACE", v),
                None => std::env::remove_var("BROWSE_WORKSPACE"),
            }
        }
        let _ = std::fs::remove_dir_all(&ws);
    }
}
