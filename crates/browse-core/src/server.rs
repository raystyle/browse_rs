//! daemon 的 HTTP API：常驻会话 + 方言求值 + 引擎生命周期。
//!
//! 移植自 browser-harness-rs `src/server.rs`（POST /eval、GET /health、
//! POST /quit 三端点保留），新增 `/engine/up` 与求值前的懒引擎策略：
//!
//! - 求值前未连接且片段里没有 `session.connect` -> 自动 [`crate::Engine::ensure`]；
//!   片段自带 `session.connect` 则不预连（显式意图优先，ADR-0004）。
//! - 求值撞上「Not connected」再兜底 ensure + 重试一次（覆盖先语句后连接的写法）。
//! - 单飞槽：同一时刻只跑一条片段，后来者排队（不拒 429）。
//! - `/eval` 超时默认 300 秒（`BROWSE_EVAL_TIMEOUT`，单位秒）。

use crate::{Engine, EngineSpec, JsHost};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

/// 描述 POST /eval 的请求体。
#[derive(Debug, Deserialize)]
pub struct EvalRequest {
    /// 方言片段源码；`js` 为真时是全量 JS（不经方言解析器，#22 旁路）。
    pub code: String,
    /// 求值前先新开 about:blank tab（`browse --new-tab` 面）。
    #[serde(default)]
    pub new_tab: bool,
    /// 全量 JS 形态（`browse --js` 面，#22）：走 [`JsHost::eval_js`]。
    #[serde(default)]
    pub js: bool,
}

/// 描述 POST /rune 的请求体（账本 #67：Rune 编排面）。
#[derive(Debug, Default, Deserialize)]
pub struct RuneRequest {
    /// Rune 编排脚本源码（入口恒 `pub async fn main()`；宿主只装
    /// json/time/cdp 三模块，双限额与错误约定见 docs/adr/ADR-0009 与
    /// crates/browse-rune）。
    pub code: String,
    /// snippets 根覆写（三形态批：`js_lib` 解析位；旗标 > env > 内置，
    /// CLI 侧收口后递入）。
    #[serde(default)]
    pub root: Option<String>,
    /// 管道参数道捕获串（`cdp::stdin()` 的值；无管道不传）。
    #[serde(default)]
    pub stdin: Option<String>,
}

/// 描述 POST /engine/up 的请求体。
#[derive(Debug, Default, Deserialize)]
pub struct EngineUpRequest {
    /// spawn 时无头。
    #[serde(default)]
    pub headless: bool,
    /// 指定 chrome 可执行文件。
    pub chrome: Option<String>,
    /// 显式 WS URL。
    pub ws: Option<String>,
    /// 显式端口。
    pub port: Option<u16>,
    /// spawn 走 CDP 管道通道（S005，Windows）。
    #[serde(default)]
    pub pipe: bool,
    /// spawn 引擎的自定义 profile（user-data-dir；缺省用固定 engine-profile）。
    pub profile: Option<String>,
    /// 引擎代理（`--proxy-server`，#25.5）。
    pub proxy: Option<String>,
    /// 代理旁路清单（`--proxy-bypass-list`，#25.5）。
    pub proxy_bypass: Option<String>,
    /// 隔离态 profile：引擎退出即删（#25.3 拆出）。
    #[serde(default)]
    pub isolated: bool,
    /// 引擎附加旗标（#48）：spawn 时 argv 元素原样直通。
    #[serde(default)]
    pub engine_args: Vec<String>,
}

/// daemon 自描述（#59）：宿主与进程身份一次采值，status 与 /health 内嵌
/// （`daemon` 键），CLI 比对自身平台做跨宿主标注（Windows CLI 经
/// localhost 转发命中 WSL daemon 的场景）。
#[derive(Debug, Clone)]
pub struct DaemonDesc {
    /// 宿主 OS（`std::env::consts::OS`：windows/macos/linux）。
    pub os: &'static str,
    /// 宿主主机名（`hostname` 命令采值，取不到 `unknown`）。
    pub hostname: String,
    /// daemon 进程 pid。
    pub pid: u32,
    /// 实例名（BROWSE_NAME 派生，缺省 default）。
    pub name: String,
    /// daemon 启动时刻（Unix 毫秒）。
    pub started_at: u64,
}

