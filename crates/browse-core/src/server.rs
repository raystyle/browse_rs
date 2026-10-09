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

/// 一条油猴（userscript）事件回调（#77）：脚本经 `GM_xmlhttpRequest`
/// `POST /events` 上报的页面事件，agent 侧经 `browse events` 消费。
/// 何时用：页面内事件要主动通知宿主（区别于轮询/快照拉取）的探针形
/// 集成。边界：只承载上报与消费，脚本安装管理归引擎面（Userscript.\*
/// CDP 域，clean-chrome 批 4），两者正交。
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserscriptEvent {
    /// 单调序号（daemon 生命周期内递增，`--since` 消费游标）。
    pub seq: u64,
    /// daemon 侧收到时刻（Unix 毫秒）。
    pub ts_ms: u64,
    /// 事件类型（标准形两档）：`see`（观察上报）/ `stop`（条件停止）。
    pub r#type: String,
    /// 技能 slug（如 `challenge-stop`；裸事件无 slug 时为 `None`）。
    pub slug: Option<String>,
    /// daemon 生成的建议句（响应字段,非请求字段,P0 收口）：合法形只有
    /// 冻结 slug × 动作枚举的模板（27 条闭集）；探针只发 `(action, slug)`,
    /// 页面不可供命令文本。消费契约:cta 是候选命令,`data`/`url` 是观测。
    pub cta: Option<String>,
    /// 上报页面 URL（脚本自报的 `location.href`）。
    pub url: Option<String>,
    /// 任意载荷（超限序列化截断，见 [`EventRing::push`]）。
    pub data: Option<Value>,
}

/// 事件环（#77）：容量有界的环形缓冲，满即丢最旧，序号单调递增。
/// 纯同步逻辑（`std::sync::Mutex` 包裹进 [`Daemon`]），handler 层做
/// 校验与截断后入环。
#[derive(Debug, Default)]
pub struct EventRing {
    next_seq: u64,
    buf: std::collections::VecDeque<UserscriptEvent>,
    /// P1 服务端下限表（grok 评审）：`(slug, action)` 最近接纳时刻(ms),
    /// see 事件同键 2 秒内拒收（客户端 throttleMs 是省请求,这才是界）。
    floor: std::collections::HashMap<(String, String), u64>,
}

/// 事件环容量：1000 条（页面侧失控上报也不打爆 daemon 内存）。
const EVENT_RING_CAP: usize = 1000;
/// 单条事件 `name`/`url` 截断上限（字符）。
const EVENT_STR_CAP: usize = 512;
/// 单条事件 `data` 序列化字节上限：超出截断并以 `__truncated` 标记。
const EVENT_DATA_CAP: usize = 32 * 1024;
/// P1 服务端下限窗：同一 `(slug, action)` 的 see 事件 2 秒内只收一条。
const EVENT_FLOOR_MS: u64 = 2_000;
/// P1 单 slug 在环的 see 条数帽：失控探针不许独占环（stop 不计不受逐）。
const EVENT_PER_SLUG_CAP: usize = 20;
/// P1 cta 优先序（冻结,安全类先于爬虫类）：`next` 的选取依据。
pub const CTA_PRIORITY: &[&str] = &[
    "challenge-stop",
    "antibot-vendor",
    "fingerprint-watch",
    "policy-surface",
    "scroll-until-end",
    "next-page",
    "spa-route",
    "response-tap",
    "link-table",
];

