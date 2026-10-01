//! 自注册 `cdp` 模块（账本 #67）：八个效果函数全走自有 [`cdp::Session`]
//! 的 `call`，Chrome 只经隔离世界 `Runtime.evaluate` 碰页面。
//!
//! 纪律：参数经 `serde_json::to_string` 做成 JS 字面量再传入（禁止把
//! 选择器拼进 JS 字符串）；小结果走 JSON 过桥（`() ↔ null`、`#{ } ↔
//! object`、数组对数组），DOM 节点只留 [`ObjHandle`] 不透明句柄；对象
//! 组随会话记账（本组恒名 `browse-rune`，执行收尾整组
//! `Runtime.releaseObjectGroup`）；会话句柄留 Rust 侧，不进脚本全局。

use std::sync::Arc;

use anyhow::Result;
use rune::runtime::Value as RuneValue;
use rune::{Any, Module};
use serde_json::{Value, json};

/// 脚本可见的 CDP 失败型（方案错误约定）：`op` 是失败操作名，`message`
/// 是给人读的细节；脚本用 `?` 传播或自构返回。
#[derive(Debug, Clone, Any)]
#[rune(item = ::cdp)]
pub struct CdpError {
    /// 失败操作名（如 `"goto"`、`"eval"`、`"click"`）。
    #[rune(get)]
    pub op: String,
    /// 失败细节（页面异常文本、协议错误、选择器未命中等）。
    #[rune(get)]
    pub message: String,
}

/// DOM 节点不透明句柄：`eval_handle` 产出，`call`/`release` 消费；脚本
/// 侧拿不到 objectId 字面量（防拼错与跨组泄漏）。
#[derive(Debug, Clone, Any)]
#[rune(item = ::cdp)]
pub struct ObjHandle {
    /// CDP `RemoteObject.objectId`（只在 Rust 侧使用，脚本不可读）。
    #[rune(get)]
    id: String,
}

impl ObjHandle {
    /// CLONE 协议：句柄按值消费（`call`/`release` 都吃句柄），脚本侧
    /// `clone(h)` 复用同一次 `eval_handle` 产物。
    #[rune::function(protocol = CLONE)]
    fn clone_handle(&self) -> Self {
        Self {
            id: self.id.clone(),
        }
    }
}

/// 隔离世界上下文（每 tab 一个，导航即失效重建）。
#[derive(Default)]
struct WorldCtx {
    /// `Runtime.evaluate` 要的 executionContextId。
    context_id: i64,
    /// addScriptToEvaluateOnNewDocument 是否已随会话注册（幂等闸）。
    bootstrap_registered: bool,
}

/// cdp 模块的会话侧状态：会话句柄加当前隔离世界。
pub(crate) struct CdpState {
    /// 自有 cdp crate 的会话（句柄留 Rust 侧，不进脚本全局）。
    session: Arc<cdp::Session>,
    /// 隔离世界缓存（导航后由 goto 置空重建）。
    world: tokio::sync::Mutex<WorldCtx>,
}

const WORLD_NAME: &str = "browse-rune";
const OBJECT_GROUP: &str = "browse-rune";

fn cdp_err(op: &str, message: impl std::fmt::Display) -> CdpError {
    CdpError {
        op: op.to_string(),
        message: message.to_string(),
    }
}