impl DaemonDesc {
    /// 现场采值：hostname 走 `hostname` 命令（仓内 git shell-out 先例，
    /// 零新依赖），失败降级 `unknown` 不阻断 daemon 起。
    pub fn capture() -> Self {
        let hostname = std::process::Command::new("hostname")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "unknown".into());
        Self {
            os: std::env::consts::OS,
            hostname,
            pid: std::process::id(),
            name: crate::paths::instance_name().unwrap_or_else(|| "default".into()),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    /// `daemon` 键的序列化形（status 与 /health 共用）。
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "os": self.os,
            "hostname": self.hostname,
            "pid": self.pid,
            "name": self.name,
            "startedAt": self.started_at,
        })
    }
}

/// HTTP daemon 的运行面聚合：宿主、引擎、缺省引擎意图、单飞槽与退出旗标。
pub struct Daemon {
    /// 方言宿主（vars 跨请求持久）。
    pub host: Arc<JsHost>,
    /// 引擎状态机。
    pub engine: Arc<Engine>,
    /// 没有显式意图时的引擎策略（来自 CLI 旗标 / 环境；#61 后成功
    /// 的显式 spawn up 会回写它，懒重 spawn 沿用最近形态）。锁形因
    /// Daemon 在 Arc 后共享。
    pub spec: std::sync::Mutex<EngineSpec>,
    /// 单飞槽：同一时刻一条片段。
    pub eval_lock: tokio::sync::Mutex<()>,
    /// 退出旗标（POST /quit 置位）。
    pub quit: Arc<AtomicBool>,
    /// daemon 启动时刻。
    pub started: Instant,
    /// 自描述（#59）：status 与 /health 的 daemon 键。
    pub desc: DaemonDesc,
    /// #60 上次操作的引擎指纹（变化检测在 daemon 侧，CLI 无状态）：
    /// eval 信封的 changes 由与它的比对产生。
    pub last_fp: std::sync::Mutex<Option<serde_json::Value>>,
    /// 最近活动时刻（#25.1 idle-timeout 判据）：eval/up 请求刷新。
    pub last_activity: std::sync::Mutex<Instant>,
    /// rune 执行的主动停止道（批次二）：执行腿置发送端，`POST /rune/stop`
    /// 取出发送端置真丢弃执行 future（与整体超时同构，清账走 Drop 兜底）。
    pub rune_cancel: std::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    /// rune 是否在跑（stop 回执的 running 判据）。
    pub rune_running: Arc<AtomicBool>,
}

impl Daemon {
    /// 把宿主、引擎与缺省意图组装成运行面。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # // no_run：JsHost 构造会 spawn watcher 任务，需要 tokio runtime
    /// let session = cdp::Session::new();
    /// let d = browse_core::server::Daemon::new(
    ///     browse_core::JsHost::new(session.clone()),
    ///     browse_core::Engine::new(session),
    ///     browse_core::EngineSpec::from_env(None, false, false),
    /// );
    /// assert!(std::sync::Arc::strong_count(&d) >= 1);
    /// ```
    pub fn new(host: Arc<JsHost>, engine: Arc<Engine>, spec: EngineSpec) -> Arc<Self> {
        Arc::new(Self {
            host,
            engine,
            eval_lock: tokio::sync::Mutex::new(()),
            quit: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
            desc: DaemonDesc::capture(),
            spec: std::sync::Mutex::new(spec),
            last_fp: std::sync::Mutex::new(None),
            last_activity: std::sync::Mutex::new(Instant::now()),
            rune_cancel: std::sync::Mutex::new(None),
            rune_running: Arc::new(AtomicBool::new(false)),
        })
    }
}

