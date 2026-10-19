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
pub async fn fetch(
    url: &str,
    dom_html: bool,
    markdown: bool,
    timeout_s: u64,
    engine: &EngineIntent,
) -> Result<Value> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_s))
        .user_agent(concat!("browse/", env!("CARGO_PKG_VERSION")))
        .build()?;
    // 渲染 DOM 形（--html）：渲染后的 HTML 需要 JS 执行，恒走引擎腿直取
    // documentElement.outerHTML（不经 HTTP 先试——raw body 不是渲染面）
    if dom_html {
        return fetch_engine_leg(
            url,
            "dom-html",
            LegShape {
                need_html: true,
                include_html: true,
                markdown,
            },
            None,
            timeout_s,
            &client,
            engine,
        )
        .await;
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
            // G-B 解耦：抽取要 html 供转换（need_html=markdown），但 html
            // 字段只在显式 --html 入册（include_html=false），与全局腿同律
            return fetch_engine_leg(
                url,
                why,
                LegShape {
                    need_html: markdown,
                    include_html: false,
                    markdown,
                },
                Some(status),
                timeout_s,
                &client,
                engine,
            )
            .await;
        }
    }
}

/// 引擎腿的输出形三旗标（G-B 解耦后语义各自独立）。
#[derive(Clone, Copy)]
struct LegShape {
    /// 抽取连 `documentElement.outerHTML` 一起带回（markdown 转换要源
    /// html 时置位）。
    need_html: bool,
    /// `html` 字段入册（显式 `--html` 或 dom-html 直取形）。
    include_html: bool,
    /// `markdown` 字段入册（`--markdown`，对抽取带回的 html 做 mdka 转换）。
    markdown: bool,
}

/// 引擎腿（统一实现：三条件升级与 --html 渲染形同源）：goto 后页内抽取
/// 标题/正文，`need_html` 时连 `documentElement.outerHTML`（渲染后的 DOM
/// HTML）一起取；`include_html` 置位才把 html 字段入册（显式 `--html`
/// 或直取形），`markdown` 置位时对抽取带回的 html 做 mdka 转换回
/// `markdown` 字段——两者解耦（`--markdown` 单置不连带 html 入册）。
/// `reason` 是引擎腿成因（三条件升级名或 `dom-html`），`http_status`
/// 是 HTTP 先试腿的状态（直取渲染形没有先试，None）。
async fn fetch_engine_leg(
    url: &str,
    reason: &str,
    shape: LegShape,
    http_status: Option<u16>,
    timeout_s: u64,
    client: &reqwest::Client,
    engine: &EngineIntent,
) -> Result<Value> {
    let extract = fetch_core::extract_js(shape.need_html);
    // 引擎腿 goto 预算随 --timeout 走（评审 G5/kimi G-5）：写死 20 秒让
    // `--timeout 300` 的导航腿先撞 20 秒墙、`--timeout 5` 又等满 20 秒；
    // 页内抽取回整 evaluate 回执（评审 grok P1 + 二轮 F）：成员调用回的
    // 是 CDP 结果**对象**，/eval 信封的 value 随之是对象——CLI 侧按字符串
    // 解析（as_str）必空。回执在片段里 JSON.stringify 成串再回（方言
    // 全局 JSON.stringify），Rust 侧解析后判 exceptionDetails 再取
    // /result/value，成功与异常两路都通
    let code = format!(
        "await goto({url}, {{timeout: {to}}}); return JSON.stringify(await session.Runtime.evaluate({{expression: {expr}, returnByValue: true}}))",
        url = serde_json::to_string(url)?,
        to = timeout_s,
        expr = serde_json::to_string(extract)?,
    );
    // G1：复用 CLI 求值面的 daemon 发现（含 BROWSE_NAME 派生端口），
    // 不只认 BROWSE_PORT 缺省；懒拉起同构（#49）：daemon 不在则
    // 自动拉起，冷启动 fetch 不再要求先手工 browse up
    crate::client::ensure_daemon_with_env(&crate::client::skills_passthrough_env()).await?;
    // 引擎意图透传（fetch 冷附着修）：显式 --connect/--ws/--port 与 eval
    // 面同一语义——冷 daemon 先落附着意图再求值，不再回落 Auto 去本地
    // spawn chrome（容器/远端 chrome 形冷跑 fetch 报「找不到 chrome」的
    // 根因）
    if let Some(p) = engine.up_params() {
        crate::client::engine_up(p).await?;
    }
    let bind = crate::client::daemon_bind();
    let resp = client
        .post(format!("http://{bind}/eval"))
        // 外层 /eval 硬界给 goto 预算之上再加余量（二轮 G5）：goto 用满
        // timeout_s 后还有探针（至多 8 秒）与页内抽取要跑，硬界同数会把
        // 内层预算掐死在导航上；+12 秒余量，客户端 HTTP 侧另有 +60
        .json(&serde_json::json!({ "code": code, "timeout_s": timeout_s + 12 }))
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
    let ev = parse_engine_eval_receipt(&v)?;
    if let Some(exc) = ev.pointer("/exceptionDetails/text").and_then(Value::as_str) {
        let detail = ev
            .pointer("/exceptionDetails/exception/description")
            .and_then(Value::as_str)
            .unwrap_or(exc);
        bail!(
            "引擎腿页内抽取失败：{detail}；下一步：--html 看渲染 DOM、加大 --timeout，或 goto 后手动 evaluate 定位"
        );
    }
    let inner_raw = ev
        .pointer("/result/value")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("引擎腿回执缺 result.value（无异常却无值，形异常）：{ev}"))?;
    let inner: Value =
        serde_json::from_str(inner_raw).map_err(|e| anyhow!("引擎腿抽取值不是合法 JSON：{e}"))?;
    Ok(shape_engine_receipt(
        url,
        reason,
        http_status,
        &inner,
        shape,
        add_domain_skills,
    ))
}