impl CdpState {
    /// 确保隔离世界在位：`Page.createIsolatedWorld`（每 tab 首次与每次
    /// 导航后），并随会话幂等注册 `Page.addScriptToEvaluateOnNewDocument`
    /// 带同一 `worldName`（新文档世界先行在位，兼作导航前引导钩）。
    async fn ensure_world(&self) -> Result<i64, CdpError> {
        let mut w = self.world.lock().await;
        if w.context_id != 0 {
            return Ok(w.context_id);
        }
        let tree = self
            .session
            .call("Page.getFrameTree", json!({}))
            .await
            .map_err(|e| cdp_err("ensure_world", format!("Page.getFrameTree: {e:#}")))?;
        let frame_id = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .ok_or_else(|| cdp_err("ensure_world", "取主 frame id 失败"))?
            .to_string();
        if !w.bootstrap_registered {
            self.session
                .call(
                    "Page.addScriptToEvaluateOnNewDocument",
                    json!({ "source": "/* browse-rune world bootstrap */", "worldName": WORLD_NAME }),
                )
                .await
                .map_err(|e| {
                    cdp_err("ensure_world", format!("addScriptToEvaluateOnNewDocument: {e:#}"))
                })?;
            w.bootstrap_registered = true;
        }
        let world = self
            .session
            .call(
                "Page.createIsolatedWorld",
                json!({ "frameId": frame_id, "worldName": WORLD_NAME, "grantUniveralAccess": false }),
            )
            .await
            .map_err(|e| cdp_err("ensure_world", format!("Page.createIsolatedWorld: {e:#}")))?;
        let id = world
            .pointer("/executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| cdp_err("ensure_world", "取 executionContextId 失败"))?;
        w.context_id = id;
        Ok(id)
    }

    /// 隔离世界求值（本模块所有碰页面的道都收在这）：`awaitPromise` 加
    /// `returnByValue`；`handle` 形不取值只取 `objectId` 入对象组。
    async fn eval_in_world(&self, expr: &str, handle: bool) -> Result<Value, CdpError> {
        let ctx = self.ensure_world().await?;
        let mut params = json!({
            "expression": expr,
            "contextId": ctx,
            "awaitPromise": true,
        });
        if handle {
            params["returnByValue"] = json!(false);
            params["objectGroup"] = json!(OBJECT_GROUP);
        } else {
            params["returnByValue"] = json!(true);
        }
        let r = self
            .session
            .call("Runtime.evaluate", params)
            .await
            .map_err(|e| cdp_err("eval", format!("Runtime.evaluate: {e:#}")))?;
        if let Some(exc) = r.pointer("/exceptionDetails/text").and_then(Value::as_str) {
            let detail = r
                .pointer("/exceptionDetails/exception/description")
                .and_then(Value::as_str)
                .unwrap_or(exc);
            return Err(cdp_err("eval", detail));
        }
        Ok(r)
    }

    /// CDP `RemoteObject` 的 value 腿过 serde 桥成 rune 值（`() ↔ null`、
    /// `#{ } ↔ object`、数组对数组；value 缺席即 undefined 形单位值）。
    fn bridge(r: &Value) -> Result<RuneValue, CdpError> {
        match r.pointer("/result/value") {
            Some(v) => serde_json::from_value::<RuneValue>(v.clone())
                .map_err(|e| cdp_err("eval", format!("结果过桥失败：{e}"))),
            None => Ok(RuneValue::from(())),
        }
    }
}

/// 注册 `cdp` 模块（方案八函数面）：`goto`/`eval`/`eval_handle`/`call`/
/// `release`/`click`/`type_text`/`wait_sel`，全部返回 `Result`。
///
/// # Errors
///
/// 模块注册失败（函数签名或类型登记冲突）。
pub fn cdp_module(
    session: Arc<cdp::Session>,
    result_slot: Arc<std::sync::Mutex<Option<String>>>,
) -> Result<Module, rune::ContextError> {
    let mut m = Module::with_crate("cdp")?;
    m.ty::<CdpError>()?;
    m.ty::<ObjHandle>()?;
    m.function_meta(ObjHandle::clone_handle)?;
    // 结果回传道（#67 桥形定案）：send_execute 的逃逸包装连 FromValue 都
    // 穿不透，唯脚本->原生参数方向畅通——执行器自动包一层
    // __browse_rune_wrap__，把 main 的 Result 拆开在 VM 内序列化成
    // "ok:<json>"/"err:<json>" 字符串经本函数送回 Rust 侧槽位
    {
        let slot = result_slot;
        m.function("__result", move |s: String| {
            if let Ok(mut g) = slot.lock() {
                *g = Some(s);
            }
        })
        .build()?;
    }
    let state = Arc::new(CdpState {
        session,
        world: tokio::sync::Mutex::new(WorldCtx::default()),
    });

    {
        let st = state.clone();
        m.function("goto", move |url: String| {
            let st = st.clone();
            async move {
                let nav = st
                    .session
                    .call("Page.navigate", json!({ "url": url }))
                    .await
                    .map_err(|e| cdp_err("goto", format!("Page.navigate: {e:#}")))?;
                if let Some(err_text) = nav.get("errorText").and_then(Value::as_str) {
                    return Err(cdp_err("goto", err_text));
                }
                // 导航即废世界：置空待下次 ensure_world 重建
                st.world.lock().await.context_id = 0;
                // 有界等加载：readyState 脱 loading（上限 15s，百毫秒步进；
                // 更细的超时由脚本 select 自包）
                for _ in 0..150 {
                    let r = st
                        .eval_in_world("document.readyState", false)
                        .await
                        .map_err(|e| cdp_err("goto", format!("等加载：{}", e.message)))?;
                    let ready = serde_json::to_value(
                        r.pointer("/result/value").cloned().unwrap_or(Value::Null),
                    )
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string));
                    if ready.as_deref().is_some_and(|rs| rs != "loading") {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Ok(nav
                    .get("frameId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string())
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("eval", move |expr: String| {
            let st = st.clone();
            async move {
                let r = st.eval_in_world(&expr, false).await?;
                CdpState::bridge(&r)
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("eval_handle", move |expr: String| {
            let st = st.clone();
            async move {
                let r = st.eval_in_world(&expr, true).await?;
                let id = r
                    .pointer("/result/objectId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| cdp_err("eval_handle", "结果不是对象（无 objectId）"))?;
                Ok::<ObjHandle, CdpError>(ObjHandle { id: id.to_string() })
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("call", move |handle: ObjHandle, fn_src: String| {
            let st = st.clone();
            async move {
                let r = st
                    .session
                    .call(
                        "Runtime.callFunctionOn",
                        json!({
                            "objectId": handle.id,
                            "functionDeclaration": fn_src,
                            "arguments": [],
                            "awaitPromise": true,
                            "returnByValue": true,
                        }),
                    )
                    .await
                    .map_err(|e| cdp_err("call", format!("callFunctionOn: {e:#}")))?;
                if let Some(exc) = r.pointer("/exceptionDetails/text").and_then(Value::as_str) {
                    return Err(cdp_err("call", exc));
                }
                CdpState::bridge(&r)
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("release", move |handle: ObjHandle| {
            let st = st.clone();
            async move {
                st.session
                    .call("Runtime.releaseObject", json!({ "objectId": handle.id }))
                    .await
                    .map_err(|e| cdp_err("release", format!("releaseObject: {e:#}")))?;
                Ok::<(), CdpError>(())
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("click", move |sel: String| {
            let st = st.clone();
            async move {
                // 选择器经 serde_json 字面量注入（禁止拼进 JS 字符串）；
                // 隔离世界取中心点，Input 域发真实点击
                let sel_lit = serde_json::to_string(&sel).unwrap_or_default();
                let expr = format!(
                    "((el) => el ? (() => {{ const r = el.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; }})() : null)(document.querySelector({sel_lit}))"
                );
                let r = st.eval_in_world(&expr, false).await?;
                let point = r
                    .pointer("/result/value")
                    .cloned()
                    .unwrap_or(Value::Null);
                let [x, y] = serde_json::from_value::<[f64; 2]>(point)
                    .map_err(|_| cdp_err("click", format!("选择器未命中：{sel}")))?;
                for ty in ["mousePressed", "mouseReleased"] {
                    st.session
                        .call(
                            "Input.dispatchMouseEvent",
                            json!({ "type": ty, "x": x, "y": y, "button": "left", "clickCount": 1 }),
                        )
                        .await
                        .map_err(|e| cdp_err("click", format!("Input.{ty}: {e:#}")))?;
                }
                Ok::<(), CdpError>(())
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("type_text", move |sel: String, text: String| {
            let st = st.clone();
            async move {
                let sel_lit = serde_json::to_string(&sel).unwrap_or_default();
                st.eval_in_world(
                    &format!("document.querySelector({sel_lit})?.focus()"),
                    false,
                )
                .await
                .map_err(|e| cdp_err("type_text", format!("focus：{}", e.message)))?;
                st.session
                    .call("Input.insertText", json!({ "text": text }))
                    .await
                    .map_err(|e| cdp_err("type_text", format!("Input.insertText: {e:#}")))?;
                Ok::<(), CdpError>(())
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("wait_sel", move |sel: String, ms: i64| {
            let st = st.clone();
            async move {
                let sel_lit = serde_json::to_string(&sel).unwrap_or_default();
                let expr = format!("!!document.querySelector({sel_lit})");
                let steps = (ms.max(0) / 100) as u32;
                for _ in 0..=steps {
                    let r = st.eval_in_world(&expr, false).await?;
                    if r.pointer("/result/value").and_then(Value::as_bool) == Some(true) {
                        return Ok::<bool, CdpError>(true);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Ok::<bool, CdpError>(false)
            }
        })
        .build()?;
    }
    Ok(m)
}

/// 执行收尾的对象组清账：目标销毁/执行结束整组释放（方案边界条款）。
/// best-effort：会话已断时静默（清账失败不掩盖脚本结果）。
pub(crate) async fn release_object_group(session: &Arc<cdp::Session>) {
    let _ = session
        .call(
            "Runtime.releaseObjectGroup",
            json!({ "objectGroup": OBJECT_GROUP }),
        )
        .await;
}