/// 引擎闲置回收窗（#25.1）：`BROWSE_IDLE_TIMEOUT` 毫秒，缺省 1 小时，
/// `0` 关闭。到期引擎退役（daemon 不退），下次求值懒拉起。
fn idle_timeout() -> Duration {
    std::env::var("BROWSE_IDLE_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(3_600_000))
}

fn eval_timeout() -> Duration {
    std::env::var("BROWSE_EVAL_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

/// rune 面整体执行上界（ADR-0010 增补终裁 2026-10-02，批次二落地）：
/// 独立旋钮 `BROWSE_RUNE_TIMEOUT`（秒，默认 45，战役台诉求口径）；
/// `/eval` 的 `BROWSE_EVAL_TIMEOUT` 300 秒契约零动（长跑方言片段正当
/// 面）。daemon 级上界：实例内全部 rune 执行同界，长爬（逐页 goto 累计
/// 分钟级）触顶按需调大或拆专用命名实例。
fn rune_timeout() -> Duration {
    std::env::var("BROWSE_RUNE_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(45))
}

/// 起 HTTP daemon，监听 `bind`，直到 POST /quit。
///
/// # Errors
///
/// 端口绑定失败或 axum serve 出错。
pub async fn serve(daemon: Arc<Daemon>, bind: &str) -> anyhow::Result<()> {
    let state = AppState {
        daemon: daemon.clone(),
    };
    let app = Router::new()
        .route("/eval", post(eval_handler))
        .route("/rune", post(rune_handler))
        .route("/rune/stop", post(rune_stop_handler))
        .route("/health", get(health_handler))
        .route("/engine/up", post(engine_up_handler))
        .route("/quit", post(quit_handler))
        .with_state(state);

    // 闲置回收机（#25.1）：到期退引擎不退 daemon，下次求值自动拉起
    {
        let daemon = daemon.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let timeout = idle_timeout();
                if timeout.is_zero() {
                    continue;
                }
                let idle_for = daemon
                    .last_activity
                    .lock()
                    .map(|t| t.elapsed())
                    .unwrap_or(Duration::ZERO);
                if idle_for < timeout || !daemon.host.session().is_connected() {
                    continue;
                }
                // 与求值共用单飞槽（评审 F1）：不撕断在跑的求值，也防
                // 「引擎退役 -> Not connected 兜底 -> 整段重放」的副作用重放；
                // 拿到槽后二次确认（等槽期间可能刚来了求值）
                let _slot = daemon.eval_lock.lock().await;
                let still_idle = daemon
                    .last_activity
                    .lock()
                    .map(|t| t.elapsed())
                    .unwrap_or(Duration::ZERO);
                if still_idle < timeout || !daemon.host.session().is_connected() {
                    continue;
                }
                eprintln!(
                    "{}",
                    json!({
                        "idleRetire": true,
                        "idleMs": still_idle.as_millis() as u64,
                        "note": "引擎闲置退役，下次求值自动拉起（BROWSE_IDLE_TIMEOUT 可调，0 关闭）",
                    })
                );
                let _ = daemon.engine.shutdown().await;
            }
        });
    }

    let listener = TcpListener::bind(bind).await?;
    eprintln!(
        "{}",
        json!({
            "ok": true,
            "ready": true,
            "bind": bind,
            "message": format!("browse daemon listening on http://{bind}"),
        })
    );
    let quit = daemon.quit.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !quit.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await?;
    daemon.engine.shutdown().await?;
    Ok(())
}

#[derive(Clone)]
struct AppState {
    daemon: Arc<Daemon>,
}

/// /eval 的语言分流（#22）：`js` 旗标走全量 JS 旁路（[`JsHost::eval_js`]，
/// 不经方言解析器），缺省走方言片段。
async fn run_eval_code(host: &JsHost, code: &str, js: bool) -> anyhow::Result<Value> {
    if js {
        host.eval_js(code).await
    } else {
        host.eval_snippet(code).await
    }
}

/// 三支柱同源宿主函数束（#68 修复批）：闭包桥 js_host `global_json`，
/// rune 面与方言面同一实现。
fn host_fns(host: &std::sync::Arc<JsHost>) -> browse_rune::HostFns {
    let wrap = |name: &'static str| -> browse_rune::module::HostFn {
        let h = std::sync::Arc::clone(host);
        std::sync::Arc::new(move |args: Value| {
            let h = std::sync::Arc::clone(&h);
            Box::pin(async move {
                let argv: Vec<Value> = args.as_array().cloned().unwrap_or_default();
                // #68:js_host 腿自身无逐调用 deadline,闭包层整体 60s 上界
                // (同源三支柱的最坏复合调用面也远低于此)
                tokio::time::timeout(Duration::from_secs(60), h.global_json(name, &argv))
                    .await
                    .map_err(|_| {
                        format!("{name} 同源调用超时（60 秒上界）；下一步:browse status 看引擎态")
                    })?
                    .map_err(|e| format!("{e:#}"))
            })
                as std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<Value, String>> + Send>,
                >
        })
    };
    browse_rune::HostFns {
        snapshot: Some(wrap("snapshot")),
        requests: Some(wrap("requests")),
        request_detail: Some(wrap("requestDetail")),
    }
}