/// /eval 信封到 evaluate 回执的解析（二轮 F 回归锁）：片段回
/// `JSON.stringify` 的串，value 恒字符串；对象形（成员面直返的旧错形，
/// 一轮 P1 修复曾引入）不再静默落 `{}` 空回执——显式报形异常。
fn parse_engine_eval_receipt(v: &Value) -> Result<Value> {
    let raw = v.get("value").and_then(Value::as_str).ok_or_else(|| {
        // 形异常给类型面（二轮 F 教训：对象形曾静默落空回执，归因要能
        // 直接看出「片段漏了 stringify」）
        let kind = match v.get("value") {
            None => "缺席".to_string(),
            Some(Value::String(_)) => "字符串".to_string(),
            Some(Value::Null) => "null".to_string(),
            Some(Value::Bool(_)) => "布尔".to_string(),
            Some(Value::Number(_)) => "数值".to_string(),
            Some(Value::Array(_)) => "数组".to_string(),
            Some(Value::Object(_)) => "对象（片段漏了 JSON.stringify？）".to_string(),
        };
        anyhow!("引擎腿回执形异常（value 应是 JSON.stringify 的串，得到{kind}）")
    })?;
    serde_json::from_str::<Value>(raw).map_err(|e| anyhow!("引擎腿回执不是合法 JSON：{e}"))
}

