//! 自注册 `cdp` 模块（账本 #67）：效果函数全走自有 [`cdp::Session`] 的
//! `call`/`call_with_deadline`，Chrome 只经隔离世界 `Runtime.evaluate` 碰
//! 页面（js 族走主世界全量）。
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
    /// 失败操作名（如 `"goto"`、`"eval"`、`"raw"`）。
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
}

/// cdp 模块的会话侧状态：会话句柄加当前隔离世界。
pub(crate) struct CdpState {
    /// 自有 cdp crate 的会话（句柄留 Rust 侧，不进脚本全局）。
    session: Arc<cdp::Session>,
    /// 隔离世界缓存（导航后由 goto 置空重建）。
    world: tokio::sync::Mutex<WorldCtx>,
    /// 对象组名（每执行随机后缀，评审 #67 一轮 G2：并行执行不互踩
    /// 他组句柄；今有 eval_lock 串行兜底，随机组是纵深）。
    object_group: String,
    /// addScriptToEvaluateOnNewDocument 的注册 identifier（收尾注销用，
    /// 评审 #67 一轮 G1）。独立 std 锁：Drop 兜底道不可 await/blocking_lock。
    script_id: std::sync::Mutex<Option<i64>>,
    /// `js_lib` 的片段库根。
    snippets_root: std::path::PathBuf,
    /// 管道参数道捕获串。
    stdin: String,
    /// 三支柱同源宿主函数束。
    host_fns: HostFns,
    /// 在飞调用面探针（v0.26.1 超时 CTA）。
    op_probe: OpProbe,
    /// 模板定界形态（js_lib 件级指令可覆写；cdp::render 跟随）。
    vars_syntax: crate::vars_syntax::VarsSyntax,
}

const WORLD_NAME: &str = "browse-rune";

/// 单次 CDP 调用上界（#68 主修：丢响应快错不永挂；引擎面操作毫秒级，
/// 30 秒是宽裕界）。全部 `call_with_deadline` 走此值。
const CALL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// 同源宿主面（#68 随批三薄封装）：snapshot/requests/requestDetail 经
/// js_host `global_json` 同一实现注入（闭包形避免 crate 循环依赖）。
pub type HostFn = std::sync::Arc<
    dyn Fn(
            Value,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send>>
        + Send
        + Sync,
>;

/// 三支柱宿主函数注入束（Debug 手写：闭包无派生）。构造请用
/// `..Default::default()` 形——字段加法（如 dialect 腿）不破既有构造点
/// 的编译（评审 #75 轮 G）。
#[derive(Default, Clone)]
pub struct HostFns {
    /// `snapshot(opts)` 同源腿。
    pub snapshot: Option<HostFn>,
    /// `requests(opts)` 同源腿。
    pub requests: Option<HostFn>,
    /// `requestDetail(index_or_id, opts)` 同源腿。
    pub request_detail: Option<HostFn>,
    /// 方言全量桥通用腿（issue #75）：载荷形 `{"name": 函数名, "args":
    /// [位置参]}`，daemon 侧闭包解包转 js_host `global_json` 按名派发——
    /// 一条通道直达方言面全部全局函数（与方言同一实现同一守卫）。
    pub dialect: Option<HostFn>,
}

/// 在飞调用面探针（v0.26.1 超时 CTA；0.30.2 多槽化）：cdp 函数入口置
/// 名、正常退出清空、future 被丢弃（整体超时或中止）时不清空即保留在
/// 飞面——daemon 层的整体超时臂读它报告「卡在哪个调用面」。**多槽**：
/// `future::join` 并发在飞多个 cdp 调用时逐个记账（此前单槽先完成者会
/// 清掉别人的在飞名）。std 锁与 `script_id` 同律（drop 兜底道不可
/// await/blocking_lock）。
pub type OpProbe = std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>;

fn cdp_err(op: &str, message: impl std::fmt::Display) -> CdpError {
    CdpError {
        op: op.to_string(),
        message: message.to_string(),
    }
}

/// js_lib 运行时 vars 的占位前缀扫描（REQ-023 值注入理论形收口）：
/// 递归查任意深度对象/数组里的串值是否含内部占位前缀。何时用：js_lib
/// 渲染前（CLI 道的 `--vars` 已有 vars_value_guard，直 POST /rune 与
/// 脚本内拼串只经此面）。边界：非串标量恒 false。
fn json_contains_placeholder(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::String(s) => s.contains(crate::rawstr::PLACEHOLDER_PREFIX),
        serde_json::Value::Array(a) => a.iter().any(json_contains_placeholder),
        serde_json::Value::Object(o) => o.values().any(json_contains_placeholder),
        _ => false,
    }
}