/// POST /rune（账本 #67）：Rune 编排脚本执行。与 /eval 同槽串行、同
/// 引擎懒起与超时口径；回执 envelope 同形（value 是 rune 回执 JSON：
/// `{"ok": …}` 或 `{"error": …}`，宿主终态走 err 面）。
async fn rune_handler(
    State(st): State<AppState>,
    Json(req): Json<RuneRequest>,
) -> impl IntoResponse {
    let _ = st
        .daemon
        .last_activity
        .lock()
        .map(|mut t| *t = Instant::now());
    // 锁获取有界（#68 连坐修复）：执行串行不变，排队上限 10 秒，
    // 过界快错不无界等
    let _slot = match tokio::time::timeout(Duration::from_secs(10), st.daemon.eval_lock.lock())
        .await
    {
        Ok(g) => g,
        Err(_) => {
            return err_response(anyhow::anyhow!(
                "引擎忙或正在退役（执行槽被占/闲置回收进行中，等 10 秒未获得；重试即通）；下一步：稍后重试，或 browse status 看引擎态"
            ));
        }
    };
    let d = &st.daemon;
    // Rune 编排恒碰页面：懒引擎预连（方言的「片段自己连」分支不适用）
    if !d.host.session().is_connected() {
        let spec = d.spec.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Err(e) = d.engine.ensure(&spec).await {
            return err_response(e);
        }
    }
    let root = std::path::PathBuf::from(
        req.root
            .clone()
            .unwrap_or_else(|| crate::paths::snippets_root().to_string_lossy().into_owned()),
    );
    let opts = browse_rune::RunOpts {
        snippets_root: root,
        stdin: req.stdin.clone().unwrap_or_default(),
        host_fns: host_fns(&d.host),
        // v0.26.1 超时 CTA：探针克隆自持，超时丢弃执行 future 后读在飞面
        op_probe: browse_rune::OpProbe::default(),
    };
    let op_probe = opts.op_probe.clone();
    // 主动停止道（批次二总台指令）：本执行的取消通道入槽，select 上与
    // 整体超时并列；停止即丢弃执行 future（清账走 Drop 兜底，与超时同构）
    let started = Instant::now();
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    *d.rune_cancel.lock().unwrap_or_else(|e| e.into_inner()) = Some(cancel_tx);
    d.rune_running.store(true, Ordering::Relaxed);
    let watch_session = d.host.session();
    enum Outcome {
        Done(anyhow::Result<Value>),
        Timeout,
        Stopped,
        EngineLost,
    }
    let outcome = tokio::select! {
        r = tokio::time::timeout(
            rune_timeout(),
            browse_rune::run(d.host.session(), &req.code, browse_rune::DEFAULT_CAPS, opts),
        ) => match r {
            Ok(v) => Outcome::Done(v),
            Err(_) => Outcome::Timeout,
        },
        _ = cancel_rx.changed() => Outcome::Stopped,
        _ = engine_lost_watch(&watch_session) => Outcome::EngineLost,
    };
    d.rune_running.store(false, Ordering::Relaxed);
    *d.rune_cancel.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let value = match outcome {
        Outcome::Done(Ok(v)) => v,
        Outcome::Done(Err(e)) => return err_response(e),
        Outcome::Timeout => {
            // 整体超时（v0.26.1 最小 CTA，批次二换 rune 专属旋钮）：结构化
            // timeout 面（op 在飞调用面、elapsed_ms 已等待毫秒、bound_s 上
            // 界秒）加 daemon 日志行；错误文案保持 CTA 形
            let bound = rune_timeout();
            let op = op_probe.lock().ok().and_then(|g| *g);
            eprintln!(
                "{}",
                json!({ "runeTimeout": {
                    "op": op,
                    "elapsedMs": bound.as_millis() as u64,
                    "boundS": bound.as_secs(),
                }})
            );
            return rune_timeout_response(op, bound);
        }
        Outcome::Stopped | Outcome::EngineLost => {
            // 主动停止（批次二）：结构化 stopped 面与超时同构（op 在飞调用
            // 面、elapsed_ms 实测毫秒、cause 二分流：stop_request 请求停 /
            // engine_lost 失联联动停）加日志行；脚本侧自停走原生控制流
            //（select/分支加 Err 返回，error 面自带 CTA，不自封装原语）
            let cause = if matches!(outcome, Outcome::EngineLost) {
                "engine_lost"
            } else {
                "stop_request"
            };
            let elapsed = started.elapsed().as_millis() as u64;
            let op = op_probe.lock().ok().and_then(|g| *g);
            eprintln!(
                "{}",
                json!({ "runeStopped": {
                    "op": op,
                    "elapsedMs": elapsed,
                    "cause": cause,
                }})
            );
            return rune_stopped_response(op, elapsed, cause);
        }
    };
    let ctx = d.engine.context_snapshot(&d.desc).await;
    let changes = {
        let fp = ctx
            .get("fingerprint")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut guard = d.last_fp.lock().unwrap_or_else(|e| e.into_inner());
        let changes = match guard.as_ref() {
            Some(old) => crate::engine::fingerprint_changes(old, &fp),
            None => Vec::new(),
        };
        *guard = Some(fp);
        changes
    };
    let mut envelope = json!({ "ok": true, "value": value });
    envelope["engineContext"] = json!({
        "provenance": ctx.get("provenance").cloned().unwrap_or(serde_json::Value::Null),
        "daemon": d.desc.to_json(),
        "changes": changes,
    });
    (axum::http::StatusCode::OK, Json(envelope))
}

