//! daemon 的 HTTP API：常驻会话 + 方言求值 + 引擎生命周期。
//!
//! 移植自 browser-harness-rs `src/server.rs`（POST /eval、GET /health、
//! POST /quit 三端点保留），新增 `/engine/up` 与求值前的懒引擎策略：
//!
//! - 求值前未连接且片段里没有 `session.connect` -> 自动 [`crate::Engine::ensure`]；
//!   片段自带 `session.connect` 则不预连（显式意图优先，ADR-0004）。
//! - 求值撞上「Not connected」再兜底 ensure + 重试一次（覆盖先语句后连接的写法）。
//! - 单飞槽：同一时刻只跑一条片段，后来者排队（不拒 429）。
//! - 兜底硬界（REQ-027 统一超时，ADR-0012）：/eval 与 /rune 同缺省 60
//!   秒，请求级 `timeout_s`（`--timeout` 旗标）覆写 env 旋钮缺省
//!   （`BROWSE_EVAL_TIMEOUT`/`BROWSE_RUNE_TIMEOUT`）；到期结构化退出带
//!   在飞面卡点（/eval 方言函数探针、/rune OpProbe）。

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
    /// 本次求值的兜底硬界秒（REQ-027 统一兜底超时；`--timeout` 旗标递
    /// 入，请求级覆写 env 缺省）。与 wait 族等待窗正交（ADR-0012）。
    #[serde(default)]
    pub timeout_s: Option<u64>,
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
    /// 模板定界形态（0.30.3 缺省 ERB；`classic`/`erb`，CLI 旗标收口递
    /// 入，未知值回落缺省）。
    #[serde(default)]
    pub vars_syntax: Option<String>,
    /// 本次执行的兜底硬界秒（REQ-027 统一兜底超时；`--timeout` 旗标递
    /// 入，请求级覆写 `BROWSE_RUNE_TIMEOUT` 缺省）。与 wait 族等待窗
    /// 正交（ADR-0012）。
    #[serde(default)]
    pub timeout_s: Option<u64>,
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

/// 环境旋钮秒值解析（REQ-027 统一兜底超时）：经 [`crate::js_host::
/// secs_to_ms`] 防误写守卫（>=1000 按毫秒换算并 daemon 留痕告警），
/// 缺省回落 default_s。
fn env_timeout_secs(var: &str, default_s: u64) -> Duration {
    match std::env::var(var).ok().and_then(|s| s.parse::<u64>().ok()) {
        None => Duration::from_secs(default_s),
        Some(v) => {
            // 0 值拒绝（评审 G-r8）：0 秒界=每次调用必超时的病态值，回落
            // 缺省并一次性留痕（CLI 面 --timeout 0 已直出 exit 2）
            if v == 0 {
                static WARNED_ZERO: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> =
                    std::sync::OnceLock::new();
                let seen = WARNED_ZERO.get_or_init(Default::default);
                let fresh = seen
                    .lock()
                    .map(|g| !g.iter().any(|x| x == var))
                    .unwrap_or(false);
                if fresh {
                    if let Ok(mut g) = seen.lock() {
                        g.push(var.to_string());
                    }
                    eprintln!("[browse] {var}=0 是病态界（即刻超时），回落缺省 {default_s} 秒");
                }
                return Duration::from_secs(default_s);
            }
            let (ms, warn) = crate::js_host::secs_to_ms(v);
            if let Some(w) = warn {
                // 每旋钮每进程只警示一次（评审 G-1）：本函数在请求路径逐
                // 次调（rune 超时臂还会复算），裸打会每请求刷行
                static WARNED: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> =
                    std::sync::OnceLock::new();
                let seen = WARNED.get_or_init(Default::default);
                let fresh = seen
                    .lock()
                    .map(|g| !g.iter().any(|x| x == var))
                    .unwrap_or(false);
                if fresh {
                    if let Ok(mut g) = seen.lock() {
                        g.push(var.to_string());
                    }
                    eprintln!("[browse] {var}={v} {w}（按换算值生效）");
                }
            }
            Duration::from_millis(ms)
        }
    }
}