impl EventRing {
    /// 入环一条（标准形）：`type` 两档、`slug` 可空（各字段超限截断），
    /// `data` 序列化超上限字节截断成 `{__truncated: true, bytes: 原长}`。
    /// 返回分配的序号;P1 服务端下限命中（see 同 `(slug, action)` 2 秒内
    /// 重复）返回 `None`（事件不入环）。
    pub fn push(
        &mut self,
        r#type: String,
        slug: Option<String>,
        url: Option<String>,
        data: Option<Value>,
        action: Option<&str>,
        ts_ms: u64,
    ) -> Option<u64> {
        let clamp = |s: String| s.chars().take(EVENT_STR_CAP).collect::<String>();
        // P1 服务端下限:see 同 (slug, action) 2 秒内拒收(返回 None,
        // 调用方回 throttled;stop 不设下限——打断信号不容丢)。首见键
        // 不查下限(floor 表里没有即从未收过)
        let key = (
            slug.clone().unwrap_or_default(),
            action.unwrap_or_default().to_string(),
        );
        if r#type == "see"
            && let Some(last) = self.floor.get(&key)
            && ts_ms.saturating_sub(*last) < EVENT_FLOOR_MS
        {
            return None;
        }
        // P0 收口:cta 只由 (冻结 slug, 动作枚举) 模板生成,请求里任何
        // 自由串都不参与;非法组合事件保留、cta 键不出现;stop 不带 cta
        let cta = if r#type == "see" {
            cta_for(slug.as_deref(), action)
        } else {
            None
        };
        let data = data.map(|d| {
            let full = serde_json::to_string(&d).unwrap_or_default();
            if full.len() > EVENT_DATA_CAP {
                json!({ "__truncated": true, "bytes": full.len() })
            } else {
                d
            }
        });
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        let is_see = r#type == "see";
        self.buf.push_back(UserscriptEvent {
            seq,
            ts_ms,
            r#type: clamp(r#type),
            slug: slug.map(clamp),
            url: url.map(clamp),
            data,
            cta,
        });
        if is_see {
            self.floor.insert(key, ts_ms);
        }
        // P1 驱逐律:先满 per-slug 帽再满环帽,都只逐 see 不逐 stop
        // (stop 条数天然少且是打断信号,不许被观测挤掉)
        self.enforce_caps();
        Some(seq)
    }

    /// P1 容量纪律:驱逐只针对 see。per-slug 帽从最旧逐起;环帽优先逐
    /// 最旧的 see,全是 stop 时才逐最旧条目(理论不会到:帽 1000 远大于
    /// stop 量级)。
    fn enforce_caps(&mut self) {
        let mut per_slug: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for e in &self.buf {
            if e.r#type == "see"
                && let Some(s) = &e.slug
            {
                *per_slug.entry(s.clone()).or_default() += 1;
            }
        }
        for (s, n) in per_slug {
            let mut to_drop = n.saturating_sub(EVENT_PER_SLUG_CAP);
            if to_drop == 0 {
                continue;
            }
            let mut i = 0;
            while i < self.buf.len() && to_drop > 0 {
                let hit = self.buf[i].r#type == "see" && self.buf[i].slug.as_deref() == Some(&s);
                if hit {
                    self.buf.remove(i);
                    to_drop -= 1;
                } else {
                    i += 1;
                }
            }
        }
        while self.buf.len() > EVENT_RING_CAP {
            match self.buf.iter().position(|e| e.r#type == "see") {
                Some(i) => {
                    self.buf.remove(i);
                }
                None => {
                    self.buf.pop_front();
                }
            }
        }
    }

    /// 取序号严格大于 `since` 的全部事件（升序）。
    pub fn since(&self, since: u64) -> Vec<UserscriptEvent> {
        self.buf.iter().filter(|e| e.seq > since).cloned().collect()
    }

    /// 取最近 `n` 条（升序返回；`n` 为 0 取全部）。
    pub fn last(&self, n: usize) -> Vec<UserscriptEvent> {
        if n == 0 || n >= self.buf.len() {
            return self.buf.iter().cloned().collect();
        }
        self.buf.iter().skip(self.buf.len() - n).cloned().collect()
    }

    /// 清空（序号继续递增不回卷，消费游标不失位）。
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// 下一个将分配的序号（消费面回执的 `nextSeq`）。
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// P1 `next` 选取：全环带合法 cta 的 see 里，按 [`CTA_PRIORITY`]
    /// 取优先级最高（序值最小）的一条；同级取最新（尾部）。空环或无
    /// 建议返回 `None`。stop 不参与（它已走打断道，不该再领 agent）。
    pub fn next_cta(&self) -> Option<UserscriptEvent> {
        self.buf
            .iter()
            .rev()
            .filter(|e| e.r#type == "see" && e.cta.is_some())
            .min_by_key(|e| {
                e.slug
                    .as_deref()
                    .and_then(|s| CTA_PRIORITY.iter().position(|p| *p == s))
                    .unwrap_or(usize::MAX)
            })
            .cloned()
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
    /// 油猴事件回调环（#77）：`POST /events` 入环，`browse events`
    /// 消费。锁形因 Daemon 在 Arc 后共享。
    pub events_ring: std::sync::Mutex<EventRing>,
    /// 页内条件停止的捕获槽（#77 stop api，只留最后一次）：`POST /userscript/stop`
    /// 先落槽再打取消，被停 rune 的 stopped 回执从槽取 capture（槽是
    /// 真源，事件环只是副本）；新 rune 起跑即清槽防陈货串场。
    pub stop_capture: std::sync::Mutex<Option<Value>>,
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
            events_ring: std::sync::Mutex::new(EventRing::default()),
            stop_capture: std::sync::Mutex::new(None),
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
        .route(
            "/events",
            post(events_create_handler)
                .get(events_handler)
                .delete(events_clear_handler),
        )
        .route("/userscript/stop", post(userscript_stop_handler))
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
            .unwrap_or_else(|| crate::paths::scripts_root().to_string_lossy().into_owned()),
    );
    let opts = browse_rune::RunOpts {
        scripts_root: root,
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
    // 页内条件停止的捕获槽清场（#77 stop api）：上一轮残留不串进本次回执
    *d.stop_capture.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
            // 页内条件停止（#77）：捕获槽有货即 userscript_stop 因（槽只在真停到
            // rune 时落），否则按 stop_request / engine_lost 二分流
            let capture = d
                .stop_capture
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            let cause = if matches!(outcome, Outcome::EngineLost) {
                "engine_lost"
            } else if capture.is_some() {
                "userscript_stop"
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
            return rune_stopped_response(op, elapsed, cause, capture);
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
    capture: Option<Value>,
) -> (axum::http::StatusCode, Json<Value>) {
    let cta = match cause {
        "engine_lost" => format!(
            "browse: rune 执行因引擎失联被主动停止（已跑 {elapsed_ms} 毫秒；两次连通性探测失败）；下一步：browse status 看引擎态（崩溃/被关/网络），重试即自动重拉引擎"
        ),
        "userscript_stop" => format!(
            "browse: rune 执行被页内条件停止（已跑 {elapsed_ms} 毫秒；POST /userscript/stop slug={}）；下一步：看 stopped.capture（脚本捕获的 what），编排断点即 stopped.op",
            capture
                .as_ref()
                .and_then(|c| c.get("slug"))
                .and_then(Value::as_str)
                .unwrap_or("userscript/stop")
        ),
        _ => format!(
            "browse: rune 执行被主动停止（已跑 {elapsed_ms} 毫秒；POST /rune/stop 或 browse rune stop）；下一步：停止方属预期回执；被停方看 stopped.op 定位中断调用面，长等待段可拆批或改事件等待后重跑"
        ),
    };
    let mut stopped = json!({
        "op": op,
        "elapsed_ms": elapsed_ms,
        "cause": cause,
    });
    if let Some(c) = capture {
        stopped["capture"] = c;
    }
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "ok": false,
            "error": cta,
            "stopped": stopped,
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

/// `POST /events` 请求体（#77 油猴事件回调，标准形）：`type` 两档
/// （`see` 观察 / `stop` 条件停止）加 `slug`（技能名，可空）；兼容旧
/// `name` 前缀形（`see.<slug>` / `stop.<slug>`，前缀推导两字段）。
#[derive(Debug, Deserialize)]
pub struct EventCreateRequest {
    /// 事件类型：`see` / `stop`（缺省走 `name` 推导，两者全缺 400）。
    #[serde(default, rename = "type")]
    pub r#type: Option<String>,
    /// 技能 slug（标准形主键；截 512 字符）。
    #[serde(default)]
    pub slug: Option<String>,
    /// 旧形事件名（`see.<slug>`/`stop.<slug>` 前缀推导；type 给了则以
    /// type 为准）。
    #[serde(default)]
    pub name: Option<String>,
    /// 上报页面 URL。
    #[serde(default)]
    pub url: Option<String>,
    /// 任意载荷（环侧截断）。
    #[serde(default)]
    pub data: Option<Value>,
    /// 建议动作枚举（P0 收口,2026-10-10 grok 安全评审）:`read`/`rune`/
    /// `export` 三档。探针只发枚举,cta 句子由 daemon 模板生成——页面
    /// 可控侧不得供命令文本（间接注入面,同 FROZEN_PAGE_SLUGS 律）。
    #[serde(default)]
    pub action: Option<String>,
}

/// 请求体到环字段的标准形收口：type 两档校验（非法 400 由调用方），旧
/// `name` 前缀在此推导（`see.`/`stop.` 前缀剥给 slug；裸名当 `see` 且
/// slug 留空）。返回 `(type, slug)`。
fn normalize_event_kind(req: &EventCreateRequest) -> anyhow::Result<(String, Option<String>)> {
    if let Some(t) = req.r#type.as_deref() {
        if t != "see" && t != "stop" {
            anyhow::bail!(
                "events 的 type 只认 see/stop（当前：{t}）；下一步：观察上报用 see，条件停止用 stop"
            );
        }
        return Ok((t.to_string(), req.slug.clone()));
    }
    if let Some(n) = req.name.as_deref() {
        let n = n.trim();
        if n.is_empty() {
            return Ok((String::new(), None));
        }
        if let Some(slug) = n.strip_prefix("stop.") {
            return Ok(("stop".into(), Some(slug.to_string())));
        }
        if let Some(slug) = n.strip_prefix("see.") {
            return Ok(("see".into(), Some(slug.to_string())));
        }
        return Ok(("see".into(), None));
    }
    anyhow::bail!(
        "events 请求缺 type（标准形）或 name（旧形），两者至少其一；下一步：{{\"type\":\"see\",\"slug\":\"challenge-stop\"}}"
    )
}

/// 建议句模板生成（P0 收口,grok 安全评审 2026-10-10）：合法闭集 =
/// `FROZEN_PAGE_SLUGS` × {read, rune, export} 27 条;slug 不在白名单或
/// action 非枚举返回 `None`（事件保留,cta 键不出现）。模板外的字符串
/// 一律不进环——页面可控侧不得供命令文本（间接注入面）。
fn cta_for(slug: Option<&str>, action: Option<&str>) -> Option<String> {
    let slug = slug?;
    if !crate::skills::FROZEN_PAGE_SLUGS.contains(&slug) {
        return None;
    }
    match action? {
        "read" => Some(format!("browse workspace page {slug}")),
        "rune" => Some(format!("browse rune page-skills/{slug}/mech.rn")),
        "export" => Some(format!("browse rune page-skills/{slug}/mech.rn")),
        _ => None,
    }
}

/// Unix 毫秒钟（同 engine 侧面形）：取不到回 0（时间锚缺失比崩溃面轻）。
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `POST /events`（#77）：油猴脚本经 `GM_xmlhttpRequest` 直达 daemon
/// 的事件上报入口（与 `GET /events` 同径对称——集合资源的 create；
/// GM 面绕 CORS，页面 `fetch` 直调会被 CORS 拦——脚本侧用 GM 面是文
/// 档口径）。daemon 绑定 127.0.0.1（缺省）即天然只收本机上报。回
/// `{ok, seq}`。
async fn events_create_handler(
    State(st): State<AppState>,
    Json(req): Json<EventCreateRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    let (r#type, slug) = match normalize_event_kind(&req) {
        Ok(v) => v,
        Err(e) => return err_response(e),
    };
    if r#type.is_empty() {
        return err_response(anyhow::anyhow!(
            "events 的 name 不能为空串；下一步：标准形 {{\"type\":\"see\",\"slug\":\"...\"}}"
        ));
    }
    let mut ring = st
        .daemon
        .events_ring
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match ring.push(
        r#type,
        slug,
        req.url,
        req.data,
        req.action.as_deref(),
        now_ms(),
    ) {
        Some(seq) => (
            axum::http::StatusCode::OK,
            Json(json!({ "ok": true, "seq": seq })),
        ),
        // P1 服务端下限命中:事件不入环（2 秒同键重复）,回 throttled 而非错
        None => (
            axum::http::StatusCode::OK,
            Json(json!({ "ok": true, "throttled": true })),
        ),
    }
}

/// `GET /events` 查询参（#77 消费面）：`since` 序号游标与 `last`
/// 条数窗二选一（都给以 `since` 优先），全缺省取最近 20 条。手解
/// query 串（axum 裁了 default-features，`Query` 提取器不在；两个整
/// 数参不需要百分号解码）。
fn parse_events_query(q: &str) -> (Option<u64>, Option<usize>) {
    let mut since = None;
    let mut last = None;
    for kv in q.split('&') {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        match k {
            "since" => since = v.parse().ok(),
            "last" => last = v.parse().ok(),
            _ => {}
        }
    }
    (since, last)
}

/// `GET /events`（#77）：事件消费面，`browse events` 同源。回
/// `{ok, events: [...], nextSeq}`（nextSeq 作下一轮 `since` 游标）。
async fn events_handler(
    State(st): State<AppState>,
    uri: axum::http::Uri,
) -> (axum::http::StatusCode, Json<Value>) {
    let (since, last) = parse_events_query(uri.query().unwrap_or_default());
    let ring = st
        .daemon
        .events_ring
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let events = match since {
        Some(s) => ring.since(s),
        None => ring.last(last.unwrap_or(20)),
    };
    let next = ring.next_seq();
    // P1 `next`:全环按冻结优先序(CTA_PRIORITY,安全类先于爬虫类)取
    // 最高优先的未消费建议(带合法 cta 的 see);同级取最新。agent 可
    // 忽略它只看 events,也可只跟 next——挑战与采集并存时先看挑战。
    let next_cta = ring.next_cta();
    (
        axum::http::StatusCode::OK,
        Json(json!({ "ok": true, "events": events, "nextSeq": next, "next": next_cta })),
    )
}

/// `DELETE /events`（#77）：清环（`browse events --clear` 同源）；序号
/// 不回卷，消费游标不失位。
async fn events_clear_handler(State(st): State<AppState>) -> impl IntoResponse {
    st.daemon
        .events_ring
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    Json(json!({ "ok": true }))
}

/// `POST /userscript/stop`（#77 stop api，用户令「油猴 js 检测到什么发送
/// 请求到这个 api 就停止进程输出什么」；标准形端点名，旧 `/userscript/stop` 已
/// 弃）：页内脚本条件成立即打本端点，daemon 打断当前正在跑的那次 rune
/// （`mech.rn` 编排），被停回执带 `stopped.capture`（脚本捕获的 what 与
/// 停在宿主哪一步）。与 `/rune/stop` 的差异：有人给的停止因与捕获，走
/// 的取消道相同。不停浏览器、不关 daemon、不清事件环；无在跑 rune 时
/// 入环并回 `stopped:false`（不落捕获槽，防陈货串下一次执行）。不拿
/// `eval_lock`。body 标准形 `{slug, url, data}`（type 恒 stop 由端点语
/// 义内定），兼容旧 `name`（`stop.<slug>` 前缀推导）。
async fn userscript_stop_handler(
    State(st): State<AppState>,
    Json(mut req): Json<EventCreateRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    req.r#type = Some("stop".into());
    let (r#type, slug) = match normalize_event_kind(&req) {
        Ok(v) => v,
        Err(e) => return err_response(e),
    };
    // 先入环（副本，browse events 可见同 seq;stop 不设下限恒入环）
    let seq = {
        let mut ring = st
            .daemon
            .events_ring
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let ns = ring.next_seq();
        ring.push(r#type, slug.clone(), req.url, req.data, None, now_ms())
            .unwrap_or(ns)
    };
    let d = &st.daemon;
    let running = d.rune_running.load(Ordering::Relaxed);
    let capture = json!({ "seq": seq, "type": "stop", "slug": slug });
    let tx = d
        .rune_cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let requested = tx.is_some_and(|t| t.send(true).is_ok());
    if running && requested {
        *d.stop_capture.lock().unwrap_or_else(|e| e.into_inner()) = Some(capture);
        (
            axum::http::StatusCode::OK,
            Json(json!({ "ok": true, "stopped": true, "seq": seq })),
        )
    } else {
        (
            axum::http::StatusCode::OK,
            Json(json!({
                "ok": true, "stopped": false, "seq": seq,
                "reason": if running { "eval_busy" } else { "idle" },
                "note": "无在跑 rune 执行（事件已入环；capture 不落槽）"
            })),
        )
    }
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

    /// #77 事件环：序号单调、since/last 窗、容量截断、data 超限截断、
    /// clear 不回卷序号。
    #[test]
    fn event_ring_semantics() {
        let mut ring = EventRing::default();
        for i in 0..5u64 {
            assert_eq!(
                ring.push(
                    "see".into(),
                    Some(format!("e{i}")),
                    None,
                    None,
                    None,
                    1_700_000_000_000 + i
                ),
                Some(i)
            );
        }
        assert_eq!(ring.next_seq(), 5);
        let since2 = ring.since(2);
        assert_eq!(
            since2.iter().map(|e| e.seq).collect::<Vec<_>>(),
            vec![3, 4],
            "since 严格大于"
        );
        assert_eq!(ring.last(2).len(), 2, "last 窗");
        assert_eq!(ring.last(0).len(), 5, "last 0 = 全部");
        // data 超限截断成 __truncated 标记
        let big = Value::String("x".repeat(40 * 1024));
        ring.push("see".into(), Some("big".into()), None, Some(big), None, 0);
        let big_ev = ring.last(0).pop().expect("big 在环尾");
        let d = big_ev.data.expect("截断标记仍在");
        assert_eq!(d["__truncated"], json!(true), "{d}");
        assert!(
            d["bytes"].as_u64().unwrap_or_default() > EVENT_DATA_CAP as u64,
            "{d}"
        );
        // 容量截断：灌满翻倍，只留尾部
        for i in 0..(EVENT_RING_CAP as u64 * 2) {
            ring.push(
                "see".into(),
                Some(format!("f{i}")),
                Some("http://x/".into()),
                None,
                None,
                0,
            );
        }
        assert_eq!(ring.last(0).len(), EVENT_RING_CAP);
        // P0 收口:cta 只出自 (冻结 slug, 动作枚举) 模板,自由串不进环
        let mut r2 = EventRing::default();
        let mut see = |slug: &str, action: Option<&str>| {
            r2.push("see".into(), Some(slug.into()), None, None, action, 0)
        };
        see("challenge-stop", Some("rune"));
        see("challenge-stop", Some("read"));
        see("response-tap", Some("export"));
        see("challenge-stop", Some("rm -rf /")); // 非法枚举:事件保留 cta 无
        see("pwn-slug", Some("rune")); // 名单外 slug:同上
        see("challenge-stop", None);
        let all = r2.last(0);
        let cta_of = |i: usize| all[i].cta.clone();
        assert_eq!(
            cta_of(0),
            Some("browse rune page-skills/challenge-stop/mech.rn".into()),
            "rune 模板"
        );
        assert_eq!(
            cta_of(1),
            Some("browse workspace page challenge-stop".into()),
            "read 模板"
        );
        assert_eq!(
            cta_of(2),
            Some("browse rune page-skills/response-tap/mech.rn".into()),
            "export 模板"
        );
        assert_eq!(cta_of(3), None, "非法动作不生成 cta");
        assert_eq!(cta_of(4), None, "名单外 slug 不生成 cta");
        assert_eq!(cta_of(5), None, "无 action 无 cta");
        // stop 恒不带 cta
        r2.push(
            "stop".into(),
            Some("challenge-stop".into()),
            None,
            None,
            Some("rune"),
            0,
        );
        let tail = r2.last(0);
        assert_eq!(tail[tail.len() - 1].cta, None, "stop 不带 cta");

        // clear 不回卷
        let before = ring.next_seq();
        ring.clear();
        assert!(ring.last(0).is_empty());
        assert_eq!(ring.next_seq(), before);
    }

    /// P1 三件:服务端下限(2 秒同键拒收)、per-slug 封顶(stop 免逐)、
    /// next_cta 冻结优先序(安全类先于爬虫类,同级取最新)。
    #[test]
    fn event_ring_p1_floor_caps_next() {
        let mut r = EventRing::default();
        // 下限:同 (slug,action) 2 秒内第二条拒收
        let t = 1_700_000_000_000u64;
        assert_eq!(
            r.push(
                "see".into(),
                Some("challenge-stop".into()),
                None,
                None,
                Some("rune"),
                t
            ),
            Some(0)
        );
        assert_eq!(
            r.push(
                "see".into(),
                Some("challenge-stop".into()),
                None,
                None,
                Some("rune"),
                t + 1_000
            ),
            None,
            "2 秒内同键拒收"
        );
        assert_eq!(
            r.push(
                "see".into(),
                Some("challenge-stop".into()),
                None,
                None,
                Some("rune"),
                t + 2_001
            ),
            Some(1),
            "过窗再收"
        );
        // 不同 action 不同键,不撞
        assert!(
            r.push(
                "see".into(),
                Some("challenge-stop".into()),
                None,
                None,
                Some("read"),
                t + 1_000
            )
            .is_some()
        );
        // stop 不设下限
        assert!(
            r.push(
                "stop".into(),
                Some("challenge-stop".into()),
                None,
                None,
                Some("rune"),
                t + 1_000
            )
            .is_some()
        );
        // per-slug 封顶:同 slug 灌 25 条 see(ts 各隔 3 秒防下限),只留 20
        let mut r2 = EventRing::default();
        for i in 0..25u64 {
            r2.push(
                "see".into(),
                Some("response-tap".into()),
                None,
                None,
                Some("export"),
                t + i * 3_000,
            );
        }
        assert_eq!(r2.last(0).len(), EVENT_PER_SLUG_CAP, "per-slug 帽生效");
        // stop 免逐:先塞 stop 再灌满,stop 还在
        let mut r3 = EventRing::default();
        r3.push(
            "stop".into(),
            Some("challenge-stop".into()),
            None,
            None,
            None,
            t,
        );
        for i in 0..25u64 {
            r3.push(
                "see".into(),
                Some("response-tap".into()),
                None,
                None,
                Some("export"),
                t + i * 3_000,
            );
        }
        assert!(
            r3.last(0).iter().any(|e| e.r#type == "stop"),
            "stop 不被 see 挤掉"
        );
        // next_cta:安全类(challenge-stop)压爬虫类(response-tap);同级取最新
        let mut r4 = EventRing::default();
        r4.push(
            "see".into(),
            Some("response-tap".into()),
            None,
            None,
            Some("export"),
            t,
        );
        r4.push(
            "see".into(),
            Some("challenge-stop".into()),
            None,
            None,
            Some("rune"),
            t + 3_000,
        );
        r4.push(
            "see".into(),
            Some("antibot-vendor".into()),
            None,
            None,
            Some("read"),
            t + 6_000,
        );
        let n = r4.next_cta().expect("有建议");
        assert_eq!(n.slug.as_deref(), Some("challenge-stop"), "安全类优先");
        let mut r5 = EventRing::default();
        r5.push(
            "see".into(),
            Some("challenge-stop".into()),
            None,
            None,
            Some("rune"),
            t,
        );
        r5.push(
            "see".into(),
            Some("challenge-stop".into()),
            None,
            None,
            Some("rune"),
            t + 3_000,
        );
        assert_eq!(r5.next_cta().unwrap().seq, 1, "同级取最新");
        // 无建议:cta 为 None 的(非法 action)不入选
        let mut r6 = EventRing::default();
        r6.push(
            "see".into(),
            Some("challenge-stop".into()),
            None,
            None,
            Some("bogus"),
            t,
        );
        assert!(r6.next_cta().is_none(), "无合法 cta 时 next 为空");
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
        let (code, Json(v)) = rune_stopped_response(json!("goto"), 1234, "stop_request", None);
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
        let (_, Json(v)) = rune_stopped_response(json!("wait_event"), 5678, "engine_lost", None);
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