async fn eval_handler(
    State(st): State<AppState>,
    Json(req): Json<EvalRequest>,
) -> impl IntoResponse {
    let _ = st
        .daemon
        .last_activity
        .lock()
        .map(|mut t| *t = Instant::now());
    // 锁获取有界（#68 连坐修复，与 /rune 同款）
    let _slot = match tokio::time::timeout(Duration::from_secs(10), st.daemon.eval_lock.lock())
        .await
    {
        Ok(g) => g,
        Err(_) => {
            return err_response(anyhow::anyhow!(
                "引擎忙或正在退役（执行槽被占/闲置回收进行中，等 10 秒未获得；重试即通）；下一步：稍后重试，或 browse status 看引擎态"
            ));
        }
    };
    let d = &st.daemon;
    if req.new_tab
        && let Err(e) = d.engine.new_tab().await
    {
        return err_response(e);
    }
    // 懒引擎：未连接且片段没打算自己连，才预连（#61：spec 锁形，
    // 读侧克隆快照防长持锁）
    if !d.host.session().is_connected() && !req.code.contains("connect") {
        let spec = d.spec.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Err(e) = d.engine.ensure(&spec).await {
            return err_response(e);
        }
    }
    // 分流（#22）：js 旗标走全量 JS 旁路，缺省走方言
    let outcome =
        tokio::time::timeout(eval_timeout(), run_eval_code(&d.host, &req.code, req.js)).await;
    let value = match outcome {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            // 撞上「还没连」的片段：兜底拉引擎重试一次
            if format!("{e:#}").contains("Not connected") {
                let spec = d.spec.lock().unwrap_or_else(|e| e.into_inner()).clone();
                match d.engine.ensure(&spec).await {
                    Ok(_) => match run_eval_code(&d.host, &req.code, req.js).await {
                        Ok(v) => v,
                        Err(e2) => return err_response(e2),
                    },
                    Err(e2) => return err_response(e2),
                }
            } else {
                return err_response(e);
            }
        }
        Err(_) => {
            return err_response(anyhow::anyhow!(
                "eval 超时（{} 秒；BROWSE_EVAL_TIMEOUT 可调）",
                eval_timeout().as_secs()
            ));
        }
    };
    // #60 操作时刻引擎上下文（信封键，不进方言 value）：daemon 侧比对
    // 上次指纹出 changes；provenance 与 daemon 自描述同源 #59
    let ctx = d.engine.context_snapshot(&d.desc).await;
    let changes = {
        let fp = ctx
            .get("fingerprint")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut guard = d.last_fp.lock().unwrap_or_else(|e| e.into_inner());
        let changes = match guard.as_ref() {
            Some(old) => crate::engine::fingerprint_changes(old, &fp),
            None => Vec::new(),
        };
        *guard = Some(fp);
        changes
    };
    let mut envelope = json!({ "ok": true, "value": value });
    envelope["engineContext"] = json!({
        "provenance": ctx.get("provenance").cloned().unwrap_or(serde_json::Value::Null),
        "daemon": d.desc.to_json(),
        "changes": changes,
    });
    (axum::http::StatusCode::OK, Json(envelope))
}