/// raw 的参数串预检：空串（含纯空白）等价空对象，其余必须是合法 JSON 且
/// 为对象形（CDP 参数恒为对象）。何时用：`cdp::raw` 发命令前——预检先
/// 于连接检查，坏串在进程内即拦（无引擎也报得出）。边界：不校验参数键
/// 与方法名的匹配（那是 Chrome 侧协议校验面）。
///
/// # Errors
///
/// 串非合法 JSON 或不是对象形时，返 op=`raw` 的 [`CdpError`]（带 CTA）。
fn parse_raw_params(params_json: &str) -> Result<Value, CdpError> {
    if params_json.trim().is_empty() {
        return Ok(json!({}));
    }
    let v: Value = serde_json::from_str(params_json).map_err(|e| {
        cdp_err(
            "raw",
            format!(
                "params 不是合法 JSON：{e}；下一步：对照协议文档参数形（对象串，如 {{\"x\": 1}}；空串等价空对象，r# 裸串免转义）"
            ),
        )
    })?;
    if !v.is_object() {
        return Err(cdp_err(
            "raw",
            format!(
                "params 必须是 JSON 对象，得到 {v}；下一步：外层用 {{…}}（CDP 参数恒为对象形）"
            ),
        ));
    }
    Ok(v)
}

/// dialect 的参数串预检（issue #75）：空串（含纯空白）等价空数组，其余
/// 必须是合法 JSON 且为数组形（方言函数的位置参序）。何时用：
/// `cdp::dialect` 发调前——与 [`parse_raw_params`] 同构（raw 要对象、
/// dialect 要数组），坏串进程内即拦。边界：不校验参型与函数名的匹配
/// （那是方言派发面的事，未知函数由 js_host 报「未知函数」带 CTA）。
///
/// # Errors
///
/// 串非合法 JSON 或不是数组形时，返 op=`dialect` 的 [`CdpError`]。
fn parse_dialect_args(args_json: &str) -> Result<Value, CdpError> {
    if args_json.trim().is_empty() {
        return Ok(json!([]));
    }
    let v: Value = serde_json::from_str(args_json).map_err(|e| {
        cdp_err(
            "dialect",
            format!(
                "args 不是合法 JSON：{e}；下一步：位置参按数组串给（如 [\"q\", 10]；空串等价空数组）"
            ),
        )
    })?;
    if !v.is_array() {
        return Err(cdp_err(
            "dialect",
            format!(
                "args 必须是 JSON 数组（位置参序），得到 {v}；下一步：外层用 […]（零参给空串）"
            ),
        ));
    }
    Ok(v)
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
            .call_with_deadline("Page.getFrameTree", json!({}), CALL_DEADLINE)
            .await
            .map_err(|e| cdp_err("ensure_world", format!("Page.getFrameTree: {e:#}")))?;
        let frame_id = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .ok_or_else(|| cdp_err("ensure_world", "取主 frame id 失败"))?
            .to_string();
        if self.script_id.lock().map(|g| g.is_none()).unwrap_or(false) {
            let r = self
                .session
                .call_with_deadline("Page.addScriptToEvaluateOnNewDocument",
                    json!({ "source": "/* browse-rune world bootstrap */", "worldName": WORLD_NAME }),
                CALL_DEADLINE,
                )
                .await
                .map_err(|e| {
                    cdp_err("ensure_world", format!("addScriptToEvaluateOnNewDocument: {e:#}"))
                })?;
            if let Ok(mut g) = self.script_id.lock() {
                *g = r.get("identifier").and_then(Value::as_i64);
            }
        }
        let world = self
            .session
            .call_with_deadline("Page.createIsolatedWorld",
                json!({ "frameId": frame_id, "worldName": WORLD_NAME, "grantUniveralAccess": false }),
            CALL_DEADLINE,
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
            params["objectGroup"] = json!(self.object_group);
        } else {
            params["returnByValue"] = json!(true);
        }
        let r = self
            .session
            .call_with_deadline("Runtime.evaluate", params, CALL_DEADLINE)
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

    /// 主世界全量 JS 求值（`--js` 同语义：页面真 V8、页面函数可调）：
    /// 无 contextId 即主世界；异常文本进 `CdpError`。js/js_lib 的执行腿。
    async fn eval_main_world(&self, wrapped: &str) -> Result<Value, CdpError> {
        let r = self
            .session
            .call_with_deadline(
                "Runtime.evaluate",
                json!({ "expression": wrapped, "returnByValue": true, "awaitPromise": true }),
                CALL_DEADLINE,
            )
            .await
            .map_err(|e| cdp_err("js", format!("Runtime.evaluate: {e:#}")))?;
        if let Some(exc) = r.pointer("/exceptionDetails/text").and_then(Value::as_str) {
            let detail = r
                .pointer("/exceptionDetails/exception/description")
                .and_then(Value::as_str)
                .unwrap_or(exc);
            return Err(cdp_err("js", detail));
        }
        Ok(r)
    }

    /// 片段库解析（js_lib 专用）：`根/<名>` 原样与补 `.js` 两试，越根
    /// 候选跳过；未命中报错列试过的位（与 CLI 侧 load_template_ext 同
    /// 语义，daemon 侧第二实现属库内双轨，ADR-0010 注记）。
    fn resolve_js_lib(&self, name: &str) -> Result<(String, String), CdpError> {
        if name.is_empty() {
            return Err(cdp_err("js_lib", "缺库名"));
        }
        let mut tried: Vec<String> = Vec::new();
        if self.snippets_root.canonicalize().is_ok() {
            for cand in [self.snippets_root.join(name), {
                let mut p = self.snippets_root.join(name);
                p.set_extension("js");
                p
            }] {
                if let (Ok(canon), Ok(root_canon)) =
                    (cand.canonicalize(), self.snippets_root.canonicalize())
                    && canon.starts_with(&root_canon)
                    && let Ok(body) = std::fs::read_to_string(&canon)
                {
                    return Ok((canon.display().to_string(), body));
                }
                tried.push(cand.display().to_string());
            }
        }
        Err(cdp_err(
            "js_lib",
            format!("库内 js 小程序 {name} 未命中（试过：{}）", tried.join("、")),
        ))
    }

    /// 同源宿主腿（#68 三薄封装）：走注入闭包直取方言全局函数的同一
    /// 实现结果；未注入（进程内测试态）报 CdpError 点名。
    async fn call_host(
        &self,
        op: &'static str,
        argv: Vec<RuneValue>,
    ) -> Result<RuneValue, CdpError> {
        let chosen = match op {
            "snapshot" => self.host_fns.snapshot.clone(),
            "requests" => self.host_fns.requests.clone(),
            "requestDetail" => self.host_fns.request_detail.clone(),
            _ => None,
        };
        let f = chosen.ok_or_else(|| {
            cdp_err(
                op,
                "同源宿主面未注入（进程内测试态无 daemon；真用走 browse rune）",
            )
        })?;
        let mut args: Vec<Value> = Vec::with_capacity(argv.len());
        for v in &argv {
            let jv = serde_json::to_value(v)
                .map_err(|e| cdp_err(op, format!("参数不可 JSON 化：{e}（外部类型先转基本值）")))?;
            args.push(jv);
        }
        let out = f(serde_json::to_value(&args).unwrap_or(Value::Null))
            .await
            .map_err(|e| cdp_err(op, e))?;
        serde_json::from_value::<RuneValue>(out)
            .map_err(|e| cdp_err(op, format!("结果过桥失败：{e}")))
    }

    /// 探针置名（cdp 函数入口；被丢弃路径不移除即保留）。
    fn set_op(&self, op: &'static str) {
        if let Ok(mut g) = self.op_probe.lock() {
            g.push(op);
        }
    }

    /// 探针移除一次本名（cdp 函数正常退出，含快错路径；并发下只消自
    /// 己那一份）。
    fn clear_op(&self, op: &'static str) {
        if let Ok(mut g) = self.op_probe.lock()
            && let Some(i) = g.iter().position(|x| *x == op)
        {
            g.remove(i);
        }
    }

    /// 直接结果面（无 RemoteObject 包裹的 CDP 响应）过桥。
    fn bridge_no_result_check(r: &Value, op: &str) -> Result<RuneValue, CdpError> {
        serde_json::from_value::<RuneValue>(r.clone())
            .map_err(|e| cdp_err(op, format!("结果过桥失败：{e}")))
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

/// 模块构建上下文（#67 三形态运行时批）：会话与两条输入通道（snippets
/// 根供 `js_lib`，管道串供 `stdin()`）。
pub struct ModuleCtx {
    /// 自有 cdp crate 的会话（句柄留 Rust 侧，不进脚本全局）。
    pub session: Arc<cdp::Session>,
    /// 结果回传槽（执行器所有）。
    pub result_slot: Arc<std::sync::Mutex<Option<String>>>,
    /// `js_lib` 的片段库根（daemon 侧三序收口后的终值）。
    pub snippets_root: std::path::PathBuf,
    /// 管道参数道捕获串（`cdp::stdin()` 的值；无管道即空串）。
    pub stdin: String,
    /// 三支柱同源宿主函数束（daemon 注入；进程内测试可缺省）。
    pub host_fns: HostFns,
    /// 在飞调用面探针（v0.26.1 超时 CTA；daemon 持克隆读在飞面）。
    pub op_probe: OpProbe,
    /// 模板定界形态（0.30.3 缺省 ERB）：js_lib（件级指令可覆写）与
    /// cdp::render 跟随。
    pub vars_syntax: crate::vars_syntax::VarsSyntax,
}

/// js 体的包裹器（纯函数便于单测）：体原样入、args 经
/// `serde_json::to_string` 做 JSON 字面量注入——单双引号与反引号天然
/// 安全，杜绝手工拼接道（#67 三形态批转义纪律）。
fn js_wrapper(src: &str, args_json: &str) -> String {
    format!("(async (args) => {{\n{src}\n}})({args_json})")
}

/// 注册 `cdp` 模块（通用通道形，ADR-0011）：`goto`/`eval`/`eval_handle`/
/// `call`/`release`/`wait_event`/`raw`/`dialect` 加 `js`/`js_lib`/
/// `render`/`stdin` 与三支柱 `requests`/`snapshot`/`request_detail`，
/// 全部返回 `Result`（`render`/`stdin` 不可失败面直接返值）。
///
/// # Errors
///
/// 模块注册失败（函数签名或类型登记冲突）。
pub fn cdp_module(ctx: ModuleCtx) -> Result<(Module, Cleanup), rune::ContextError> {
    let ModuleCtx {
        session,
        result_slot,
        snippets_root,
        stdin,
        host_fns,
        op_probe,
        vars_syntax,
    } = ctx;
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
        session: session.clone(),
        world: tokio::sync::Mutex::new(WorldCtx::default()),
        object_group: format!("browse-rune-{}", next_group_seq()),
        script_id: std::sync::Mutex::new(None),
        snippets_root,
        stdin,
        host_fns,
        op_probe,
        vars_syntax,
    });

    {
        let st = state.clone();
        m.function("goto", move |url: String| {
            let st = st.clone();
            async move {
                st.set_op("goto");
                let out = async {
                    let nav = st
                        .session
                        .call_with_deadline("Page.navigate", json!({ "url": url }), CALL_DEADLINE)
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
                .await;
                st.clear_op("goto");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("eval", move |expr: String| {
            let st = st.clone();
            async move {
                st.set_op("eval");
                let out = async {
                    let r = st.eval_in_world(&expr, false).await?;
                    CdpState::bridge(&r)
                }
                .await;
                st.clear_op("eval");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("eval_handle", move |expr: String| {
            let st = st.clone();
            async move {
                st.set_op("eval_handle");
                let out = async {
                    let r = st.eval_in_world(&expr, true).await?;
                    let id = r
                        .pointer("/result/objectId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| cdp_err("eval_handle", "结果不是对象（无 objectId）"))?;
                    Ok::<ObjHandle, CdpError>(ObjHandle { id: id.to_string() })
                }
                .await;
                st.clear_op("eval_handle");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("call", move |handle: ObjHandle, fn_src: String| {
            let st = st.clone();
            async move {
                st.set_op("call");
                let out = async {
                    let r = st
                        .session
                        .call_with_deadline(
                            "Runtime.callFunctionOn",
                            json!({
                                "objectId": handle.id,
                                "functionDeclaration": fn_src,
                                "arguments": [],
                                "awaitPromise": true,
                                "returnByValue": true,
                            }),
                            CALL_DEADLINE,
                        )
                        .await
                        .map_err(|e| cdp_err("call", format!("callFunctionOn: {e:#}")))?;
                    if let Some(exc) = r.pointer("/exceptionDetails/text").and_then(Value::as_str) {
                        return Err(cdp_err("call", exc));
                    }
                    CdpState::bridge(&r)
                }
                .await;
                st.clear_op("call");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("release", move |handle: ObjHandle| {
            let st = st.clone();
            async move {
                st.set_op("release");
                let out = async {
                    st.session
                        .call_with_deadline(
                            "Runtime.releaseObject",
                            json!({ "objectId": handle.id }),
                            CALL_DEADLINE,
                        )
                        .await
                        .map_err(|e| cdp_err("release", format!("releaseObject: {e:#}")))?;
                    Ok::<(), CdpError>(())
                }
                .await;
                st.clear_op("release");
                out
            }
        })
        .build()?;
    }
    // ---- 批次二（总台指令）：事件驱动等待原语（CDP 事件面） ----
    {
        let st = state.clone();
        m.function("wait_event", move |method: String, ms: i64| {
            let st = st.clone();
            async move {
                st.set_op("wait_event");
                let out = async {
                    // 事件驱动等待（非轮询）：方言 session.waitFor 同源——活
                    // 动 tab 与 browser 级事件，取出即移除；超时走
                    // CdpError（脚本 ? 传播即失败面，match 即条件分支面）
                    let ev = st
                        .session
                        .wait_for(&method, ms.max(0) as u64)
                        .await
                        .map_err(|e| {
                            cdp_err(
                                "wait_event",
                                format!(
                                    "等 {method} 超时（{ms} 毫秒）或会话异常：{e:#}；下一步：调大毫秒数、先触发事件再等（缓冲含历史），或 match 本错误走替代分支"
                                ),
                            )
                        })?;
                    let params = ev.get("params").cloned().unwrap_or(json!({}));
                    let rv = serde_json::from_value::<RuneValue>(params).map_err(|e| {
                        cdp_err("wait_event", format!("事件参数过桥失败：{e}"))
                    })?;
                    Ok::<RuneValue, CdpError>(rv)
                }
                .await;
                st.clear_op("wait_event");
                out
            }
        })
        .build()?;
    }
    // ---- 通用命令通道（账本 #73，ADR-0011）：任意 CDP 命令直发 ----
    // raw(method, params_json) 把任意 CDP 命令发到当前页面会话（活动 tab
    // 的 session target；Browser.* 前缀走 browser 级端点），params_json 是
    // JSON 对象串（空串等价空对象，非对象形报错——CDP 参数恒为对象形），
    // 响应对象整体过桥回脚本。何时用：函数面没盖到的域与方法——
    // Input.dispatchMouseEvent 序列组拟人鼠标（wiggle/拖拽/滚轮）、
    // Fetch.enable 加 wait_event(Fetch.requestPaused) 循环加
    // failRequest/continueRequest 组按 resourceType 断流放行、
    // Network.setCookie/getAllCookies 组 cookie 面。边界：与内置函数同一
    // 道 `Session::call_with_deadline` 守卫与 30 秒上界（域策略对
    // Page.navigate/Target.createTarget 照拦）；Page.navigate/Page.reload
    // 成功后与 goto 同款废隔离世界待重建，Target.createTarget 不登记 own
    // （后续 Target.closeTarget 被守卫拦，新页开档走 goto）；开
    // Fetch.enable 后 requestPaused 由脚本自己应答（本函数不代答）。
    // params 取 JSON 串形而
    // 非 rune 对象：对象形实测可行（嵌套组合与 null 经 `()` 都过桥，
    // vm_face 探针在册），选串形是协议文档参数形原样粘贴（r# 免转义，
    // 与 wait_event 的任意 method 名对偶）且避开外部类型的序列化错面
    // （js 的「args 不可 JSON 化」先例）。
    //
    // click/type_text/wait_sel/cookies/set_cookie 五件纯透传糖皮已回收
    // （ADR-0011）：raw 加 eval/js 组合全覆盖，组合形见 e2e 两组件验证。
    {
        let st = state.clone();
        m.function("raw", move |method: String, params_json: String| {
            let st = st.clone();
            async move {
                st.set_op("raw");
                let out = async {
                    let params = parse_raw_params(&params_json)?;
                    let r = st
                        .session
                        .call_with_deadline(&method, params, CALL_DEADLINE)
                        .await
                        .map_err(|e| cdp_err("raw", format!("{method}: {e:#}")))?;
                    // 导航类经 raw 触发即废世界（评审 G-6）：与 goto 同款
                    // 置空待重建，否则后续 eval 撞死 context
                    if matches!(method.as_str(), "Page.navigate" | "Page.reload") {
                        st.world.lock().await.context_id = 0;
                    }
                    CdpState::bridge_no_result_check(&r, "raw")
                }
                .await;
                st.clear_op("raw");
                out
            }
        })
        .build()?;
    }
    // ---- 方言全量桥（issue #75，ADR-0011 增补）：方言层通用通道 ----
    // dialect(fn, args_json) 把方言全局函数按名调到方言面同一实现
    // （js_host global_json 派发：currentTab/waitLoad/waitForResponse/
    // clickRef/routeBlock 等 100+ 件，含三支柱）。args_json 是 JSON 数组
    // 串（位置参序，空串等价空数组，非数组形预检拦），返回值整体过桥。
    // 何时用：方言面的有态聚合件（waitForResponse 的界内等体、waitIdle
    // 的窗口语义）与既有战役片段逻辑复用——rn 侧不再手组等价物。边界：
    // 与 raw 对偶（raw=CDP 层通道，dialect=方言层通道），同为单通道替代
    // 逐件绑定；薄封装件经桥可达不等于 rn 面内建（调用形自带 dialect
    // 前缀）；守卫在 Session 层照拦（域策略对导航类同拦）；未注入（进程
    // 内测试态）报 CdpError 点名；未知函数由方言派发面报「未知函数」CTA。
    {
        let st = state.clone();
        m.function("dialect", move |fn_name: String, args_json: String| {
            let st = st.clone();
            async move {
                st.set_op("dialect");
                let out = async {
                    let args = parse_dialect_args(&args_json)?;
                    let f = st.host_fns.dialect.clone().ok_or_else(|| {
                        cdp_err(
                            "dialect",
                            "同源宿主面未注入（进程内测试态无 daemon；真用走 browse rune）",
                        )
                    })?;
                    let payload = json!({ "name": fn_name, "args": args });
                    let out = f(payload)
                        .await
                        .map_err(|e| cdp_err("dialect", format!("{fn_name}: {e}")))?;
                    serde_json::from_value::<RuneValue>(out)
                        .map_err(|e| cdp_err("dialect", format!("结果过桥失败：{e}")))
                }
                .await;
                st.clear_op("dialect");
                out
            }
        })
        .build()?;
    }
    // 错误载荷降形（vm_face 实弹抓的坑）：json 模块与 Debug 协议对外部
    // 类型都出「object at 0x…」占位，CdpError 的 op/message 会丢——宿主
    // 侧专降 JSON 形，wrapper 的 Err 臂消费（其余错误值照旧 json/Debug）
    m.function("__describe", |v: RuneValue| -> Option<String> {
        let cloned = v.clone();
        match <CdpError as rune::FromValue>::from_value(cloned) {
            Ok(e) => {
                let op = serde_json::to_string(&e.op).unwrap_or_else(|_| "\"?\"".into());
                let msg = serde_json::to_string(&e.message).unwrap_or_else(|_| "\"?\"".into());
                Some(format!("{{\"op\":{op},\"message\":{msg}}}"))
            }
            Err(_) => match serde_json::to_string(&v) {
                Ok(s2) if !s2.contains("object at") => Some(s2),
                _ => None,
            },
        }
    })
    .build()?;

    // ---- 三形态批：js 集成（内联 js / 库内 js / 运行时 jinja / 管道参数） ----
    {
        let st = state.clone();
        m.function("js", move |src: String, args: RuneValue| {
            let st = st.clone();
            async move {
                st.set_op("js");
                let out = async {
                    // args 对象经 serde 成 JSON 字面量注入（单双引号反引号
                    // 天然安全；体原样入包裹器零拼接）
                    // 评审双批 G2：args 不可 JSON 化显式报错（外部类型静默降
                    // null 会让 JS 侧报类型错，误导根因）
                    let args_val = match serde_json::to_value(&args) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(cdp_err(
                                "js",
                                format!("args 不可 JSON 化：{e}（外部类型先在脚本内转基本值）"),
                            ));
                        }
                    };
                    let args_json =
                        serde_json::to_string(&args_val).unwrap_or_else(|_| "null".to_string());
                    let r = st.eval_main_world(&js_wrapper(&src, &args_json)).await?;
                    CdpState::bridge(&r)
                }
                .await;
                st.clear_op("js");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("js_lib", move |name: String, vars: RuneValue| {
            let st = st.clone();
            async move {
                st.set_op("js_lib");
                let out = async {
                    let (_path, body) = match st.resolve_js_lib(&name) {
                        Ok(v) => v,
                        Err(e) => return Err(e),
                    };
                    // #66 全同管线：vars 对象做 Jinja 渲染（Strict；未填键报错
                    // 带 op=js_lib），渲染产物进主世界执行（args 空对象占位）
                    let vars_json = match serde_json::to_value(&vars) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(cdp_err(
                                "js_lib",
                                format!("vars 不可 JSON 化：{e}（外部类型先在脚本内转基本值）"),
                            ));
                        }
                    };
                    // 值注入理论形守卫（REQ-023）：运行时 vars 绕过 CLI 的
                    // vars_value_guard（直 POST /rune 或脚本内拼串都到这），
                    // 值含内部占位前缀会在回填段被当作占位记号（拼接注释
                    // 区文本）或触发误导性残留报错；渲染前拦（递归含嵌套
                    // 对象/数组里的串值）
                    if json_contains_placeholder(&vars_json) {
                        return Err(cdp_err(
                            "js_lib",
                            format!(
                                "vars 值含内部占位前缀 {}（值注入面）；下一步：值避开该前缀",
                                crate::rawstr::PLACEHOLDER_PREFIX
                            ),
                        ));
                    }
                    // G-1 对齐批：库体是 JS 族，注释区先抽占位惰性化再渲染；
                    // 定界=件首行指令覆写本执行选择（0.30.3 缺省 ERB）
                    let lib_syntax =
                        crate::vars_syntax::directive_of(&body).unwrap_or(st.vars_syntax);
                    let (placed, regions) =
                        crate::rawstr::extract_comment_regions(&body).map_err(|e| {
                            cdp_err("js_lib", format!("注释惰性化预处理失败：{}", e.message))
                        })?;
                    let pre = match crate::vars_syntax::env_for(lib_syntax)
                        .render_str(&placed, vars_json)
                    {
                        Ok(t) => t,
                        Err(e) => {
                            return Err(cdp_err(
                                "js_lib",
                                format!("模板渲染失败：{e}（未填键补 vars 或模板给 default）"),
                            ));
                        }
                    };
                    let rendered =
                        crate::rawstr::subst_raw_strings(&pre, &regions).map_err(|e| {
                            cdp_err("js_lib", format!("注释惰性化预处理失败：{}", e.message))
                        })?;
                    let r = st.eval_main_world(&js_wrapper(&rendered, "{}")).await?;
                    CdpState::bridge(&r)
                }
                .await;
                st.clear_op("js_lib");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("render", move |tpl: String, vars: RuneValue| {
            // 定界随本执行（0.30.3 缺省 ERB；CLI 旗标/件指令收口递入）
            let vars_json = match serde_json::to_value(&vars) {
                Ok(v) => v,
                Err(e) => {
                    return Err(cdp_err(
                        "render",
                        format!("vars 不可 JSON 化：{e}（外部类型先在脚本内转基本值）"),
                    ));
                }
            };
            match crate::vars_syntax::env_for(st.vars_syntax).render_str(&tpl, vars_json) {
                Ok(out) => Ok(out),
                Err(e) => Err(cdp_err(
                    "render",
                    format!("模板渲染失败：{e}（未填键补 vars 或模板给 default）"),
                )),
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("stdin", move || st.stdin.clone()).build()?;
    }

    // ---- #68 修复批随行三薄封装：三支柱同源（cookie 面已回收，ADR-0011）----
    {
        let st = state.clone();
        m.function("requests", move |opts: RuneValue| {
            let st = st.clone();
            async move {
                st.set_op("requests");
                let out = st.call_host("requests", vec![opts]).await;
                st.clear_op("requests");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("snapshot", move |opts: RuneValue| {
            let st = st.clone();
            async move {
                st.set_op("snapshot");
                let out = st.call_host("snapshot", vec![opts]).await;
                st.clear_op("snapshot");
                out
            }
        })
        .build()?;
    }
    {
        let st = state.clone();
        m.function("request_detail", move |sel: RuneValue, opts: RuneValue| {
            let st = st.clone();
            async move {
                st.set_op("request_detail");
                let out = st.call_host("requestDetail", vec![sel, opts]).await;
                st.clear_op("request_detail");
                out
            }
        })
        .build()?;
    }

    Ok((
        m,
        Cleanup {
            session,
            state,
            settled: std::sync::atomic::AtomicBool::new(false),
        },
    ))
}

/// 对象组序号（每执行独立组名用）。
fn next_group_seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// 执行收尾（方案边界条款 + 评审 #67 一轮 G1/G2）：整组释放本执行
/// 对象组，注销世界引导脚本注册。best-effort：会话已断时静默（清账
/// 失败不掩盖脚本结果）。
pub struct Cleanup {
    session: Arc<cdp::Session>,
    state: Arc<CdpState>,
    /// 已清账闸（显式 `run()` 先置位；Drop 兜底见此即让）。
    settled: std::sync::atomic::AtomicBool,
}

impl Cleanup {
    /// 显式清账腿：`Runtime.releaseObjectGroup`（本执行随机组）与
    /// `Page.removeScriptToEvaluateOnNewDocument`（本次注册的引导）。
    /// 先置 settled 再干活：清账 future 自身被丢弃时不双跑。
    pub async fn run(&self) {
        self.settled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.cleanup_inner().await;
    }

    async fn cleanup_inner(&self) {
        let _ = self
            .session
            .call_with_deadline(
                "Runtime.releaseObjectGroup",
                json!({ "objectGroup": self.state.object_group }),
                CALL_DEADLINE,
            )
            .await;
        // 先取出 id 即弃守卫：std 锁守卫不可跨 await（跨即 future !Send，
        // axum Handler 界破）
        let script_id = self.state.script_id.lock().ok().and_then(|mut g| g.take());
        if let Some(id) = script_id {
            let _ = self
                .session
                .call_with_deadline(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    json!({ "identifier": id }),
                    CALL_DEADLINE,
                )
                .await;
        }
    }
}

impl Drop for Cleanup {
    /// 超时丢弃兜底（评审 #67 一轮 G2 收口）：daemon 超时 drop 执行
    /// future 时清账仍要走——借当前 tokio 运行时排程清账任务；无运行时
    /// 上下文（同步测试态）或已显式清账时静默让位。
    fn drop(&mut self) {
        if self.settled.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        self.settled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let session = self.session.clone();
        let group = self.state.object_group.clone();
        let script_id = self.state.script_id.lock().ok().and_then(|mut g| g.take());
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // #67 G7 收口：drop 兜底清账整体 10 秒上界（两调用各自 30 秒
            // deadline 的最坏 60 秒收进一条，防极端下清账任务滞留）
            handle.spawn(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    let _ = session
                        .call_with_deadline(
                            "Runtime.releaseObjectGroup",
                            json!({ "objectGroup": group }),
                            CALL_DEADLINE,
                        )
                        .await;
                    if let Some(id) = script_id {
                        let _ = session
                            .call_with_deadline(
                                "Page.removeScriptToEvaluateOnNewDocument",
                                json!({ "identifier": id }),
                                CALL_DEADLINE,
                            )
                            .await;
                    }
                })
                .await;
            });
        }
    }
}

#[cfg(test)]
mod raw_params_tests {
    use super::parse_raw_params;
    use serde_json::json;

    /// 空串（含纯空白）等价空对象——无参命令（Network.enable 等）的写法。
    #[test]
    fn empty_string_is_empty_object() {
        assert_eq!(parse_raw_params("").unwrap(), json!({}));
        assert_eq!(parse_raw_params("   ").unwrap(), json!({}));
    }

    /// 对象串原样过：嵌套数组与布尔保真（Input.dispatchMouseEvent 的
    /// modifiers 位掩码、Fetch.enable 的 patterns 数组同类形）。
    #[test]
    fn object_string_roundtrips() {
        let v = parse_raw_params(r#"{"x": 40, "mods": [1, 2], "o": {"k": true}}"#)
            .expect("合法对象串应过");
        assert_eq!(v, json!({"x": 40, "mods": [1, 2], "o": {"k": true}}));
    }

    /// 坏 JSON 与非对象形（数组、字符串、数字、布尔、null）都拦，op 恒 raw。
    #[test]
    fn non_object_and_bad_json_error() {
        for bad in [r#"[1, 2]"#, "42", "\"x\"", "true", "null", "{oops"] {
            let e = parse_raw_params(bad).unwrap_err();
            assert_eq!(e.op, "raw", "{bad}");
            assert!(!e.message.is_empty(), "{bad}");
        }
    }
}

#[cfg(test)]
mod dialect_args_tests {
    use super::parse_dialect_args;
    use serde_json::json;

    /// 空串（含纯空白）等价空数组——零参方言函数（currentTab 等）的写法。
    #[test]
    fn empty_string_is_empty_array() {
        assert_eq!(parse_dialect_args("").unwrap(), json!([]));
        assert_eq!(parse_dialect_args("   ").unwrap(), json!([]));
    }

    /// 数组串原样过：位置参序（字符串、数字、对象混排）。
    #[test]
    fn array_string_roundtrips() {
        let v = parse_dialect_args(r#"["portswigger", 10, {"depth": 2}]"#).expect("合法数组串应过");
        assert_eq!(v, json!(["portswigger", 10, {"depth": 2}]));
    }

    /// 坏 JSON 与非数组形（对象、字符串、数字、null）都拦，op 恒 dialect。
    #[test]
    fn non_array_and_bad_json_error() {
        for bad in [r#"{"filter": "x"}"#, "\"q\"", "42", "null", "[oops"] {
            let e = parse_dialect_args(bad).unwrap_err();
            assert_eq!(e.op, "dialect", "{bad}");
            assert!(!e.message.is_empty(), "{bad}");
        }
    }
}

#[cfg(test)]
mod wrapper_tests {
    use super::js_wrapper;

    /// #67 G6 收口：包裹器形状——体原样入、args JSON 字面量注入参数位。
    #[test]
    fn js_wrapper_shape() {
        let w = js_wrapper("return args.k;", "{\"k\": 1}");
        assert_eq!(w, "(async (args) => {\nreturn args.k;\n})({\"k\": 1})");
    }

    /// #67 G6 收口：体含反引号与 ${ 原样（零转义零拼接）。
    #[test]
    fn js_wrapper_verbatim_body() {
        let w = js_wrapper("const t = `${{x:1}}`;", "{}");
        assert!(w.contains("`${{x:1}}`;"), "{w}");
        assert!(w.starts_with("(async (args) => {"), "{w}");
        assert!(w.ends_with("})({})"), "{w}");
    }
}

#[cfg(test)]
mod vars_guard_tests {
    use super::json_contains_placeholder;
    use serde_json::json;

    /// REQ-023：顶层串值含前缀命中（js_lib 运行时道收口的主形）。
    #[test]
    fn top_level_string_hits() {
        assert!(json_contains_placeholder(&json!({
            "code": "x __BROWSE_RN_RAW_0__ y"
        })));
    }

    /// REQ-023：嵌套对象/数组深处的串值也命中（运行时 vars 是任意 JSON）。
    #[test]
    fn nested_string_hits() {
        assert!(json_contains_placeholder(&json!({
            "rows": [{"v": "a"}, {"v": "__BROWSE_RN_RAW_1__"}],
            "meta": {"ok": true}
        })));
    }

    /// REQ-023：正常值（含非串标量与无前缀串）零误伤。
    #[test]
    fn clean_values_pass() {
        assert!(!json_contains_placeholder(&json!({
            "url": "https://example.com/a__b",
            "n": 7,
            "f": 0.5,
            "b": false,
            "nil": null,
            "arr": ["plain", 3]
        })));
    }
}