/// 引擎腿回执整形（纯函数供单测锁旗标契约，评审二轮 F-1）：腿成因与
/// HTTP 先试状态只在该信息存在时入册（`dom-html` 直取形没有）；可选
/// 字段入册源与转换源独立——html 只在 `include_html` 入册，markdown
/// 从抽取带回的 html 转换（升级腿取了源不连带 html 入册）。
fn shape_engine_receipt(
    url: &str,
    reason: &str,
    http_status: Option<u16>,
    inner: &Value,
    shape: LegShape,
    skills: fn(&str, &mut Value),
) -> Value {
    let mut r = json!({
        "url": url, "via": "engine",
        "title": inner.get("title"),
        "text": inner.get("text"),
    });
    if reason != "dom-html" {
        r["upgradedFrom"] = json!(reason);
    }
    if let Some(st) = http_status {
        r["httpStatus"] = json!(st);
    }
    let html_src = inner.get("html").and_then(Value::as_str);
    // 求了没取到给 null 占位（评审 G-8）：键缺席只表「没求」
    fetch_core::attach_optional_fields(
        &mut r,
        shape.include_html.then_some(html_src),
        shape.markdown.then_some(html_src),
    );
    skills(url, &mut r);
    r
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

/// fetch 面的引擎意图（冷附着修）：CLI 解析出的显式连接/引擎旗标束。
/// `up_params` 在有显式意图（ws/port/host/chrome/headless/pipe/profile/
/// proxy/isolated/engine-arg 任一）时出 `UpParams`,与 eval 面的
/// engine_up 触发条件同构。
#[derive(Default)]
pub struct EngineIntent {
    /// `--ws`/`--connect` 的 ws 形。
    pub ws: Option<String>,
    /// `--port`/`--connect` 的端口形。
    pub port: Option<u16>,
    /// `--connect <host:port>` 的 host 段(S009)。
    pub host: Option<String>,
    /// `--chrome`。
    pub chrome: Option<String>,
    /// `--headless`。
    pub headless: bool,
    /// `--pipe`。
    pub pipe: bool,
    /// `--profile`。
    pub profile: Option<String>,
    /// `--proxy`。
    pub proxy: Option<String>,
    /// `--proxy-bypass`。
    pub proxy_bypass: Option<String>,
    /// `--isolated`。
    pub isolated: bool,
    /// `--engine-arg`(可叠)。
    pub engine_args: Vec<String>,
}

impl EngineIntent {
    /// 有任一显式意图才出 [`crate::client::UpParams`](冷 daemon 时先
    /// engine_up 落意图);全缺省返 None(不动引擎,fetch 走 daemon 既有态
    /// 或 Auto)。
    fn up_params(&self) -> Option<crate::client::UpParams> {
        if self.ws.is_none()
            && self.port.is_none()
            && self.host.is_none()
            && self.chrome.is_none()
            && !self.headless
            && !self.pipe
            && self.profile.is_none()
            && self.proxy.is_none()
            && self.proxy_bypass.is_none()
            && !self.isolated
            && self.engine_args.is_empty()
        {
            return None;
        }
        Some(crate::client::UpParams {
            headless: self.headless,
            chrome: self.chrome.clone(),
            ws: self.ws.clone(),
            port: self.port,
            host: self.host.clone(),
            pipe: self.pipe,
            profile: self.profile.clone(),
            proxy: self.proxy.clone(),
            proxy_bypass: self.proxy_bypass.clone(),
            isolated: self.isolated,
            engine_args: self.engine_args.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 引擎腿旗标契约（评审二轮 F-1）：升级腿 `--markdown` 单置——markdown
    /// 从渲染 html 真转换、html 字段不入册；`--html` 直取形两键齐；
    /// 纯旗标形零键。
    #[test]
    fn engine_receipt_flag_contract() {
        let inner = json!({
            "title": "t",
            "text": "body",
            "html": "<main><h1>Head</h1><p>words</p></main>"
        });
        // 升级腿 markdown 单置（need_html=true 供转换，include_html=false）
        let upgraded = shape_engine_receipt(
            "https://spa.test/x",
            "thin-content",
            Some(200),
            &inner,
            LegShape {
                need_html: true,
                include_html: false,
                markdown: true,
            },
            |_, _| {},
        );
        assert!(
            upgraded.get("html").is_none(),
            "md 单置零 html 键: {upgraded}"
        );
        assert!(
            upgraded["markdown"].as_str().unwrap().contains("# Head"),
            "markdown 从渲染 html 真转换: {upgraded}"
        );
        assert_eq!(upgraded["upgradedFrom"], json!("thin-content"));

        // dom-html 直取形：html 与 markdown 齐入册
        let direct = shape_engine_receipt(
            "https://spa.test/x",
            "dom-html",
            None,
            &inner,
            LegShape {
                need_html: true,
                include_html: true,
                markdown: true,
            },
            |_, _| {},
        );
        assert!(
            direct.get("html").is_some() && direct.get("markdown").is_some(),
            "{direct}"
        );
        assert!(
            direct.get("upgradedFrom").is_none(),
            "直取形成因不入册: {direct}"
        );

        // 纯旗标形零可选键
        let bare = shape_engine_receipt(
            "https://spa.test/x",
            "thin-content",
            Some(200),
            &inner,
            LegShape {
                need_html: false,
                include_html: false,
                markdown: false,
            },
            |_, _| {},
        );
        assert!(
            bare.get("html").is_none() && bare.get("markdown").is_none(),
            "{bare}"
        );
    }

    /// #56 域名点名两腿接线：命中时两腿回执形都附 domain_skills 与
    /// hint；未命中（data: URL 无域名段）键集与现状逐字节一致（G4 钉
    /// 进回归网的验收第一条）。
    #[test]
    fn domain_skills_wiring_both_legs() {
        let _env_guard = crate::TEST_ENV_LOCK.lock().unwrap();
        let ws = std::env::temp_dir().join(format!("browse-fetch-sk56-{}", std::process::id()));
        let seg = ws.join("domain-skills").join("x");
        std::fs::create_dir_all(&seg).unwrap();
        // 双脚本对形（清仓重建）：成对才算站点知识
        std::fs::write(seg.join("mech.rn"), "pub async fn main() {}").unwrap();
        std::fs::write(seg.join("probe.user.js"), "// probe").unwrap();
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
            Some(&json!(["mech.rn", "probe.user.js"])),
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
            Some(&json!(["mech.rn", "probe.user.js"])),
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

/// 信封解析回归锁（评审二轮 F）：value 对象形（一轮修复曾引入的错形）
/// 显式报错不落空回执；字符串形正常解析。
#[cfg(test)]
mod envelope_tests {
    use serde_json::json;

    #[test]
    fn parse_engine_eval_receipt_rejects_object_envelope() {
        let env = json!({ "ok": true, "value": { "result": { "value": "x" } } });
        let err = super::parse_engine_eval_receipt(&env).unwrap_err();
        assert!(
            format!("{err:#}").contains("对象"),
            "对象形应报形异常: {err:#}"
        );
    }

    #[test]
    fn parse_engine_eval_receipt_parses_string_envelope() {
        let env = json!({ "ok": true, "value": "{\"exceptionDetails\": null}" });
        let v = super::parse_engine_eval_receipt(&env).unwrap();
        assert_eq!(v.pointer("/exceptionDetails"), Some(&json!(null)));
    }
}