/// rune 整体超时的结构化回执（v0.26.1 最小 CTA）：`error` 面保持 CTA
/// 文案（前缀律同 [`err_response`]），`timeout` 面给机器可读三字段——
/// `op` 在飞调用面（`js`/`goto`/…，执行间隙为 null）、`elapsed_ms` 已
/// 等待毫秒、`bound_s` 上界秒；CLI 侧把 `timeout` 对象一行 JSON 打到
/// stderr 供上层 agent 直接解析。
fn rune_timeout_response(
    op: Option<&str>,
    bound: Duration,
) -> (axum::http::StatusCode, Json<Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "ok": false,
            "error": format!(
                "browse: rune 执行超时（{} 秒；BROWSE_RUNE_TIMEOUT 可调，daemon 级上界实例内全部 rune 执行同界）；下一步：看 timeout.op 定位在飞调用面，核对长等待段（goto 等加载、js awaitPromise、wait_sel 轮询）或调大 BROWSE_RUNE_TIMEOUT 重试（长爬拆专用命名实例）",
                bound.as_secs()
            ),
            "timeout": {
                "op": op,
                "elapsed_ms": bound.as_millis() as u64,
                "bound_s": bound.as_secs(),
            }
        })),
    )
}

/// rune 主动停止的结构化回执（批次二）：形状与超时回执同构——`stopped`
/// 面给 `op`（在飞调用面）、`elapsed_ms`（实测毫秒）、`cause`
/// （`stop_request` 主动停 / `engine_lost` 引擎失联联动停）；CTA 按
/// cause 分流，对停止方是预期回执，对被停方指中断定位法。
fn rune_stopped_response(
    op: Option<&str>,
    elapsed_ms: u64,
    cause: &str,
) -> (axum::http::StatusCode, Json<Value>) {
    let cta = match cause {
        "engine_lost" => format!(
            "browse: rune 执行因引擎失联被主动停止（已跑 {elapsed_ms} 毫秒；两次连通性探测失败）；下一步：browse status 看引擎态（崩溃/被关/网络），重试即自动重拉引擎"
        ),
        _ => format!(
            "browse: rune 执行被主动停止（已跑 {elapsed_ms} 毫秒；POST /rune/stop 或 browse rune stop）；下一步：停止方属预期回执；被停方看 stopped.op 定位中断调用面，长等待段可拆批或改事件等待后重跑"
        ),
    };
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "ok": false,
            "error": cta,
            "stopped": {
                "op": op,
                "elapsed_ms": elapsed_ms,
                "cause": cause,
            }
        })),
    )
}

/// 引擎失联监视（批次二联动，总台指令）：执行期两次连续断连探测（各
/// 隔 300 毫秒）判失联即触发主动停止分支；容忍瞬时重连窗与导航期瞬断。
async fn engine_lost_watch(session: &Arc<cdp::Session>) {
    loop {
        tokio::time::sleep(Duration::from_millis(300)).await;
        if !session.is_connected() {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if !session.is_connected() {
                return;
            }
        }
    }
}

/// 主动停止在跑的 rune 执行（批次二）：取当前执行的取消发送端置真；无
/// 在跑执行时空放（幂等，回 running=false）。
async fn rune_stop_handler(State(st): State<AppState>) -> impl IntoResponse {
    let d = &st.daemon;
    let running = d.rune_running.load(Ordering::Relaxed);
    let tx = d
        .rune_cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let requested = tx.is_some_and(|t| t.send(true).is_ok());
    let note = if !running {
        "无在跑 rune 执行（停止请求空放，幂等）"
    } else if !requested {
        "执行刚收尾，停止未达（如需再停请重试）"
    } else {
        "停止信号已投递，在跑执行将以内嵌 stopped 面回执退出（若恰在收尾则以实际收尾回执为准）"
    };
    Json(json!({ "ok": true, "running": running, "requested": requested, "note": note }))
}

fn err_response(e: anyhow::Error) -> (axum::http::StatusCode, Json<Value>) {
    // 前缀在此统一加：错误串形态是「browse: <下一步指令>」进 stderr。
    // 错误链内已有前缀的（cdp 守卫、引擎面文案）不再叠加，防「browse: browse:」。
    // 错误链过密钥面具再上前缀（#25.4 评审 G4）：守卫文案嵌 id/url、CDP
    // 错误嵌参数，agent 拿 secrets.X 当 token/URL 片段是自然用法
    let msg = crate::js_host::mask_secrets_str(&format!("{e:#}"));
    let msg = if msg.starts_with("browse:") {
        msg
    } else {
        format!("browse: {msg}")
    };
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "ok": false, "error": msg })),
    )
}

async fn health_handler(State(st): State<AppState>) -> impl IntoResponse {
    Json(
        st.daemon
            .engine
            .health_json(st.daemon.started.elapsed(), &st.daemon.desc)
            .await,
    )
}