/// 统一兜底硬界缺省（REQ-027，用户令 2026-10-03 终裁「timeout 默认 60
/// 秒」）：两执行道同缺省 60 秒，请求级 `--timeout` 与 env 旋钮可调。
/// 沿革：rune 前缺省 45（v0.26.1 战役台诉求）、eval 前缺省 300（REQ-016
/// 契约）、中途拟 180 后收敛 60，本批统一收口（ADR-0012 在册）。
const UNIFIED_TIMEOUT_DEFAULT_S: u64 = 60;

/// /eval 的兜底硬界（REQ-027）：请求级 `timeout_s`（`--timeout` 旗标）
/// 覆写 env 缺省（`BROWSE_EVAL_TIMEOUT`）。
fn eval_timeout_for(req_s: Option<u64>) -> Duration {
    req_s.map_or_else(
        || env_timeout_secs("BROWSE_EVAL_TIMEOUT", UNIFIED_TIMEOUT_DEFAULT_S),
        Duration::from_secs,
    )
}

/// rune 面整体执行兜底硬界（ADR-0010 增补终裁 2026-10-02；REQ-027 请求
/// 级覆写）：请求级 `timeout_s`（`--timeout` 旗标）覆写 env 缺省
/// （`BROWSE_RUNE_TIMEOUT`）。daemon 级语义修订：env 是实例内缺省，
/// 请求级可逐次覆写（长爬逐次调，不再全局定格）。
fn rune_timeout_for(req_s: Option<u64>) -> Duration {
    req_s.map_or_else(
        || env_timeout_secs("BROWSE_RUNE_TIMEOUT", UNIFIED_TIMEOUT_DEFAULT_S),
        Duration::from_secs,
    )
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
/// rune 面与方言面同一实现。`bridge_deadline` 是同源腿的单调用上界，
/// 取请求级兜底硬界（评审 kimi G-6：恒 60 秒把
/// `dialect("fetch", [url, {timeout: 300}])` 这类长预算方言腿在 60 秒
/// 截死；缺省仍 60 秒，`--timeout` 覆写时同随——rune 整体界封顶总预算）。
fn host_fns(host: &std::sync::Arc<JsHost>, bridge_deadline: Duration) -> browse_rune::HostFns {
    let wrap = |name: &'static str| -> browse_rune::module::HostFn {
        let h = std::sync::Arc::clone(host);
        let bound = bridge_deadline;
        std::sync::Arc::new(move |args: Value| {
            let h = std::sync::Arc::clone(&h);
            Box::pin(async move {
                let argv: Vec<Value> = args.as_array().cloned().unwrap_or_default();
                // #68:js_host 腿自身无逐调用 deadline,闭包层整体上界
                // (同源三支柱的最坏复合调用面也远低于缺省 60s)
                tokio::time::timeout(bound, h.global_json(name, &argv))
                    .await
                    .map_err(|_| {
                        format!(
                            "{name} 同源调用超时（{} 秒上界）；下一步:browse status 看引擎态",
                            bound.as_secs()
                        )
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
        // 方言全量桥通用腿（issue #75）：载荷 {name, args} 解包转
        // global_json 按名派发——方言面全部全局函数一条通道；上界
        // 与三支柱同律（随请求级界）
        dialect: Some({
            let h = std::sync::Arc::clone(host);
            let bound = bridge_deadline;
            std::sync::Arc::new(move |v: Value| {
                let h = std::sync::Arc::clone(&h);
                Box::pin(async move {
                    let name = v
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            "dialect 载荷缺 name；下一步：这是 rn 侧组装面问题，报告 browse issue"
                                .to_string()
                        })?
                        .to_string();
                    let argv: Vec<Value> = v
                        .get("args")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    tokio::time::timeout(bound, h.global_json(&name, &argv))
                        .await
                        .map_err(|_| {
                            format!(
                                "dialect 同源调用超时（{} 秒上界）；下一步：browse status 看引擎态",
                                bound.as_secs()
                            )
                        })?
                        .map_err(|e| format!("{e:#}"))
                })
                    as std::pin::Pin<
                        Box<dyn std::future::Future<Output = Result<Value, String>> + Send>,
                    >
            })
        }),
    }
}

/// 丢弃在飞执行体后的 pending 僵尸回收（评审 G-3）：超时/停止丢弃
/// future 时，内层调用的清登记路径被一并丢弃，在飞 CDP 调用登记成僵尸
/// ——32 秒后扫 30 秒以上老项（与 skills.rs 的 probe 超时臂同款；正常
/// 调用早已应答出表，reaper 只扫真僵尸）。
fn spawn_pending_reaper(session: &std::sync::Arc<cdp::Session>) {
    let ses = std::sync::Arc::clone(session);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(32)).await;
        let n = ses.reap_pending_older_than(Duration::from_secs(30)).await;
        if n > 0 {
            eprintln!("[browse] 执行超时/停止回收 pending 僵尸 {n} 条");
        }
    });
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
        // 桥上界随请求级兜底硬界（评审 kimi G-6）
        host_fns: host_fns(&d.host, rune_timeout_for(req.timeout_s)),
        // v0.26.1 超时 CTA：探针克隆自持，超时丢弃执行 future 后读在飞面
        op_probe: browse_rune::OpProbe::default(),
        // 模板定界（0.30.3 缺省 ERB）：CLI 旗标/件指令收口后随请求递入
        vars_syntax: req
            .vars_syntax
            .as_deref()
            .and_then(browse_rune::vars_syntax::VarsSyntax::parse)
            .unwrap_or_default(),
    };
    let op_probe = opts.op_probe.clone();
    // 主动停止道（批次二总台指令）：本执行的取消通道入槽，select 上与
    // 整体超时并列；停止即丢弃执行 future（清账走 Drop 兜底，与超时同构）
    let started = Instant::now();
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    *d.rune_cancel.lock().unwrap_or_else(|e| e.into_inner()) = Some(cancel_tx);
    d.rune_running.store(true, Ordering::Relaxed);
    // 恐慌滞留守卫（评审 G-41）：store(true) 与 store(false) 之间处理器
    // panic 时旗标永滞 true（stop 道拒绝服务）；Drop 守卫在 unwind 与
    // 正常返回两道都复位
    struct RunningGuard(std::sync::Arc<AtomicBool>);
    impl Drop for RunningGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Relaxed);
        }
    }
    let _running_guard = RunningGuard(std::sync::Arc::clone(&d.rune_running));
    let watch_session = d.host.session();
    enum Outcome {
        Done(anyhow::Result<Value>),
        Timeout,
        Stopped,
        EngineLost,
    }
    let outcome = tokio::select! {
        r = tokio::time::timeout(
            rune_timeout_for(req.timeout_s),
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
            // 整体超时（v0.26.1 最小 CTA，批次二换 rune 专属旋钮；REQ-027
            // 请求级覆写）：结构化 timeout 面（op 在飞调用面、elapsed_ms
            // 已等待毫秒、bound_s 上界秒）加 daemon 日志行；错误文案带卡点
            let bound = rune_timeout_for(req.timeout_s);
            let op = probe_snapshot_json(&op_probe);
            eprintln!(
                "{}",
                json!({ "runeTimeout": {
                    "op": op,
                    "elapsedMs": bound.as_millis() as u64,
                    "boundS": bound.as_secs(),
                }})
            );
            // 丢弃执行体后 pending 登记成僵尸（评审 G-3，与 probe 臂同款）
            spawn_pending_reaper(&watch_session);
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
            let op = probe_snapshot_json(&op_probe);
            eprintln!(
                "{}",
                json!({ "runeStopped": {
                    "op": op,
                    "elapsedMs": elapsed,
                    "cause": cause,
                }})
            );
            // 停止道同丢弃执行体（评审 G-3）
            spawn_pending_reaper(&watch_session);
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
    let bound = eval_timeout_for(req.timeout_s);
    let eval_started = Instant::now();
    let outcome = tokio::time::timeout(bound, run_eval_code(&d.host, &req.code, req.js)).await;
    let value = match outcome {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            // 撞上「还没连」的片段：兜底拉引擎重试一次；重试腿整段
            // （ensure 冷拉 + 重求值）同受兜底硬界约束（全量审 M3 按
            // 剩余时间包一层；评审 G-1：ensure 冷 spawn 最坏数十秒
            // ——等口 15s+connect+握手——在界外裸跑同样脱界占槽）
            if format!("{e:#}").contains("Not connected") {
                let remaining = bound.saturating_sub(eval_started.elapsed());
                let spec = d.spec.lock().unwrap_or_else(|e| e.into_inner()).clone();
                match tokio::time::timeout(remaining, async {
                    d.engine.ensure(&spec).await?;
                    run_eval_code(&d.host, &req.code, req.js).await
                })
                .await
                {
                    Ok(Ok(v)) => v,
                    Ok(Err(e2)) => return err_response(e2),
                    Err(_) => {
                        let op = d.host.current_op_snapshot();
                        eprintln!(
                            "{}",
                            json!({ "evalTimeout": {
                                "op": op,
                                "elapsedMs": bound.as_millis() as u64,
                                "boundS": bound.as_secs(),
                            }})
                        );
                        // 丢弃执行体后 pending 登记成僵尸（评审 G-3）
                        spawn_pending_reaper(&d.host.session());
                        return eval_timeout_response(op, bound);
                    }
                }
            } else {
                return err_response(e);
            }
        }
        Err(_) => {
            // 兜底硬界到期（REQ-027 统一超时）：结构化 timeout 面与 /rune
            // 同形（op 在飞方言函数面、elapsed_ms、bound_s）加 daemon 日
            // 志行；错误文案带卡点与 CTA
            let op = d.host.current_op_snapshot();
            eprintln!(
                "{}",
                json!({ "evalTimeout": {
                    "op": op,
                    "elapsedMs": bound.as_millis() as u64,
                    "boundS": bound.as_secs(),
                }})
            );
            // 丢弃执行体后 pending 登记成僵尸（评审 G-3）
            spawn_pending_reaper(&d.host.session());
            return eval_timeout_response(op, bound);
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
fn rune_timeout_response(op: Value, bound: Duration) -> (axum::http::StatusCode, Json<Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "ok": false,
            "error": format!(
                "browse: rune 执行超时（{} 秒；--timeout 旗标请求级覆写或 BROWSE_RUNE_TIMEOUT 可调，env 是实例内缺省）；下一步：看 timeout.op 定位在飞调用面，核对长等待段（goto 等加载、js awaitPromise、wait_event 事件窗、raw 裸调用的 30 秒界）或调大界重试（长爬拆专用命名实例；与 wait 族等待窗正交，等待窗参数自调）",
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

/// /eval 兜底硬界到期的结构化回执（REQ-027 统一兜底超时）：形状与
/// [`rune_timeout_response`] 同构——`op` 是在飞方言函数面（JsHost 探针；
/// null 即执行间隙或 `--js`/`session.*` 直调面，无函数名可点）、
/// `elapsed_ms` 已等待毫秒、`bound_s` 上界秒；CTA 带
/// 卡点定位法（等待类函数收窗参 vs 页面执行慢）。
fn eval_timeout_response(
    op: Option<String>,
    bound: Duration,
) -> (axum::http::StatusCode, Json<Value>) {
    let op_desc = op.clone().unwrap_or_else(|| {
        "执行间隙（两函数之间；--js 与 session.* 直调面同此形，无函数名可点）".into()
    });
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "ok": false,
            "error": format!(
                "browse: eval 超时（{} 秒；--timeout 旗标请求级覆写或 BROWSE_EVAL_TIMEOUT 可调）；卡点：{}；下一步：卡在等待类函数（waitForResponse/waitJs/waitFor）就收窄其等待窗参或确认触发条件，卡在执行类（goto 加载慢、pageEval 重脚本）就调大界重试；等待窗参数与本兜底界正交（ADR-0012）",
                bound.as_secs(),
                op_desc
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
/// 探针多槽快照（0.30.2）：0 在飞 null、1 个字符串、多个数组（并发在
/// 飞时全列，消单槽盲区）。
fn probe_snapshot_json(probe: &browse_rune::OpProbe) -> Value {
    let ops = probe.lock().ok().map(|g| g.clone()).unwrap_or_default();
    match ops.len() {
        0 => json!(null),
        1 => json!(ops[0]),
        _ => json!(ops),
    }
}

fn rune_stopped_response(
    op: Value,
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

    /// REQ-027 统一兜底超时：两执行道同缺省 60 秒；请求级覆写直通；
    /// /eval 结构化超时回执与 rune 同形（op/elapsed_ms/bound_s + 卡点
    /// CTA；执行间隙 op 为 null）。
    #[test]
    fn unified_timeout_defaults_and_receipt() {
        assert_eq!(
            eval_timeout_for(None),
            Duration::from_secs(60),
            "eval 缺省 60（本测试假定 BROWSE_EVAL_TIMEOUT 未设）"
        );
        assert_eq!(
            rune_timeout_for(None),
            Duration::from_secs(60),
            "rune 缺省 60（本测试假定 BROWSE_RUNE_TIMEOUT 未设）"
        );
        assert_eq!(eval_timeout_for(Some(5)), Duration::from_secs(5));
        assert_eq!(rune_timeout_for(Some(300)), Duration::from_secs(300));
        let (_, Json(v)) =
            eval_timeout_response(Some("waitForResponse".into()), Duration::from_secs(60));
        assert_eq!(v["ok"], json!(false));
        assert_eq!(v["timeout"]["op"], json!("waitForResponse"));
        assert_eq!(v["timeout"]["bound_s"], json!(60));
        assert_eq!(v["timeout"]["elapsed_ms"], json!(60_000));
        let msg = v["error"].as_str().unwrap_or_default();
        assert!(
            msg.contains("卡点：waitForResponse") && msg.contains("下一步"),
            "{msg}"
        );
        let (_, Json(v)) = eval_timeout_response(None, Duration::from_secs(60));
        assert_eq!(v["timeout"]["op"], json!(null), "执行间隙 null");
    }

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
        let (code, Json(v)) = rune_timeout_response(json!("js"), Duration::from_secs(45));
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
        let (_, Json(v)) = rune_timeout_response(json!(null), Duration::from_secs(300));
        assert_eq!(v["timeout"]["op"], json!(null));
        assert_eq!(v["timeout"]["bound_s"], json!(300));
    }

    /// 主动停止回执的结构化面（批次二）：stopped 三字段（op/elapsed_ms/
    /// cause）与 CTA 文案（stop_request 与 engine_lost 双形）。
    #[test]
    fn rune_stopped_envelope_has_structured_face() {
        let (code, Json(v)) = rune_stopped_response(json!("goto"), 1234, "stop_request");
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
        let (_, Json(v)) = rune_stopped_response(json!("wait_event"), 5678, "engine_lost");
        let msg = v["error"].as_str().unwrap_or_default();
        assert!(msg.contains("引擎失联"), "{msg}");
        assert!(msg.contains("browse status"), "{msg}");
        assert_eq!(v["stopped"]["cause"], json!("engine_lost"));
    }

    /// 多槽快照（0.30.2）：0 在飞 null、1 字符串、N 数组。
    #[test]
    fn probe_snapshot_multiset_face() {
        let p = browse_rune::OpProbe::default();
        assert_eq!(probe_snapshot_json(&p), json!(null));
        p.lock().unwrap().push("js");
        assert_eq!(probe_snapshot_json(&p), json!("js"));
        p.lock().unwrap().push("goto");
        assert_eq!(probe_snapshot_json(&p), json!(["js", "goto"]));
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