async fn engine_up_handler(
    State(st): State<AppState>,
    Json(req): Json<EngineUpRequest>,
) -> impl IntoResponse {
    let _ = st
        .daemon
        .last_activity
        .lock()
        .map(|mut t| *t = Instant::now());
    let spec = if let Some(ws) = req.ws {
        EngineSpec::Attach { ws_url: ws }
    } else if let Some(p) = req.port {
        EngineSpec::Port(p)
    } else {
        // 缺字段回落 from_env（评审 F2）：消除「同一 BROWSE_PROXY/
        // BROWSE_PROFILE 一会生效一会不生效」的旗标与 env 两态。走到本块
        // 时请求必未显式给 ws/port：env 钉了 BROWSE_CDP_WS（Attach）则
        // 钉死意图直通（不 panic，那是手册在册配置，二轮快核雷）
        let env_spec = EngineSpec::from_env(None, false, false);
        let (env_profile, env_proxy, env_bypass, env_engine_args, env_ws) = match env_spec {
            EngineSpec::Attach { ws_url } => (None, None, None, Vec::new(), Some(ws_url)),
            EngineSpec::Auto {
                profile,
                proxy,
                proxy_bypass,
                engine_args,
                ..
            } => (profile, proxy, proxy_bypass, engine_args, None),
            EngineSpec::Port(_) => (None, None, None, Vec::new(), None),
        };
        if let Some(ws_url) = env_ws {
            EngineSpec::Attach { ws_url }
        } else {
            // #48：env 段先行、显式 --engine-arg 随后（chrome 重复旗标
            // 后值胜，显式意图压过 daemon 启动残留 env；同值去重消
            // up 道 env+请求双份投递）
            let mut engine_args = env_engine_args;
            for a in &req.engine_args {
                if !engine_args.contains(a) {
                    engine_args.push(a.clone());
                }
            }
            EngineSpec::Auto {
                chrome: req.chrome.map(Into::into),
                headless: req.headless,
                pipe: req.pipe,
                profile: req.profile.map(Into::into).or(env_profile),
                proxy: req.proxy.clone().or(env_proxy),
                proxy_bypass: req.proxy_bypass.clone().or(env_bypass),
                isolated: req.isolated,
                engine_args,
            }
        }
    };
    match st.daemon.engine.ensure(&spec).await {
        Ok(_) => {
            // #61：显式 spawn 意图回写缺省 spec——引擎意外死掉后的懒重
            // spawn 沿用最近一次 up 的形态（headless 等），不再回退 daemon
            // 启动时的 from_env 缺省（有头）。只记 Auto（spawn）意图；
            // Attach 是连接意图不是重生意图，不回写
            if let EngineSpec::Auto { .. } = &spec {
                *st.daemon.spec.lock().unwrap_or_else(|e| e.into_inner()) = spec.clone();
            }
            (
                axum::http::StatusCode::OK,
                Json(
                    st.daemon
                        .engine
                        .health_json(st.daemon.started.elapsed(), &st.daemon.desc)
                        .await,
                ),
            )
        }
        Err(e) => err_response(e),
    }
}

async fn quit_handler(State(st): State<AppState>) -> impl IntoResponse {
    st.daemon.quit.store(true, Ordering::Relaxed);
    Json(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 错误前缀只加一次（#33 快核 G-lite）：cdp 守卫/引擎面文案自带
    /// 「browse: 」时不得叠成「browse: browse:」。
    #[test]
    fn err_response_prefixes_exactly_once() {
        let (_, Json(v)) = err_response(anyhow::anyhow!("browse: 守卫拦截 Browser.close"));
        let msg = v["error"].as_str().unwrap_or_default();
        assert_eq!(msg, "browse: 守卫拦截 Browser.close");

        assert!(!msg.starts_with("browse: browse:"), "{msg}");

        let (_, Json(v)) = err_response(anyhow::anyhow!("CDP Page.navigate: boom"));
        assert_eq!(
            v["error"].as_str().unwrap_or_default(),
            "browse: CDP Page.navigate: boom"
        );
    }

    /// rune 整体超时回执的结构化面（v0.26.1 最小 CTA）：`timeout` 三字段
    /// （op/elapsed_ms/bound_s）齐、错误文案带前缀与下一步。
    #[test]
    fn rune_timeout_envelope_has_structured_face() {
        let (code, Json(v)) = rune_timeout_response(Some("js"), Duration::from_secs(45));
        assert_eq!(code, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(v["ok"], json!(false));
        let msg = v["error"].as_str().unwrap_or_default();
        assert!(msg.starts_with("browse: rune 执行超时（45 秒"), "{msg}");
        assert!(
            msg.contains("BROWSE_RUNE_TIMEOUT"),
            "CTA 点名专属旋钮：{msg}"
        );
        assert!(msg.contains("下一步"), "{msg}");
        assert_eq!(v["timeout"]["op"], json!("js"));
        assert_eq!(v["timeout"]["elapsed_ms"], json!(45000));
        assert_eq!(v["timeout"]["bound_s"], json!(45));
        // 执行间隙超时：op 为 null（serde_json 的 null 形）
        let (_, Json(v)) = rune_timeout_response(None, Duration::from_secs(300));
        assert_eq!(v["timeout"]["op"], json!(null));
        assert_eq!(v["timeout"]["bound_s"], json!(300));
    }

    /// 主动停止回执的结构化面（批次二）：stopped 三字段（op/elapsed_ms/
    /// cause）与 CTA 文案（stop_request 与 engine_lost 双形）。
    #[test]
    fn rune_stopped_envelope_has_structured_face() {
        let (code, Json(v)) = rune_stopped_response(Some("goto"), 1234, "stop_request");
        assert_eq!(code, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(v["ok"], json!(false));
        let msg = v["error"].as_str().unwrap_or_default();
        assert!(
            msg.starts_with("browse: rune 执行被主动停止（已跑 1234 毫秒"),
            "{msg}"
        );
        assert!(msg.contains("stopped.op"), "{msg}");
        assert_eq!(v["stopped"]["op"], json!("goto"));
        assert_eq!(v["stopped"]["elapsed_ms"], json!(1234));
        assert_eq!(v["stopped"]["cause"], json!("stop_request"));
        // 引擎失联联动形：cause 与 CTA 分流
        let (_, Json(v)) = rune_stopped_response(Some("wait_event"), 5678, "engine_lost");
        let msg = v["error"].as_str().unwrap_or_default();
        assert!(msg.contains("引擎失联"), "{msg}");
        assert!(msg.contains("browse status"), "{msg}");
        assert_eq!(v["stopped"]["cause"], json!("engine_lost"));
    }

    /// 引擎失联监视：未连接会话（Session::new）两次探测即返回——联动
    /// 分支可达；已连接会话不触发（用真引擎的 e2e 面覆盖）。
    #[tokio::test]
    async fn engine_lost_watch_fires_on_dead_session() {
        let session = std::sync::Arc::new(cdp::Session::new());
        let fired = tokio::time::timeout(Duration::from_secs(2), engine_lost_watch(&session)).await;
        assert!(fired.is_ok(), "未连接会话应在两次探测后触发失联分支");
    }

    /// 错误链脱敏调用点锁（#25.4 评审 G4 二轮）：err_response 必须过
    /// [`crate::js_host::mask_secrets_str`]——守卫文案嵌 id/url、CDP 错误
    /// 嵌参数，密钥值不得经错误出口上 stderr；CTA 保留。
    #[tokio::test]
    async fn err_response_masks_secret_values() {
        let _ser = crate::js_host::SECRETS_TEST_LOCK.lock().await;
        let dir = std::env::temp_dir().join(format!("browse-sec-srv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let f = dir.join("s.env");
        std::fs::write(&f, "TOKEN=sk-live-abc123\n").unwrap();
        crate::js_host::load_secrets(f.to_str().unwrap()).expect("装仓");
        // 复刻守卫错误形态（评审方实弹件：Target.closeTarget 带密钥当 id）
        let (_, Json(v)) = err_response(anyhow::anyhow!(
            "守卫拦截 Target.closeTarget：sk-live-abc123 不是本会话自建 tab；下一步：只关 listPageTargets()"
        ));
        let msg = v["error"].as_str().unwrap_or_default();
        assert!(!msg.contains("sk-live-abc123"), "密钥值应被换: {msg}");
        assert!(msg.contains("***"), "{msg}");
        assert!(msg.contains("下一步"), "CTA 应保留: {msg}");
        // 前缀形态不回归（脱敏不破坏 browse: 头）
        assert!(msg.starts_with("browse: "), "{msg}");
        // 清仓防串测（空文件 = 清空全局）
        std::fs::write(&f, "").unwrap();
        crate::js_host::load_secrets(f.to_str().unwrap()).expect("清仓");
        std::fs::remove_dir_all(&dir).ok();
    }
}
