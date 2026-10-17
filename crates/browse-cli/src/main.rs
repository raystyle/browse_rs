//! browse：给 agent 用的 browse CLI。
//!
//! 忠实移植 browser-harness-rs 的传输三形态（`--eval` / stdin / TTY REPL），
//! 外加常驻 daemon（首次使用自动拉起）与引擎生命周期子命令：
//!
//! ```text
//! browse 'await session.Page.navigate({url:"https://example.com"})'
//! browse up [--headless]     browse down     browse status
//! ```
//!
//! 引擎策略：附着优先（探测 9222 / DevToolsActivePort），缺则 spawn
//! clean-chrome 专属实例；`browse down` 只终结自己 spawn 的。

use anyhow::{Result, anyhow};
use browse_cli::client;
use browse_core::snippet_complete;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde_json::{Value, json};
use std::io::IsTerminal;
use std::path::PathBuf;

/// CLI 入口解析出的形态：缺省求值或生命周期子命令。
/// userscript enable/disable/remove 的多 id 收集（空格分隔批量）与旗标归
/// 集（`--restart` 可出现在任意 id 位；尾置全局旗标按全局道收（尾置合法
/// 批），其它旗标按用法错拒）。
fn collect_ids(g: &mut Globals, args: &mut Args, sub: &str) -> (Vec<String>, bool) {
    let mut ids = vec![args.next(&format!("userscript {sub} <id|名>"))];
    let mut restart = false;
    while let Some(t) = args.next_opt() {
        if t == "--restart" {
            restart = true;
        } else if t.starts_with("--") && !g.apply(&t, args) {
            eprintln!("browse: userscript {sub} 只收 id/名与 --restart（收到 {t}，退出 2）");
            std::process::exit(2)
        } else if !t.starts_with("--") {
            ids.push(t);
        }
    }
    (ids, restart)
}

/// userscript 族的目标 profile（#84 下腿）：显式 `--profile` > daemon 记录
/// spec 的 profile（运行/最近一次 up 形态——盘上装改与引擎读写同库，
/// 消 write-A-read-B）> 当前实例缺省 engine-profile。
async fn engine_profile_path(profile: &Option<String>) -> PathBuf {
    if let Some(p) = profile {
        return PathBuf::from(p);
    }
    if let Some(p) = client::daemon_spec_profile().await {
        return PathBuf::from(p);
    }
    browse_core::paths::engine_profile_dir()
}

/// #79 `--restart`：引擎级重启让盘上装改对引擎生效。#84 起 daemon 记录
/// spec 复起（`up --profile` 等形态不失，headed 也不被强转 headless）；
/// daemon 不在时拉起后按本参 up（无记录面可循）。
async fn restart_engine(profile: Option<String>) -> anyhow::Result<()> {
    if client::daemon_alive().await.is_some() {
        client::engine_restart().await?;
    } else {
        client::ensure_daemon().await?;
        client::engine_up(client::UpParams {
            headless: true,
            chrome: None,
            ws: None,
            port: None,
            pipe: false,
            profile,
            proxy: None,
            proxy_bypass: None,
            isolated: false,
            engine_args: vec![],
        })
        .await?;
    }
    println!("引擎已重启（装改生效）");
    Ok(())
}

enum Mode {
    /// 求值片段（缺省）。
    Eval,
    /// `browse up`：显式起引擎。
    Up,
    /// `browse down`：退 daemon（只杀自起引擎）。
    Down,
    /// `browse status`：看 daemon/引擎状态。
    Status,
    /// `browse events [--since <seq>] [--last <n>] [--clear]`：消费油猴事件
    /// 回调（#77；userscript 经 GM_xmlhttpRequest POST /events 上报，type 两档 see/stop）。
    Events {
        /// 序号游标（严格大于）。
        since: Option<u64>,
        /// 最近 N 条（0 = 全部）。
        last: Option<usize>,
        /// 先清环再回执。
        clear: bool,
    },
    /// `browse update`：browse 自更新（发现加锚校验加自替换；ark 管理拦）。
    Update,
    /// `browse chrome install <版本> [部署目录]`：镜像下载或本地导入安装 Chromium 版本。
    ChromeInstall {
        version: String,
        from_dir: Option<String>,
    },
    /// `browse chrome list`：列已装版本与 pin。
    ChromeList,
    /// `browse chrome use <版本>`：pin 切换。
    ChromeUse(String),
    /// `browse chrome update`：发现最新版，未装则镜像装，pin 切最新。
    ChromeUpdate,
    /// `browse chrome remove <版本>`：删已装版本（pin 指向的拒删）。
    ChromeRemove(String),
    /// `browse chrome doctor`：托管部署体检。
    ChromeDoctor,
    /// `browse issue new <标题> --acceptance <验收> [--kind bug|improvement] [--body <正文>] [--dry-run]`：账本 issue 流开单（REQ-063，真源 ledger.ohmygh.com）。
    IssueNew {
        title: String,
        body: String,
        kind: String,
        acceptance: String,
        dry: bool,
    },
    /// `browse issue list [--limit <n>] [--before <id>]`：列 issue（账本面；count 是返回条数，has_more 权威）。
    IssueList { limit: u32, before: Option<u64> },
    /// `browse issue show <id>`：看 issue 详情。
    IssueShow(String),
    /// `browse workspace status [--json]`：workspace 仓与 git 状态概览（#50/#51 配套）。
    WorkspaceStatus,
    /// `browse workspace install`：git clone 种子仓到仓根。
    WorkspaceInstall,
    /// `browse workspace update`：git pull --ff-only（脏树拒绝）。
    WorkspaceUpdate,
    /// `browse workspace list`：列在册域名段与 page slug。
    WorkspaceList,
    /// `browse userscript install <file...>`：油猴脚本盘上装机（幂等重装
    /// enabled 保留；引擎重启生效，`--restart` 自动 down/up；REQ #79）。
    UserscriptInstall(Vec<String>, bool),
    /// `browse userscript list`：列在册脚本（index 对读+workspace 对册）。
    UserscriptList,
    /// `browse userscript enable/disable <id|名>... [--restart]`（多脚本批量，
    /// 逐条定位；`--restart` 生效引擎）。
    UserscriptSetEnabled(Vec<String>, bool, bool),
    /// `browse userscript remove <id|名>...`（多脚本批量；`--restart` 同 install）。
    UserscriptRemove(Vec<String>, bool),
    /// `browse userscript update <id|名> <file>`（同 id 换源）。
    UserscriptUpdate(String, String, bool),
    /// `browse userscript path <id|名>`：输出脚本体路径（编辑入口）。
    UserscriptPath(String),
    /// `browse userscript eval <code>`：油猴世界直通求值（#81，GM 面可用）。
    UserscriptEval(String),
    /// `browse cdp <Method> [--params JSON]`：任意 CDP 域方法直通（#80）。
    CdpCall(String, Option<String>),
    /// `browse cdpMethods [Domain]`：列出原生 CDP 方法，不启动引擎。
    CdpMethods(Option<String>),
    /// `browse workspace site <段>[/<文件>]`：域技能清单或全文。
    WorkspaceSite(String),
    /// `browse workspace page <slug>`：页面技能全文。
    WorkspacePage(String),
    /// `browse workspace add <path>`：登记自定义技能仓（#63，多根序首优先）。
    WorkspaceAdd(String),
    /// `browse workspace remove <path>`：移除登记的自定义仓（#63）。
    WorkspaceRemove(String),
    /// `browse fetch <url> [--html] [--markdown|-m] [--timeout <s>]`：一次性只读抓取（#50；渲染 DOM 与 markdown 形 REQ-028）。
    Fetch {
        url: String,
        dom_html: bool,
        markdown: bool,
        timeout_s: u64,
    },
    /// `browse issue close` 面已移除（总台修正令 2026-09-20 收口：关闭与删除唯一道 = omc 工位经 herdr 委托）。
    /// `browse artifact publish --name <n> --kind <k> --digest <d>`：产物共享库发布（REQ-063；--dep 可重复记依赖出处；--summary/--outcome/--git-sha 结构化字段网页详情直出，参数面随 ledger-client v0.1.3）。
    ArtifactPublish {
        name: String,
        kind: String,
        digest: String,
        version: Option<String>,
        git_range: Option<String>,
        note: Option<String>,
        summary: Option<String>,
        outcome: Option<String>,
        git_sha: Option<String>,
        deps: Vec<String>,
    },
    /// `browse artifact attest <id> --type <t>`：产物验证事件（attest_dev/attest_prod/verification_failed 三型；promote/demote/supersede 归 omc）。
    ArtifactAttest {
        id: String,
        ev_type: String,
        note: Option<String>,
    },
    /// `browse artifact list [--current] [--env dev|prod]`：产物列表。
    ArtifactList {
        current: bool,
        env_f: Option<String>,
    },
    /// `browse ledger keygen [--force]`：Ed25519 密钥对（REQ-063）。
    LedgerKeygen { force: bool },
    /// `browse rune <script.rn|@名> [--vars k=v] [--root <dir>]`：Rune 编排面（#67）。
    Rune { script: Option<String> },
    /// `browse rune stop`：主动停止在跑的 rune 执行（批次二总台指令；
    /// 名是动词位，脚本文件别叫 stop）。
    RuneStop,
}

/// 账本调用统一走阻塞线程（收口适配 2026-09-20）：ledger-client 是
/// reqwest blocking 客户端，在 async 上下文里构造/销毁会 panic（嵌套
/// runtime 的 drop 限制，实弹红）；spawn_blocking 让它整个生命周期留在
/// 阻塞线程池。
async fn ledger_call<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> std::result::Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| anyhow!("ledger 线程故障：{e}"))?
        .map_err(|e| anyhow!("{e}"))
}

/// 全局旗标态（尾置合法批，R4 上游缺陷 b 修复）：--llms 手册读序句
/// 「全局旗标尾置合法（无关子命令静默忽略）」的实装载体——旗标态从
/// main 局部变量收拢成结构，主环与子命令收集环共用 [`Globals::apply`]
/// 一道解析，尾置与前置同语义。
#[derive(Default)]
struct Globals {
    /// 求值片段（`--eval` 收集；子命令形态下收集不消费，静默忽略）。
    snippets: Vec<String>,
    /// `--serve` 前台 daemon。
    serve: bool,
    /// `--bind` 监听位。
    bind: Option<String>,
    /// `--connect` 显式连接意图。
    connect: Option<String>,
    /// `--new-tab` 开新标签再求值。
    new_tab: bool,
    /// `--ws` 钉死 WS URL。
    ws: Option<String>,
    /// `--port` 附着端口。
    port: Option<u16>,
    /// `--chrome` 引擎二进制。
    chrome: Option<String>,
    /// `--profile` 引擎 user-data-dir。
    profile: Option<String>,
    /// `--headless` 无头。
    headless: bool,
    /// `--pipe` 管道调试通道。
    pipe: bool,
    /// `--json` 机器形输出。
    json: bool,
    /// `--js` 片段按 JS 求值。
    js: bool,
    /// `--b64` 片段 base64 解码。
    b64: bool,
    /// `--llms` 手册面。
    llms: bool,
    /// `--full` 人读完整目录。
    full: bool,
    /// `--repl` 交互面。
    repl: bool,
    /// `--proxy` 引擎代理。
    proxy: Option<String>,
    /// `--proxy-bypass` 代理旁路。
    proxy_bypass: Option<String>,
    /// `--engine-arg` 引擎附加旗标（可叠加）。
    engine_args: Vec<String>,
    /// `--isolated` 临时隔离 profile。
    isolated: bool,
    /// `--idle-timeout` daemon 空闲回收毫秒。
    idle_timeout: Option<String>,
    /// `--cookies` 域 cookie 热迁。
    cookies_csv: Option<String>,
    /// 统一兜底硬界秒（REQ-027，默认 3 分钟由 daemon 缺省；请求级覆写）。
    cli_timeout_s: Option<u64>,
    /// `--secrets` 密钥仓。
    secrets: Option<String>,
    /// #66 命令小程序：--vars 键=值 可重复累积；snippets 根旗标（eval 面
    /// 模板道与 rune js_lib 共用一根，旗标 > workspace scripts）。
    vars: Vec<String>,
    /// `--root` 片段库根。
    snippets_root_flag: Option<String>,
    /// 0.30.3 模板定界（缺省 ERB）：--vars-syntax classic|erb；件首行指令
    /// 优先（render 侧收口）。
    vars_syntax_flag: Option<String>,
    /// `--gen-surface` 命令面目录重生成。
    gen_surface: Option<String>,
}

impl Globals {
    /// 应用一个全局旗标（含取值）：认识即收进本结构返 true，未知旗标返
    /// false 不动游标（调用方走 bail 面）。主环与子命令收集环同一道——
    /// 尾置全局旗标与前置同语义；子命令自有旗标（fetch 的 --timeout、
    /// rune 的 --vars/--root/--timeout）由各自内环先收，优先于本道。
    fn apply(&mut self, a: &str, args: &mut Args) -> bool {
        match a {
            "--eval" | "-e" => self.snippets.push(args.next("--eval")),
            "--gen-surface" => {
                self.gen_surface = Some(args.next("--gen-surface"));
            }
            "--serve" => self.serve = true,
            "--bind" => self.bind = Some(args.next("--bind")),
            "--connect" => self.connect = Some(args.next("--connect")),
            "--new-tab" => self.new_tab = true,
            "--ws" => self.ws = Some(args.next("--ws")),
            "--port" => {
                self.port = match args.next("--port").parse() {
                    Ok(p) => Some(p),
                    // 用法错直出 exit 2（同 next 闭包口径，评审 G-F）
                    Err(_) => {
                        eprintln!("browse: --port 要数字（退出 2）");
                        std::process::exit(2)
                    }
                }
            }
            "--chrome" => self.chrome = Some(args.next("--chrome")),
            "--profile" => self.profile = Some(args.next("--profile")),
            "--headless" => self.headless = true,
            "--pipe" => self.pipe = true,
            "--proxy" => self.proxy = Some(args.next("--proxy")),
            "--proxy-bypass" => self.proxy_bypass = Some(args.next("--proxy-bypass")),
            // #48：引擎附加旗标直通 spawn（可叠加），值以 -- 开头也原样收
            "--engine-arg" => self.engine_args.push(args.next("--engine-arg")),
            "--isolated" => self.isolated = true,
            "--idle-timeout" => {
                // 正整数毫秒门禁（评审 G-17）：坏值不再静默回落缺省，
                // 「连错实例/回收不生效」难归因
                let raw = args.next("--idle-timeout");
                if raw.parse::<u64>().map_or(true, |v| v == 0) {
                    eprintln!("browse: --idle-timeout 要正整数毫秒（当前 {raw}；用法错，退出 2）");
                    std::process::exit(2);
                }
                self.idle_timeout = Some(raw);
            }
            // 统一兜底硬界（REQ-027）：执行道（rune 与 --js/方言求值）的
            // 请求级覆写秒，>=1000 按毫秒误写换算并告警；与 wait 族等待
            // 窗正交（等待窗是命中型参数，本界是到期即退的硬指标）
            "--timeout" => {
                let raw: u64 = args.next("--timeout").parse().unwrap_or_else(|_| {
                    eprintln!("browse: --timeout 要整秒数（退出 2）");
                    std::process::exit(2)
                });
                // secs_to_ms 回的是毫秒,请求字段是秒——换回秒再递(实弹
                // 抓过「3 变 3000 秒」的雷,sink 捕获在卷)
                let (ms, warn) = browse_core::js_host::secs_to_ms(raw);
                if let Some(w) = warn {
                    eprintln!("browse: --timeout {w}");
                }
                // 0 秒即「即刻超时」病态值（评审 G-17）：显式拒，不静默成
                // 每次调用必超时
                if ms == 0 {
                    eprintln!("browse: --timeout 要正秒数（0 即刻超时；用法错，退出 2）");
                    std::process::exit(2);
                }
                self.cli_timeout_s = Some(ms / 1000);
            }
            // #48：无头起引擎时从附着浏览器按域克隆 cookie（逗号分隔多域）
            "--cookies" => self.cookies_csv = Some(args.next("--cookies")),
            "--secrets" => self.secrets = Some(args.next("--secrets")),
            "--js" => self.js = true,
            // #66 模板参：只收重复旗标累积（与 --engine-arg 同族），不收
            // 逗号分隔叠法（k=v 值本身可含逗号，歧义）；形状校验在使用口
            "--vars" => self.vars.push(args.next("--vars")),
            // #65/#66 共用根旗标：eval 面模板道与 rune js_lib 同根
            "--root" => self.snippets_root_flag = Some(args.next("--root")),
            "--vars-syntax" => {
                self.vars_syntax_flag = Some(args.next("--vars-syntax"));
            }
            // base64 通道只留长参：`-b` 短参是 issue new 的 --body 既有
            // 契约（REQ-057），不夺权
            "--b64" => self.b64 = true,
            "--json" => self.json = true,
            "--llms" => self.llms = true,
            "--repl" => self.repl = true,
            "--full" => self.full = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("browse {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => return false,
        }
        true
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut mode = Mode::Eval;
    let mut g = Globals::default();
    let mut args = Args(std::env::args().skip(1));
    while let Some(a) = args.next_opt() {
        if g.apply(&a, &mut args) {
            continue;
        }
        match a.as_str() {
            "up" if g.snippets.is_empty() && mode_is_eval(&mode) => mode = Mode::Up,
            "down" if g.snippets.is_empty() && mode_is_eval(&mode) => mode = Mode::Down,
            "status" if g.snippets.is_empty() && mode_is_eval(&mode) => mode = Mode::Status,
            "events" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                let mut since = None;
                let mut last = None;
                let mut clear = false;
                while let Some(f) = args.next_opt() {
                    match f.as_str() {
                        "--since" => {
                            since = Some(args.next("--since <seq>").parse().unwrap_or_else(|_| {
                                eprintln!("browse: --since 要序号整数（退出 2）");
                                std::process::exit(2);
                            }));
                        }
                        "--last" => {
                            last = Some(args.next("--last <n>").parse().unwrap_or_else(|_| {
                                eprintln!("browse: --last 要条数整数（退出 2）");
                                std::process::exit(2);
                            }));
                        }
                        "--clear" => clear = true,
                        // 尾置全局旗标（R4 上游缺陷 b 修复）：先试全局道
                        // （含取值），未知才走 bail
                        other2 => {
                            if !g.apply(other2, &mut args) {
                                bail_arg(other2);
                            }
                        }
                    }
                }
                mode = Mode::Events { since, last, clear };
            }
            "update" if g.snippets.is_empty() && mode_is_eval(&mode) => mode = Mode::Update,
            "chrome" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                match args.next("chrome").as_str() {
                    "list" => mode = Mode::ChromeList,
                    "doctor" => mode = Mode::ChromeDoctor,
                    "use" => mode = Mode::ChromeUse(args.next("chrome use")),
                    "remove" => mode = Mode::ChromeRemove(args.next("chrome remove <版本>")),
                    "update" => mode = Mode::ChromeUpdate,
                    "install" => {
                        let version = args.next("chrome install <版本>");
                        // 可选位不吃旗标（评审 G-15）；尾置全局旗标按全局
                        // 道收（尾置合法批），未知旗标 bail，第二位置位显式报
                        let mut from_dir: Option<String> = None;
                        while let Some(t) = args.next_opt() {
                            if t.starts_with('-') {
                                if !g.apply(&t, &mut args) {
                                    bail_arg(&t);
                                }
                            } else if from_dir.is_none() {
                                from_dir = Some(t);
                            } else {
                                eprintln!(
                                    "browse: chrome install 只要一个部署目录位（{t} 多余，退出 2）"
                                );
                                std::process::exit(2);
                            }
                        }
                        mode = Mode::ChromeInstall { version, from_dir };
                    }
                    other => {
                        eprintln!(
                            "browse: chrome 子命令不认识 {other}（install/use/update/remove/list/doctor，退出 2）"
                        );
                        std::process::exit(2);
                    }
                }
            }
            "fetch" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                let url = args.next("fetch <url>");
                // REQ-028：--html 渲染 DOM 形（引擎腿直取 outerHTML）；
                // --markdown 从 v1 的 no-op 升 mdka 真转换
                let mut dom_html = false;
                let mut markdown = false;
                let mut timeout_s = 15u64;
                while let Some(f) = args.next_opt() {
                    match f.as_str() {
                        "--markdown" | "-m" => markdown = true,
                        "--html" => dom_html = true,
                        "--timeout" => {
                            // 统一守卫（REQ-027）：>=1000 按毫秒误写换算
                            // 并告警（与全局 --timeout 同一口径；换算回毫秒
                            // 后除千回秒,本变量是秒形）
                            let raw: u64 = args.next("--timeout").parse().unwrap_or_else(|_| {
                                eprintln!("browse: --timeout 要整秒数（退出 2）");
                                std::process::exit(2);
                            });
                            let (ms, warn) = browse_core::js_host::secs_to_ms(raw);
                            if let Some(w) = warn {
                                eprintln!("browse: --timeout {w}");
                            }
                            // 0 秒病态值显式拒（评审 G-17，与全局 --timeout 同律）
                            if ms == 0 {
                                eprintln!(
                                    "browse: --timeout 要正秒数（0 即刻超时；用法错，退出 2）"
                                );
                                std::process::exit(2);
                            }
                            timeout_s = ms / 1000;
                        }
                        // 尾置全局旗标（R4 上游缺陷 b 修复）：先试全局道，
                        // 未知才 bail
                        other2 => {
                            if !g.apply(other2, &mut args) {
                                bail_arg(other2);
                            }
                        }
                    }
                }
                mode = Mode::Fetch {
                    url,
                    dom_html,
                    markdown,
                    timeout_s,
                };
            }
            "artifact" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                match args.next("artifact").as_str() {
                    "publish" => {
                        // kind 必填（REQ-063 十五枚举无缺省）；--dep 可重复
                        //（deps[] 一等公民，回溯链即证据链）；参数面随标准
                        // crate ledger-client 收口（总台修正令 2026-09-20）
                        let mut name = String::new();
                        let mut kind = String::new();
                        let mut digest = String::new();
                        let mut version = None;
                        let mut git_range = None;
                        let mut note = None;
                        let mut summary = None;
                        let mut outcome = None;
                        let mut git_sha = None;
                        let mut deps: Vec<String> = Vec::new();
                        loop {
                            match args.next_opt() {
                                Some(f) if f == "--name" => name = args.next("--name"),
                                Some(f) if f == "--kind" => kind = args.next("--kind"),
                                Some(f) if f == "--digest" => digest = args.next("--digest"),
                                Some(f) if f == "--version" => {
                                    version = Some(args.next("--version"))
                                }
                                Some(f) if f == "--git-range" => {
                                    git_range = Some(args.next("--git-range"))
                                }
                                Some(f) if f == "--note" => note = Some(args.next("--note")),
                                Some(f) if f == "--dep" => deps.push(args.next("--dep")),
                                // 结构化字段（ledger-client v0.1.3 面）：summary
                                // 一行技术摘要与 outcome 结果倾向，网页详情直出
                                Some(f) if f == "--summary" => {
                                    summary = Some(args.next("--summary"))
                                }
                                Some(f) if f == "--outcome" => {
                                    outcome = Some(args.next("--outcome"))
                                }
                                Some(f) if f == "--git-sha" => {
                                    git_sha = Some(args.next("--git-sha"))
                                }
                                Some(f) => {
                                    if !g.apply(&f, &mut args) {
                                        bail_arg(&f);
                                    }
                                }
                                None => break,
                            }
                        }
                        if kind.is_empty() {
                            eprintln!(
                                "browse: --kind 必填（十五枚举：binary/experience/lesson/research/prototype/…，退出 2）"
                            );
                            std::process::exit(2);
                        }
                        if let Some(o) = outcome.as_deref()
                            && !matches!(o, "success" | "failure")
                        {
                            eprintln!(
                                "browse: --outcome 只认 success|failure，得 {o}（用法错，退出 2）"
                            );
                            std::process::exit(2);
                        }
                        // 自由字段本地预检（评审 G1，与 digest/kind/name 同款
                        // 免配额损耗）：summary 非空限长，git_sha 十六进制形
                        if let Some(s) = summary.as_deref()
                            && (s.trim().is_empty() || s.chars().count() > 1000)
                        {
                            eprintln!("browse: --summary 要非空且至多 1000 字符（用法错，退出 2）");
                            std::process::exit(2);
                        }
                        if let Some(g) = git_sha.as_deref()
                            && (!(7..=40).contains(&g.len())
                                || !g.chars().all(|c| c.is_ascii_hexdigit()))
                        {
                            eprintln!("browse: --git-sha 要 7 至 40 位十六进制（用法错，退出 2）");
                            std::process::exit(2);
                        }
                        mode = Mode::ArtifactPublish {
                            name,
                            kind,
                            digest,
                            version,
                            git_range,
                            note,
                            summary,
                            outcome,
                            git_sha,
                            deps,
                        };
                    }
                    "attest" => {
                        let id = args.next("artifact attest <id>");
                        let mut ev_type = String::from("attest_dev");
                        let mut note = None;
                        loop {
                            match args.next_opt() {
                                Some(f) if f == "--type" => ev_type = args.next("--type"),
                                Some(f) if f == "--note" => note = Some(args.next("--note")),
                                Some(f) => {
                                    if !g.apply(&f, &mut args) {
                                        bail_arg(&f);
                                    }
                                }
                                None => break,
                            }
                        }
                        mode = Mode::ArtifactAttest { id, ev_type, note };
                    }
                    "list" => {
                        let mut current = false;
                        let mut env_f = None;
                        loop {
                            match args.next_opt() {
                                Some(f) if f == "--current" => current = true,
                                Some(f) if f == "--env" => env_f = Some(args.next("--env")),
                                Some(f) => {
                                    if !g.apply(&f, &mut args) {
                                        bail_arg(&f);
                                    }
                                }
                                None => break,
                            }
                        }
                        mode = Mode::ArtifactList { current, env_f };
                    }
                    other => {
                        eprintln!(
                            "browse: artifact 子命令不认识 {other}（publish/attest/list；promote/demote 面已收口归 omc 工位，退出 2）"
                        );
                        std::process::exit(2);
                    }
                }
            }
            "ledger" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                match args.next("ledger").as_str() {
                    "keygen" => {
                        let mut force = false;
                        while let Some(f) = args.next_opt() {
                            match f.as_str() {
                                "--force" => force = true,
                                other2 => {
                                    if !g.apply(other2, &mut args) {
                                        bail_arg(other2);
                                    }
                                }
                            }
                        }
                        mode = Mode::LedgerKeygen { force };
                    }
                    other => {
                        eprintln!("browse: ledger 子命令不认识 {other}（keygen，退出 2）");
                        std::process::exit(2);
                    }
                }
            }
            "rune" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                // 评审 #67 一轮 G4：对齐 snippets 族口径——未知旗标 bail、
                // 多余位置位用法错；--vars/--root 在脚本位前后都收（内环
                // 代收进同一变量，余下未知 - 前缀旗标 exit 2 不静默）
                let mut script = None;
                while let Some(a) = args.next_opt() {
                    match a.as_str() {
                        "--vars" => g.vars.push(args.next("--vars")),
                        "--root" => {
                            g.snippets_root_flag = Some(args.next("--root"));
                        }
                        // 统一兜底硬界（REQ-027 评审 F-2）：rune 内环代收，
                        // 尾置与前置同效（与 --vars/--root 同律，覆盖已收的
                        // 前置值）
                        "--timeout" => {
                            let raw: u64 = args.next("--timeout").parse().unwrap_or_else(|_| {
                                eprintln!("browse: --timeout 要整秒数（退出 2）");
                                std::process::exit(2)
                            });
                            let (ms, warn) = browse_core::js_host::secs_to_ms(raw);
                            if let Some(w) = warn {
                                eprintln!("browse: --timeout {w}");
                            }
                            g.cli_timeout_s = Some(ms / 1000);
                        }
                        // 尾置全局旗标（R4 上游缺陷 b 修复）：先试全局道
                        // （含取值），未知才 bail
                        other if other.starts_with('-') => {
                            if !g.apply(other, &mut args) {
                                bail_arg(other);
                            }
                        }
                        other if script.is_none() => script = Some(other.to_string()),
                        other => {
                            eprintln!("browse: rune 只要一个脚本位（{other} 多余，退出 2）");
                            std::process::exit(2);
                        }
                    }
                }
                // stop 动词位（批次二）：首位置位恰为 stop 即停止道
                if script.as_deref() == Some("stop") && g.vars.is_empty() {
                    mode = Mode::RuneStop;
                } else {
                    mode = Mode::Rune { script };
                }
            }
            "workspace" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                match args.next("workspace").as_str() {
                    "status" => mode = Mode::WorkspaceStatus,
                    "install" => mode = Mode::WorkspaceInstall,
                    "update" => mode = Mode::WorkspaceUpdate,
                    "list" => mode = Mode::WorkspaceList,
                    "site" => mode = Mode::WorkspaceSite(args.next_opt().unwrap_or_default()),
                    "page" => mode = Mode::WorkspacePage(args.next_opt().unwrap_or_default()),
                    "add" if mode_is_eval(&mode) && g.snippets.is_empty() => {
                        mode = Mode::WorkspaceAdd(args.next("workspace add 的路径"))
                    }
                    "remove" if mode_is_eval(&mode) && g.snippets.is_empty() => {
                        mode = Mode::WorkspaceRemove(args.next("workspace remove 的路径"))
                    }
                    other => {
                        eprintln!(
                            "browse: workspace 子命令不认识 {other}（status/install/update/list/site/page，退出 2）"
                        );
                        std::process::exit(2);
                    }
                }
            }
            "userscript" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                match args.next("userscript").as_str() {
                    "install" => {
                        let mut files = Vec::new();
                        let mut restart = false;
                        loop {
                            match args.next_opt() {
                                None => break,
                                Some(t) if t == "--restart" => restart = true,
                                // 尾置全局旗标（R4 上游缺陷 b 修复）：先试
                                // 全局道，未知才按 install 面报
                                Some(t) if t.starts_with("--") => {
                                    if !g.apply(&t, &mut args) {
                                        eprintln!(
                                            "browse: userscript install 只认 --restart（收到 {t}，退出 2）"
                                        );
                                        std::process::exit(2)
                                    }
                                }
                                Some(t) => files.push(t),
                            }
                        }
                        if files.is_empty() {
                            eprintln!(
                                "browse: userscript install 至少一个 .user.js 路径（退出 2）"
                            );
                            std::process::exit(2)
                        }
                        mode = Mode::UserscriptInstall(files, restart)
                    }
                    "list" => mode = Mode::UserscriptList,
                    "enable" => {
                        let (ids, restart) = collect_ids(&mut g, &mut args, "enable");
                        mode = Mode::UserscriptSetEnabled(ids, true, restart)
                    }
                    "disable" => {
                        let (ids, restart) = collect_ids(&mut g, &mut args, "disable");
                        mode = Mode::UserscriptSetEnabled(ids, false, restart)
                    }
                    "remove" => {
                        let (ids, restart) = collect_ids(&mut g, &mut args, "remove");
                        mode = Mode::UserscriptRemove(ids, restart)
                    }
                    "update" => {
                        // 位置实参依次收:第一个=id/名,第二个=源文件;--restart 任意位
                        let mut pos: Vec<String> = Vec::new();
                        let mut restart = false;
                        while let Some(t) = args.next_opt() {
                            if t == "--restart" {
                                restart = true;
                            } else if t.starts_with("--") {
                                // 尾置全局旗标（R4 上游缺陷 b 修复）：先试
                                // 全局道，未知才按 update 面报
                                if !g.apply(&t, &mut args) {
                                    eprintln!(
                                        "browse: userscript update 只认 --restart（收到 {t}，退出 2）"
                                    );
                                    std::process::exit(2)
                                }
                            } else {
                                pos.push(t);
                            }
                        }
                        if pos.len() < 2 {
                            eprintln!("browse: userscript update <id|名> <file>（退出 2）");
                            std::process::exit(2)
                        }
                        mode = Mode::UserscriptUpdate(pos[0].clone(), pos[1].clone(), restart)
                    }
                    "path" => mode = Mode::UserscriptPath(args.next("userscript path <id|名>")),
                    "eval" => mode = Mode::UserscriptEval(args.next("userscript eval <code>")),
                    other => {
                        eprintln!(
                            "browse: userscript 子命令不认识 {other}（install/list/enable/disable/remove/update/path/eval，退出 2）"
                        );
                        std::process::exit(2)
                    }
                }
            }
            "cdpMethods" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                mode = Mode::CdpMethods(args.next_opt())
            }
            "cdp" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                let method = args.next("cdp <Method 如 Runtime.evaluate>");
                if method == "methods" {
                    mode = Mode::CdpMethods(args.next_opt());
                } else {
                    let mut params = None;
                    while let Some(t) = args.next_opt() {
                        if t == "--params" {
                            params = Some(args.next("cdp --params 的 JSON"));
                        } else if !g.apply(&t, &mut args) {
                            // 尾置全局旗标（R4 上游缺陷 b 修复）：先试全局
                            // 道，未知才按 cdp 面报
                            eprintln!("browse: cdp 只认 --params JSON（收到 {t}，退出 2）");
                            std::process::exit(2)
                        }
                    }
                    mode = Mode::CdpCall(method, params)
                }
            }
            "issue" if g.snippets.is_empty() && mode_is_eval(&mode) => {
                match args.next("issue").as_str() {
                    "new" => {
                        let title = args.next("issue new <标题>");
                        // --kind（bug|improvement 缺省 bug）加 --acceptance（必
                        // 填，关单 result 的完成判据锚，REQ-063 契约与 hst 家
                        // 族同规）加可选 --body；--dry-run 本地校验加载荷预览
                        // 零网络（#57 G6 评审强制项，账本只增不可撤故保留）
                        let mut body = String::new();
                        let mut kind = String::from("bug");
                        let mut acceptance = String::new();
                        let mut dry = false;
                        loop {
                            match args.next_opt() {
                                Some(f) if f == "--body" || f == "-b" => {
                                    body = args.next("--body");
                                }
                                Some(f) if f == "--kind" => kind = args.next("--kind"),
                                Some(f) if f == "--acceptance" => {
                                    acceptance = args.next("--acceptance")
                                }
                                Some(f) if f == "--dry-run" => dry = true,
                                Some(f) => {
                                    if !g.apply(&f, &mut args) {
                                        bail_arg(&f);
                                    }
                                }
                                None => break,
                            }
                        }
                        if acceptance.is_empty() {
                            eprintln!(
                                "browse: --acceptance 必填（关单 result 引 digest 的完成判据；--dry-run 可先预览，退出 2）"
                            );
                            std::process::exit(2);
                        }
                        mode = Mode::IssueNew {
                            title,
                            body,
                            kind,
                            acceptance,
                            dry,
                        };
                    }
                    "list" => {
                        // 默认 100（服务端上限）：默认面即全量，防 open 集超
                        // 20 后旧条目静默隐形（#52）
                        let mut limit = 100u32;
                        let mut before = None;
                        // next_opt 参数尽返 None 即旗标收尾（必值口在
                        // Args::next，分面见 #53）
                        while let Some(f) = args.next_opt() {
                            match f.as_str() {
                                "--before" => {
                                    let b = args.next("--before");
                                    before = Some(b.parse().unwrap_or_else(|_| {
                                        eprintln!("browse: --before 要数字 issue 号（退出 2）");
                                        std::process::exit(2);
                                    }))
                                }
                                "--limit" => {
                                    limit = args.next("--limit").parse().unwrap_or_else(|_| {
                                        eprintln!("browse: --limit 要数字（退出 2）");
                                        std::process::exit(2);
                                    })
                                }
                                other2 => {
                                    if !g.apply(other2, &mut args) {
                                        bail_arg(other2);
                                    }
                                }
                            }
                        }
                        mode = Mode::IssueList { limit, before };
                    }
                    "show" => mode = Mode::IssueShow(args.next("issue show <id>")),
                    other => {
                        eprintln!(
                            "browse: issue 子命令不认识 {other}（new/list/show；close 面已收口归 omc 工位，退出 2）"
                        );
                        std::process::exit(2);
                    }
                }
            }
            other if other.starts_with('-') => {
                eprintln!("browse: 未知参数 {other}（用法错，退出 2）");
                std::process::exit(2);
            }
            snippet => {
                // 尾随裸词守卫（评审 G-14）：非求值形态（up/status/chrome
                // 等子命令已定）下的裸词是误用，静默丢进 snippets 再无声
                // 丢弃＝零报错；子命令自己的实参在各自臂内收，不走本臂
                if !mode_is_eval(&mode) {
                    eprintln!(
                        "browse: {snippet} 不是当前子命令的实参（尾随裸词不收；用法错，退出 2）"
                    );
                    std::process::exit(2);
                }
                g.snippets.push(snippet.to_string());
            }
        }
    }

    // 解构出同名局部：主环后的消费面（llms/serve/求值/子命令分派）沿用
    // 既有变量名，零改动续用；mode 与 g 并生（子命令臂内不迁出）
    let Globals {
        mut snippets,
        serve,
        bind,
        connect,
        new_tab,
        mut ws,
        mut port,
        chrome,
        profile,
        headless,
        pipe,
        json,
        js,
        b64,
        llms,
        full,
        repl,
        proxy,
        proxy_bypass,
        engine_args,
        isolated,
        idle_timeout,
        cookies_csv,
        cli_timeout_s,
        secrets,
        vars,
        snippets_root_flag,
        vars_syntax_flag,
        gen_surface,
    } = g;

    // --llms（REQ-060 一面，族标准名）：裸形 markdown 紧凑手册（REQ-002 的
    // 目录清单形升册）；--full 人读完整目录；--json 机器形 Schema；不拉 daemon；
    // 优先级 json > full
    if llms {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&browse_core::surface::render_schema())?
            );
        } else if full {
            print!("{}", browse_core::surface::render_llms_full());
        } else {
            print!("{}", browse_core::surface::render_manual());
        }
        return Ok(());
    }

    if serve {
        let bind = bind.unwrap_or_else(client::daemon_bind);
        return serve_foreground(
            bind,
            ws,
            port,
            chrome.map(Into::into),
            headless,
            pipe,
            profile,
        )
        .await;
    }

    // 维护命令：从 surface 目录重生成 schema/llms（提交 docs/surface/）
    // @ 实参双道（#64/#66）：给 --vars 走模板道（命令小程序：源先按
    // scripts 根解析、越根候选跳过，未命中回落字面路径；Jinja 语法
    // 渲染，未填键网络前拦 exit 2 带键名）；不给 --vars 走 #64 纯透传道
    // （零漂移：读文件直展开，字面大括号对天然不误伤）。两道都先于
    // --b64 解码；stdin 管道与 REPL 不经此（管道内容本身即片段体）
    // 评审 #66 一轮 G1：--root 只配 @+--vars 模板道与 rune js_lib，
    // 求值面漏给 --vars 时显式报用法错（防「根没生效」被误读成文件不存在）
    if snippets_root_flag.is_some() && vars.is_empty() && !matches!(mode, Mode::Rune { .. }) {
        eprintln!(
            "browse: --root 在求值面只配 @ 实参加 --vars 模板道（漏 --vars 则根不生效；退出 2）"
        );
        std::process::exit(2);
    }
    if !vars.is_empty() && mode_is_eval(&mode) {
        if !snippets.iter().any(|s| s.starts_with('@')) {
            eprintln!("browse: --vars 只配 @ 形态实参（模板道；退出 2）");
            std::process::exit(2);
        }
        let parsed_vars: Vec<(String, String)> = vars
            .iter()
            .map(|v| {
                browse_cli::client::parse_vars_arg(v).unwrap_or_else(|| {
                    eprintln!("browse: --vars 要 键=值 形（得到 {v}；退出 2）");
                    std::process::exit(2);
                })
            })
            .collect();
        let (root, _explicit) = scripts_root(snippets_root_flag.clone());
        for snip in &mut snippets {
            if let Some(name) = snip.strip_prefix('@') {
                let (_path, body) =
                    browse_cli::client::load_template(name, &root).unwrap_or_else(|e| {
                        eprintln!("{e:#}");
                        std::process::exit(2);
                    });
                *snip =
                    // G-1 对齐批：方言/--js 面注释惰性化（JS 族只抽注释）
                    browse_cli::client::render_js_template(cli_vars_syntax(&vars_syntax_flag), &body, &parsed_vars).unwrap_or_else(|e| {
                        eprintln!("{e:#}");
                        std::process::exit(2);
                    });
            }
        }
    } else if snippets.iter().any(|s| s.starts_with('@')) {
        // #64 纯透传：实参以 @ 开头即读文件内容为片段体（方言与 --js 两
        // 形态通用；绝对/相对路径皆可），读失败报错带 CTA
        browse_cli::client::expand_at_args(&mut snippets).unwrap_or_else(|e| {
            eprintln!("{e:#}");
            std::process::exit(2);
        });
    }
    // -b/--b64 通道（#22）：片段实参按 base64 解码（PowerShell 引号与
    // 编码一并绕开；方言与 --js 两形态通用），坏串报 CTA
    if b64 {
        for snip in &mut snippets {
            *snip = browse_cli::client::decode_arg_b64(snip).unwrap_or_else(|e| {
                eprintln!("{e:#}");
                std::process::exit(2);
            });
        }
    }

    if let Some(dir) = gen_surface {
        let p = std::path::PathBuf::from(&dir);
        browse_core::surface::write_surface_files(&p)?;
        println!("surface 已生成到 {}", p.display());
        return Ok(());
    }

    // --connect 映射成显式引擎意图（ws 优先，其次端口）
    if let Some(c) = connect
        && ws.is_none()
        && port.is_none()
    {
        if c.starts_with("ws://") || c.starts_with("wss://") {
            ws = Some(c);
        } else if let Some(p) = cdp::parse_port(&c) {
            port = Some(p);
        } else {
            eprintln!("browse: --connect 认不出 {c}（要 ws://…、端口或 host:port，退出 2）");
            std::process::exit(2);
        }
    }

    match mode {
        Mode::Up => {
            // 生命周期旗标随 daemon 环境注入（#25.1/#25.5）：只影响新拉起
            // 的 daemon，已在跑的以启动时口径为准
            let mut envs: Vec<(&str, String)> = Vec::new();
            if let Some(t) = &idle_timeout {
                envs.push(("BROWSE_IDLE_TIMEOUT", t.clone()));
            }
            if let Some(p) = &proxy {
                envs.push(("BROWSE_PROXY", p.clone()));
            }
            if let Some(b) = &proxy_bypass {
                envs.push(("BROWSE_PROXY_BYPASS", b.clone()));
            }
            // #48：附加旗标空格连接注入 daemon env（值含空格即失真，本
            // 通道只服务无空格 chrome 旗标；含空格场景走 /engine/up 直传）
            if !engine_args.is_empty() {
                envs.push(("BROWSE_ENGINE_ARGS", engine_args.join(" ")));
            }
            if let Some(f) = secrets
                .clone()
                .or_else(|| std::env::var("BROWSE_SECRETS").ok())
            {
                if let Err(e) = browse_core::load_secrets(&f) {
                    eprintln!("{e:#}");
                    std::process::exit(1);
                }
                envs.push(("BROWSE_SECRETS", f));
            }
            // #50/#51 技能层透传（改配置重启 daemon 口径同 BROWSE_SECRETS）
            envs.extend(client::skills_passthrough_env());
            client::ensure_daemon_with_env(&envs).await?;
            let health = client::engine_up(client::UpParams {
                headless,
                chrome,
                ws,
                port,
                pipe,
                profile,
                proxy,
                proxy_bypass,
                isolated,
                engine_args,
            })
            .await?;
            // #48：--cookies 从附着浏览器按域热迁登录态到新引擎（只读源）
            if let Some(csv) = &cookies_csv {
                let domains: Vec<String> = csv
                    .split(',')
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect();
                if !domains.is_empty() {
                    // 方言数组字面量：["a.com","b.com"] 形直接可写
                    let arr = format!(
                        "[{}]",
                        domains
                            .iter()
                            .map(|d| serde_json::to_string(d).unwrap_or_default())
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    let code = format!("return await cloneCookies({arr})");
                    let r = client::eval(&code, false, false, None).await?;
                    println!("cookies 克隆：{r}");
                }
            }
            print_health(&health, json);
            Ok(())
        }
        Mode::Down => {
            if client::daemon_alive().await.is_none() {
                println!("daemon 不在跑（无需退出）");
                return Ok(());
            }
            client::quit().await?;
            println!("daemon 退出（自起引擎已随之下线；附着来源浏览器不受影响）");
            Ok(())
        }
        // 自更新：纯本地链（发现加下载加自替换），blocking 全收 spawn_blocking
        Mode::Update => {
            let brief = tokio::task::spawn_blocking(browse_core::self_update::update_self)
                .await
                .map_err(|e| anyhow!("自更新任务崩了：{e}"))??;
            println!("{}", serde_json::to_string_pretty(&brief)?);
            Ok(())
        }
        Mode::Status => {
            match client::daemon_alive().await {
                Some(h) => print_health(&h, json),
                None => println!("daemon 不在跑（跑任意片段或 browse up 自动拉起）"),
            }
            Ok(())
        }
        // #77：油猴事件回调消费面。daemon 不在不算错（环随 daemon 生命周期，
        // 没起过就没有事件），提示口径同 status
        Mode::Events { since, last, clear } => {
            if clear {
                client::events_clear().await?;
            }
            match client::events_ring(since, last).await {
                Ok(v) => {
                    println!("{}", serde_json::to_string_pretty(&v)?);
                    Ok(())
                }
                Err(e) => {
                    if client::daemon_alive().await.is_none() {
                        println!(
                            "daemon 不在跑（事件环随 daemon 生命周期；browse up 拉起后由脚本上报）"
                        );
                        Ok(())
                    } else {
                        Err(e)
                    }
                }
            }
        }
        // Chromium 版本管理器：纯本地操作，不经 daemon（ADR-0007）。
        // 镜像腿的 blocking http client 必须在阻塞线程里生灭（async 上下文
        // drop 它会 panic），与方言侧同式收 spawn_blocking。
        Mode::ChromeInstall { version, from_dir } => {
            let root = browse_core::chrome_mgr::chromium_root();
            let brief = tokio::task::spawn_blocking(move || match from_dir.as_deref() {
                Some(dir) => browse_core::chrome_mgr::install_from_dir(
                    &root,
                    &version,
                    std::path::Path::new(dir),
                ),
                // 部署目录缺省走 R2 镜像下载腿（chrome.ohmygh.com，REQ-003）
                None => browse_core::chrome_mgr::install_from_mirror(&root, &version),
            })
            .await
            .map_err(|e| anyhow!("安装任务崩了：{e}"))??;
            println!("{}", serde_json::to_string_pretty(&brief)?);
            Ok(())
        }
        Mode::ChromeList => {
            println!(
                "{}",
                serde_json::to_string_pretty(&browse_core::chrome_mgr::list_json(
                    &browse_core::chrome_mgr::chromium_root()
                ))?
            );
            Ok(())
        }
        Mode::ChromeUse(v) => {
            let r = browse_core::chrome_mgr::use_version(
                &browse_core::chrome_mgr::chromium_root(),
                &v,
            )?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        // remove 同为本地重 IO（大版本目录整删），与 update 腿同规收 spawn_blocking
        Mode::ChromeRemove(v) => {
            let root = browse_core::chrome_mgr::chromium_root();
            let brief = tokio::task::spawn_blocking(move || {
                browse_core::chrome_mgr::remove_version(&root, &v)
            })
            .await
            .map_err(|e| anyhow!("删除任务崩了：{e}"))??;
            println!("{}", serde_json::to_string_pretty(&brief)?);
            Ok(())
        }
        // update 腿同样纯本地操作（发现加下载加 pin），blocking 全收 spawn_blocking
        Mode::ChromeUpdate => {
            let root = browse_core::chrome_mgr::chromium_root();
            let brief = tokio::task::spawn_blocking(move || browse_core::chrome_mgr::update(&root))
                .await
                .map_err(|e| anyhow!("升级任务崩了：{e}"))??;
            println!("{}", serde_json::to_string_pretty(&brief)?);
            Ok(())
        }
        Mode::ChromeDoctor => {
            println!(
                "{}",
                serde_json::to_string_pretty(&browse_core::chrome_mgr::doctor_json(
                    &browse_core::chrome_mgr::chromium_root()
                ))?
            );
            Ok(())
        }
        // issue 通道（REQ-063）：真源 ledger.ohmygh.com（旧 issues.ohmygh.com
        // 过渡保役），签名道与只增面在标准 crate ledger-client（总台修正令
        // 2026-09-20 收口）；关闭/删除唯一道 = omc 工位经 herdr 委托
        Mode::IssueNew {
            title,
            body,
            kind,
            acceptance,
            dry,
        } => {
            let mut body = body;
            // 管道读腿有界（评审 F-9，复用 #69 的 read_stdin_bounded）：裸
            // read_to_string 在「开着不关」的 agent 壳管道形态无上界挂死，
            // 且把 --dry-run 的零网络快验也挡在读腿后面
            if body.is_empty() && !std::io::stdin().is_terminal() {
                match read_stdin_bounded() {
                    StdinRead::Data(s) => body = s,
                    StdinRead::Failed(e) => {
                        eprintln!("browse: issue new 读管道失败：{e}（退出 2）");
                        std::process::exit(2);
                    }
                    StdinRead::Silent => {
                        eprintln!(
                            "browse: 提示：stdin 3 秒内未完成读入（无 EOF；agent 壳常开管道形），按空 body 继续；要传正文用 --body 或完整写入并关闭管道"
                        );
                    }
                }
            }
            // --dry-run（#57 G6）：本地同规校验加载荷与签名基预览，零网络
            // 零签名——账本只增不可撤，误发测试单比旧面更贵
            if dry {
                let r =
                    browse_cli::ledger::issue_open_dry_run(&title, &kind, &acceptance, Some(&body))
                        .map_err(|e| anyhow!("{e}"))?;
                println!("{}", serde_json::to_string_pretty(&r)?);
                return Ok(());
            }
            // 实发腿同规预检（收口批评审 F1）：与 --dry-run 同一道本地校验，
            // 坏入参 exit 2 不打网络（服务端 400 也损耗 per-key 日配额）
            if let Err(e) = browse_cli::ledger::validate_issue_open(&title, &kind) {
                eprintln!("browse: {e}（用法错，退出 2）");
                std::process::exit(2);
            }
            let n = ledger_call(move || {
                let n = browse_cli::ledger::client()?
                    .issue_new(&title, &kind, &acceptance, Some(&body))
                    .map_err(|e| e.to_string())?;
                // crate 静默默认守卫（评审 G4）：回执缺 issue 字段时 as_u64
                // 落 0（如幂等命中回执无该键），零即报错防假成功
                if n == 0 {
                    return Err("ledger 回执缺 issue 号（0），拒假成功".into());
                }
                Ok(n)
            })
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "ok": true, "issue": n }))?
            );
            Ok(())
        }
        Mode::IssueList { limit, before } => {
            let eff = browse_cli::ledger::clamp_issue_limit(limit);
            let r = ledger_call(move || {
                browse_cli::ledger::client()?
                    .issue_list(eff, before)
                    .map_err(|e| e.to_string())
            })
            .await?;
            let rows = r["issues"].as_array().cloned().unwrap_or_default();
            if r["has_more"]
                .as_bool()
                .unwrap_or_else(|| browse_cli::ledger::issue_list_saturated(rows.len(), eff))
            {
                eprintln!("{}", browse_cli::ledger::issue_list_truncation_hint(eff));
            }
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        Mode::IssueShow(id) => {
            let n: u64 = id
                .parse()
                .map_err(|_| anyhow!("issue 号须数字（得 {id}）"))?;
            let r = ledger_call(move || {
                browse_cli::ledger::client()?
                    .issue_show(n)
                    .map_err(|e| e.to_string())
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        Mode::ArtifactPublish {
            name,
            kind,
            digest,
            version,
            git_range,
            note,
            summary,
            outcome,
            git_sha,
            deps,
        } => {
            // 值域/形错统一用法错 exit 2（评审 G5，与退出码契约「2=用法错」对齐）
            if let Err(e) = browse_cli::ledger::validate_digest(&digest) {
                eprintln!("browse: {e}（用法错，退出 2）");
                std::process::exit(2);
            }
            if !ledger_client::ARTIFACT_KINDS.contains(&kind.as_str()) {
                eprintln!(
                    "browse: kind 仅十五枚举（ledger-client 同源表），得 {kind}（用法错，退出 2）"
                );
                std::process::exit(2);
            }
            if let Err(e) = browse_cli::ledger::validate_artifact_publish(&name) {
                eprintln!("browse: {e}（用法错，退出 2）");
                std::process::exit(2);
            }
            let id = ledger_call(move || {
                let id = browse_cli::ledger::client()?
                    .artifact_publish_full(
                        &name,
                        &kind,
                        &digest,
                        version.as_deref(),
                        git_range.as_deref(),
                        &deps,
                        note.as_deref(),
                        summary.as_deref(),
                        outcome.as_deref(),
                        git_sha.as_deref(),
                    )
                    .map_err(|e| e.to_string())?;
                // crate 静默默认守卫（评审 G4）：回执缺 artifact_id 时落空串
                if id.is_empty() {
                    return Err("ledger 回执缺 artifact_id（空），拒假成功".into());
                }
                Ok(id)
            })
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "ok": true, "artifact_id": id }))?
            );
            Ok(())
        }
        Mode::ArtifactAttest { id, ev_type, note } => {
            if let Err(e) = browse_cli::ledger::validate_artifact_id(&id) {
                eprintln!("browse: {e}（用法错，退出 2）");
                std::process::exit(2);
            }
            if !ledger_client::ATTEST_TYPES.contains(&ev_type.as_str()) {
                eprintln!(
                    "browse: type 仅 {}（promote/demote/supersede 归 omc 工位），得 {ev_type}（用法错，退出 2）",
                    ledger_client::ATTEST_TYPES.join("|")
                );
                std::process::exit(2);
            }
            let r = ledger_call(move || {
                browse_cli::ledger::client()?
                    .artifact_attest(&id, &ev_type, json!({}), note.as_deref())
                    .map_err(|e| e.to_string())
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        Mode::ArtifactList { current, env_f } => {
            let r = ledger_call(move || {
                browse_cli::ledger::client()?
                    .artifact_list(current, env_f.as_deref())
                    .map_err(|e| e.to_string())
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        Mode::LedgerKeygen { force } => {
            let (kid, jwk, old, backup) =
                browse_cli::ledger::keygen_write(force).map_err(|e| anyhow!("{e}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "keyId": kid,
                    "pubkeyJwk": jwk,
                    "replacedKid": old,
                    // 覆盖前备份件（评审 #66 一轮 G5）：--force 轮换的旧 seed
                    // 落 0600 备份，误操作可回滚
                    "backupKey": backup,
                    // 内置 JWK 的 kid 自证面（评审 G6）：与 keyId 对账——轮换后
                    // 须重烧常量再在册，两值不同即提示未烧
                    "builtinKid": browse_cli::ledger::key_id(),
                    "hint": "总台在册此 kid 后方可写入；私钥在 ~/.browse-rs/ledger/ed25519.key（0600），不打印不进 argv；keyId 与 builtinKid 不同 = 新钥未烧进常量"
                }))?
            );
            Ok(())
        }
        Mode::Fetch {
            url,
            dom_html,
            markdown,
            timeout_s,
        } => {
            let r = browse_cli::fetch::fetch(&url, dom_html, markdown, timeout_s).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        // ---- 片段库（#44）：纯文件系统读，零协议改动，不经 daemon ----
        // ---- workspace 单仓（#50/#51 配套）：git 与文件系统操作，
        // 不经 daemon；blocking 全收 spawn_blocking（chrome_mgr 同构） ----
        Mode::UserscriptInstall(files, restart) => {
            let prof = engine_profile_path(&profile).await;
            let r = tokio::task::spawn_blocking(move || {
                browse_core::userscript_store::install(
                    &prof,
                    &files.iter().map(PathBuf::from).collect::<Vec<_>>(),
                )
            })
            .await
            .map_err(|e| anyhow!("userscript install 任务崩了：{e}"))
            .and_then(|r| r.map_err(|e| anyhow!("{e}")))?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            if restart {
                restart_engine(profile).await?;
            } else {
                eprintln!(
                    "注：引擎启动时读盘，装改后 browse down/up 生效（或下次装改带 --restart）"
                );
            }
            Ok(())
        }
        Mode::UserscriptList => {
            let prof = engine_profile_path(&profile).await;
            let l = tokio::task::spawn_blocking(move || browse_core::userscript_store::list(&prof))
                .await
                .map_err(|e| anyhow!("userscript list 任务崩了：{e}"))
                .and_then(|r| r.map_err(|e| anyhow!("{e}")))?;
            for e in l["scripts"].as_array().cloned().unwrap_or_default() {
                println!(
                    "{} {} {}{}",
                    e["id"].as_str().unwrap_or("?"),
                    if e["enabled"].as_bool().unwrap_or(false) {
                        "on "
                    } else {
                        "off"
                    },
                    e["name"].as_str().unwrap_or("?"),
                    e["pair"]
                        .as_str()
                        .map(|p| format!("  [pair:{p}]"))
                        .unwrap_or_default(),
                );
            }
            if l["scripts"].as_array().is_none_or(|a| a.is_empty()) {
                println!("（空册；browse userscript install <file...> 装）");
            }
            Ok(())
        }
        Mode::UserscriptSetEnabled(ids, on, restart) => {
            let prof = engine_profile_path(&profile).await;
            let mut rs = Vec::new();
            for id in &ids {
                let r = tokio::task::spawn_blocking({
                    let prof = prof.clone();
                    let id = id.clone();
                    move || browse_core::userscript_store::set_enabled(&prof, &id, on)
                })
                .await
                .map_err(|e| anyhow!("userscript set_enabled 任务崩了：{e}"))
                .and_then(|r| r.map_err(|e| anyhow!("{e}")))?;
                rs.push(r);
            }
            println!("{}", serde_json::to_string_pretty(&rs)?);
            if restart {
                restart_engine(profile).await?;
            } else {
                eprintln!("注：引擎重启后生效（或带 --restart）");
            }
            Ok(())
        }
        Mode::UserscriptRemove(ids, restart) => {
            let prof = engine_profile_path(&profile).await;
            let mut rs = Vec::new();
            for id in &ids {
                let r = tokio::task::spawn_blocking({
                    let prof = prof.clone();
                    let id = id.clone();
                    move || browse_core::userscript_store::remove(&prof, &id)
                })
                .await
                .map_err(|e| anyhow!("userscript remove 任务崩了：{e}"))
                .and_then(|r| r.map_err(|e| anyhow!("{e}")))
                // 批量中途失败:已删清单随错带回,不静默(评审 F5-c)
                .map_err(|e| {
                    if rs.is_empty() {
                        e
                    } else {
                        anyhow!(
                            "{e};(已删 {} 条: {})",
                            rs.len(),
                            rs.iter()
                                .filter_map(|v: &Value| v["id"].as_str())
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                })?;
                rs.push(r);
            }
            println!("{}", serde_json::to_string_pretty(&rs)?);
            if restart {
                restart_engine(profile).await?;
            } else {
                eprintln!("注：引擎重启后生效（或带 --restart）");
            }
            Ok(())
        }
        Mode::UserscriptUpdate(id, file, restart) => {
            let prof = engine_profile_path(&profile).await;
            let r = tokio::task::spawn_blocking(move || {
                browse_core::userscript_store::update(&prof, &id, &PathBuf::from(file))
            })
            .await
            .map_err(|e| anyhow!("userscript update 任务崩了：{e}"))
            .and_then(|r| r.map_err(|e| anyhow!("{e}")))?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            if restart {
                restart_engine(profile).await?;
            } else {
                eprintln!("注：引擎重启后生效（或带 --restart）");
            }
            Ok(())
        }
        Mode::UserscriptPath(id) => {
            let prof = engine_profile_path(&profile).await;
            let p = tokio::task::spawn_blocking(move || {
                browse_core::userscript_store::script_path(&prof, &id)
            })
            .await
            .map_err(|e| anyhow!("userscript path 任务崩了：{e}"))
            .and_then(|r| r.map_err(|e| anyhow!("{e}")))?;
            println!("{}", p.display());
            Ok(())
        }
        Mode::UserscriptEval(code) => {
            client::ensure_daemon().await?;
            let r = client::userscript_eval(&code, None).await?;
            if let Some(v) = r["result"]["value"].as_str() {
                println!("{v}");
            } else {
                println!("{}", serde_json::to_string_pretty(&r["result"])?);
            }
            Ok(())
        }
        Mode::CdpCall(method, params) => {
            client::ensure_daemon().await?;
            let pv: Value = match params {
                None => json!({}),
                Some(s) => {
                    serde_json::from_str(&s).map_err(|e| anyhow!("--params 非法 JSON：{e}"))?
                }
            };
            let r = client::cdp_call(&method, pv).await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        Mode::CdpMethods(domain) => {
            let list: Vec<&str> = match domain.as_deref() {
                Some(d) => cdp::methods::methods_of_domain(d),
                None => cdp::methods::METHODS_RAW.lines().collect(),
            };
            if let Some(d) = domain.as_deref()
                && list.is_empty()
            {
                anyhow::bail!("未知 CDP 域 {d}；下一步：browse cdpMethods 查看全部域");
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                for m in list {
                    println!("{m}");
                }
            }
            Ok(())
        }
        Mode::WorkspaceStatus => {
            let root = browse_core::paths::workspace_dir();
            let st =
                tokio::task::spawn_blocking(move || browse_core::workspace::status_json(&root))
                    .await
                    .map_err(|e| anyhow!("workspace status 任务崩了：{e}"))??;
            if json {
                println!("{}", serde_json::to_string_pretty(&st)?);
            } else {
                print_workspace_status(&st);
            }
            Ok(())
        }
        Mode::WorkspaceInstall => {
            let root = browse_core::paths::workspace_dir();
            let r = tokio::task::spawn_blocking(move || browse_core::workspace::install(&root))
                .await
                .map_err(|e| anyhow!("workspace install 任务崩了：{e}"))??;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        Mode::WorkspaceUpdate => {
            let root = browse_core::paths::workspace_dir();
            let r = tokio::task::spawn_blocking(move || browse_core::workspace::update(&root))
                .await
                .map_err(|e| anyhow!("workspace update 任务崩了：{e}"))??;
            println!("{}", serde_json::to_string_pretty(&r)?);
            Ok(())
        }
        // #63 自定义仓登记面：配置固化到应用状态目录 workspaces.json，
        // 多根序首优先（env BROWSE_WORKSPACE 钉死时仍单根最高优先）
        Mode::WorkspaceAdd(path) => {
            let mut roots = browse_core::paths::read_workspace_config();
            let p = std::path::PathBuf::from(&path);
            let abs = if p.is_absolute() {
                p
            } else {
                std::env::current_dir()?.join(&p)
            };
            // G3（评审）：canonicalize 规范化（./、尾斜杠、符号链接别名
            // 归一），同仓多写法不重复入册、remove 也按同形匹配
            let abs = abs.canonicalize().unwrap_or(abs);
            if !abs.is_dir() {
                eprintln!(
                    "browse: workspace add 的路径不是目录（{}）；先建好再登记（退出 2）",
                    abs.display()
                );
                std::process::exit(2);
            }
            if !roots.iter().any(|r| r == &abs) {
                roots.push(abs);
            }
            browse_core::paths::write_workspace_config(&roots)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "config": browse_core::paths::workspace_config_path().display().to_string(),
                    "roots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                }))?
            );
            Ok(())
        }
        Mode::WorkspaceRemove(path) => {
            let mut roots = browse_core::paths::read_workspace_config();
            let before = roots.len();
            let p = std::path::PathBuf::from(&path);
            let canon = p.canonicalize().ok();
            roots.retain(|r| {
                Some(r) != canon.as_ref() && *r != p && r.display().to_string() != path
            });
            browse_core::paths::write_workspace_config(&roots)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "removed": before != roots.len(),
                    "config": browse_core::paths::workspace_config_path().display().to_string(),
                    "roots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                }))?
            );
            Ok(())
        }
        Mode::WorkspaceList => {
            // #63 多根并集（含 roots 面）；未装提示走默认根口径不变
            let root = browse_core::paths::workspace_dir();
            let roots = browse_core::paths::workspace_roots();
            let l = tokio::task::spawn_blocking({
                let roots = roots.clone();
                move || browse_core::workspace::list_json_multi(&roots)
            })
            .await
            .map_err(|e| anyhow!("workspace list 任务崩了：{e}"))?;
            let domains = l["domains"].as_array().cloned().unwrap_or_default();
            let pages = l["pages"].as_array().cloned().unwrap_or_default();
            if domains.is_empty() && pages.is_empty() {
                eprintln!(
                    "workspace 仓未安装或为空（{}）；下一步：browse workspace install",
                    root.display()
                );
                return Ok(());
            }
            println!("domain-skills:");
            for d in &domains {
                let seg = d["segment"].as_str().unwrap_or("?");
                let files = d["files"].as_array().cloned().unwrap_or_default();
                let names: Vec<&str> = files.iter().filter_map(|f| f.as_str()).collect();
                let capped = if d["capped"] == serde_json::json!(true) {
                    " …"
                } else {
                    ""
                };
                println!("  {seg}  {} 文件  {}{capped}", files.len(), names.join(" "));
            }
            println!("page-skills:");
            for p in &pages {
                println!("  {}", p.as_str().unwrap_or("?"));
            }
            Ok(())
        }
        Mode::WorkspaceSite(seg) => {
            if seg.is_empty() {
                // 用法错直出 exit 2（评审 G-F：口径与既有 bail_arg 族一致）
                eprintln!(
                    "browse: workspace site 缺段；用法：browse workspace site <段>[/<文件>]（退出 2）"
                );
                std::process::exit(2);
            }
            // #63 多根读（自定义仓序首优先）
            let roots = browse_core::paths::workspace_roots();
            let text = tokio::task::spawn_blocking(move || {
                browse_core::workspace::read_site_multi(&roots, &seg)
            })
            .await
            .map_err(|e| anyhow!("workspace site 任务崩了：{e}"))??;
            println!("{text}");
            Ok(())
        }
        Mode::WorkspacePage(slug) => {
            if slug.is_empty() {
                eprintln!(
                    "browse: workspace page 缺 slug；用法：browse workspace page <slug>（退出 2）"
                );
                std::process::exit(2);
            }
            let roots = browse_core::paths::workspace_roots();
            let text = tokio::task::spawn_blocking(move || {
                browse_core::workspace::read_page_multi(&roots, &slug)
            })
            .await
            .map_err(|e| anyhow!("workspace page 任务崩了：{e}"))??;
            println!("{text}");
            Ok(())
        }
        // ---- Rune 编排面（#67）：@ 名走 snippets 根(.rn 两试)或字面路径，
        // --vars 同 #66 模板道渲染，POST /rune 由 daemon 在隔离世界执行 ----
        Mode::Rune { script } => {
            // 输入三通道（三形态批）：文件/实参道、管道源码道（无实参时
            // cat plan.rn | browse rune，对齐 --js 管道先例）、管道参数道
            // （有实参时管道串经 RuneRequest 进 cdp::stdin()）
            let piped: Option<String> = if std::io::stdin().is_terminal() {
                None
            } else {
                match read_stdin_bounded() {
                    StdinRead::Data(s) => Some(s),
                    StdinRead::Failed(e) => {
                        eprintln!("browse: rune 读管道失败：{e}（退出 2）");
                        std::process::exit(2);
                    }
                    StdinRead::Silent => {
                        eprintln!(
                            "browse: 提示：stdin 3 秒内未完成读入（无 EOF；agent 壳常开管道形），按无管道参数道继续；管道参数道需期内完整写入并关闭（cat 文件即此形零时延），超界慢生产者数据不候"
                        );
                        None
                    }
                }
            };
            // #67 G7 收口：有实参且管道读入空白（误 </dev/null 常见）提示
            // 一次，消「参数道为何是空串」的排障面
            if let Some(p) = piped.as_deref()
                && script.is_some()
                && p.trim().is_empty()
            {
                eprintln!(
                    "browse: 提示：管道读入为空串（cdp::stdin() 将得空；如系误重定向 </dev/null 所致，参数道需写入数据）"
                );
            }
            let (target, stdin_param, piped_source) = match script {
                Some(t) => (Some(t), piped, None),
                None => match piped.as_deref().map(str::trim) {
                    Some(src) if !src.is_empty() => (None, None, Some(src.to_string())),
                    _ => {
                        eprintln!(
                            "browse: rune 缺脚本实参且无管道；用法：browse rune <script.rn|@名> [--vars k=v]，或 cat plan.rn | browse rune（退出 2）"
                        );
                        std::process::exit(2);
                    }
                },
            };
            // --vars 形状校验先于读文件（评审 #67 一轮 G7：坏形即刻报，
            // 不让文件错掩盖参数错）；--cookies/--new-tab 是 eval 前置
            // 流程，rune 面显式报不支持不静默（一轮 G3 + 二轮 G8/G9）
            if cookies_csv.is_some() {
                eprintln!(
                    "browse: --cookies 尚不支持 rune 面（属引擎起后的登录态热迁流程；退出 2）"
                );
                std::process::exit(2);
            }
            if new_tab {
                eprintln!("browse: --new-tab 尚不支持 rune 面（Rune 脚本自行 goto 开页；退出 2）");
                std::process::exit(2);
            }
            let parsed_vars: Vec<(String, String)> = vars
                .iter()
                .map(|v| {
                    browse_cli::client::parse_vars_arg(v).unwrap_or_else(|| {
                        eprintln!("browse: --vars 要 键=值 形（得到 {v}；退出 2）");
                        std::process::exit(2);
                    })
                })
                .collect();
            let (root, _explicit) = scripts_root(snippets_root_flag.clone());
            let body = match &piped_source {
                Some(src) => src.clone(),
                None => {
                    let target = target.expect("文件道在位（管道源码道已分流）");
                    let at_form = target.starts_with('@');
                    if !at_form && snippets_root_flag.is_some() {
                        // 三形态批后 --root 双生效面：@ 名解析（CLI 侧）与
                        // js_lib（经 daemon）；字面路径本体不走根解析
                        eprintln!(
                            "提示：--root 生效面是 @ 名解析与 js_lib（经 daemon），字面路径本体不读片段库根（{}）",
                            root.display()
                        );
                    }
                    if let Some(name) = target.strip_prefix('@') {
                        match browse_cli::client::load_template_ext(name, &root, "rn", "脚本") {
                            Ok((_p, b)) => b,
                            Err(e) => {
                                eprintln!("{e:#}");
                                std::process::exit(2);
                            }
                        }
                    } else {
                        match std::fs::read_to_string(&target) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!(
                                    "browse: rune 读脚本失败 {target}：{e}；下一步：核对路径，或 @名 走 scripts 根（workspace scripts/ 在册）（退出 2）"
                                );
                                std::process::exit(2);
                            }
                        }
                    }
                }
            };
            // --vars 模板道（与 #66 同一道）：给了即渲染（未填键 exit 2 带键
            // 名）；r# 裸串区先抽出免渲染（批次二），渲染毕回填
            let body = if parsed_vars.is_empty() {
                body
            } else {
                match browse_cli::client::render_rune_template(
                    cli_vars_syntax(&vars_syntax_flag),
                    &body,
                    &parsed_vars,
                ) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("{e:#}");
                        std::process::exit(2);
                    }
                }
            };
            // 引擎/连接旗标族照 eval 面生效（评审 #67 一轮 G3：--ws/--port/
            // --chrome/--headless/--pipe/--profile/--proxy/--isolated/
            // --engine-arg/--secrets 不再静默失效）
            if let Some(f) = &secrets {
                browse_core::load_secrets(f)?;
            }
            client::ensure_daemon().await?;
            client::engine_up(client::UpParams {
                headless,
                chrome,
                ws,
                port,
                pipe,
                profile,
                proxy,
                proxy_bypass,
                isolated,
                engine_args,
            })
            .await?;
            // 评审双批 G1：递根三序终值（旗标 > env > 内置在此收口），防 CLI 与
            // 长命 daemon 两侧 env 各解一次的分叉
            // 评审 F-1（0.30.3）：件首行指令优先旗标且须随请求递 daemon
            // （否则其内 cdp::render 与 js_lib 按缺省 ERB 跑与声明不符）
            let effective_syntax = browse_rune::vars_syntax::directive_of(&body)
                .unwrap_or_else(|| cli_vars_syntax(&vars_syntax_flag));
            match browse_cli::client::rune(
                &body,
                Some(root.display().to_string()),
                stdin_param,
                effective_syntax,
                cli_timeout_s,
            )
            .await
            {
                Ok(v) => {
                    println!("{}", serde_json::to_string_pretty(&v)?);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        // 主动停止道（批次二总台指令）：POST /rune/stop，回执直出 ----
        Mode::RuneStop => {
            client::ensure_daemon().await?;
            let v = browse_cli::client::rune_stop().await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        Mode::Eval => {
            // 裸调用 = 导航事件：无参不弹交互，TTY 裸跑出本仓帮助体 exit 0；
            // REPL 须 --repl 显式进；stdin 管道批处理形态不回归。
            if snippets.is_empty() {
                if repl {
                    if js {
                        eprintln!("browse: --js 与 --repl 不同行（REPL 是方言交互面，退出 2）");
                        std::process::exit(2);
                    }
                    return run_tty(new_tab, js, cli_timeout_s).await;
                }
                if std::io::stdin().is_terminal() {
                    print_help();
                    return Ok(());
                }
                // 管道批处理不回归；空管道（EOF 无内容）视同裸调用出帮助体；
                // --js 空片段裸调用 = stdin 全量 JS 形态（#22）
                let n = run_stdin(new_tab, js, b64, cli_timeout_s).await?;
                if n == 0 {
                    print_help();
                }
                return Ok(());
            }
            run_eval(
                snippets,
                new_tab,
                js,
                ws,
                port,
                chrome,
                headless,
                pipe,
                profile,
                proxy,
                proxy_bypass,
                isolated,
                secrets,
                engine_args,
                cli_timeout_s,
            )
            .await
        }
    }
}

/// 用法错统一出口（退出 2）。各子命令旗标循环共用，措辞不点名子命令。
fn bail_arg(a: &str) -> ! {
    eprintln!("browse: 参数不认识 {a}（用法错，退出 2）");
    std::process::exit(2);
}

/// CLI 实参游标，必值与旗标收集两口分面。单口混用会互踩：必值守卫直出
/// exit 2（G-F 口径）后，靠「参数尽返 Err」收尾的旗标收集循环就永远等
/// 不到终点（0.12.1 子命令族假用法错回归的根因），故拆双口。
struct Args(std::iter::Skip<std::env::Args>);

impl Args {
    /// 取必值实参：参数尽即用法错直出 exit 2（G-F 口径，bail_arg 同族）。
    fn next(&mut self, flag: &str) -> String {
        match self.0.next() {
            Some(v) => v,
            None => {
                eprintln!("browse: {flag} 需要一个值（用法错，退出 2）");
                std::process::exit(2);
            }
        }
    }

    /// 收下一实参：参数尽返 None 即收尾。子命令旗标循环与可选位（如
    /// `snippets list [site]`）专用，不报错。
    fn next_opt(&mut self) -> Option<String> {
        self.0.next()
    }
}

/// rune 面管道参数道的有界读结果（#69）。
enum StdinRead {
    /// 读到 EOF 的完整串。
    Data(String),
    /// 读错误（原 exit 2 路径保留）。
    Failed(std::io::Error),
    /// 期内既无数据也无 EOF（开而不关的 stdin，agent 壳/CI 形）。
    Silent,
}

/// rune 面管道参数道的有界读（#69）：非 TTY stdin 恒读原形在「开着不
/// 关」的调用壳里无上界挂死（战役台实锤，快错全被挡在读腿后面）；读腿
/// 移线程，静默 3 秒过界即按无管道参数道放行（提示进 stderr）。数据与
/// EOF 在期内到达则零额外时延（cat 文件管道不受影响）。读线程在放行后
/// 残留等待，随进程退出回收。
fn read_stdin_bounded() -> StdinRead {
    /// 静默界：期内无数据且无 EOF 即放行。
    const SILENCE: std::time::Duration = std::time::Duration::from_secs(3);
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, std::io::Error>>();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let r = std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf).map(|_| buf);
        let _ = tx.send(r);
    });
    match rx.recv_timeout(SILENCE) {
        Ok(Ok(s)) => StdinRead::Data(s),
        Ok(Err(e)) => StdinRead::Failed(e),
        Err(_) => StdinRead::Silent,
    }
}

/// --vars-syntax 旗标解析（0.30.3）：非法值用法错（不给即缺省 ERB）。
fn cli_vars_syntax(flag: &Option<String>) -> browse_rune::vars_syntax::VarsSyntax {
    match flag.as_deref() {
        None => browse_rune::vars_syntax::VarsSyntax::default(),
        Some(v) => browse_rune::vars_syntax::VarsSyntax::parse(v).unwrap_or_else(|| {
            eprintln!("browse: --vars-syntax 要 classic 或 erb（得到 {v}；退出 2）");
            std::process::exit(2)
        }),
    }
}

fn mode_is_eval(m: &Mode) -> bool {
    matches!(m, Mode::Eval)
}

/// #65 片段库根三序：`--root` 旗标 > `BROWSE_SNIPPETS_ROOT` env > 内置
/// 缺省（env 腿的「谁算显式」单源归
/// 脚本根收口（总台令 2026-10-09 snippets 剔除淘汰）：`--root` 旗标 >
/// workspace 序首根 `scripts/`（[`browse_core::paths::scripts_root`]）。
/// 空串旗标视同未给。返回 `(根, 是否显式配置)`：显式根不存在/不是
/// 目录时报配置错（拼错根与真空库可分），缺省根不在册维持空引导面。
fn scripts_root(flag: Option<String>) -> (std::path::PathBuf, bool) {
    if let Some(f) = flag.filter(|f| !f.is_empty()) {
        return (std::path::PathBuf::from(f), true);
    }
    (browse_core::paths::scripts_root(), false)
}

// 旗标原样透传求值前置的 engine_up，不做参数束重构（与 up 面同一套旗标族）
#[allow(clippy::too_many_arguments)]
async fn run_eval(
    snippets: Vec<String>,
    new_tab: bool,
    js: bool,
    ws: Option<String>,
    port: Option<u16>,
    chrome: Option<String>,
    headless: bool,
    pipe: bool,
    profile: Option<String>,
    proxy: Option<String>,
    proxy_bypass: Option<String>,
    isolated: bool,
    secrets: Option<String>,
    engine_args: Vec<String>,
    timeout_s: Option<u64>,
) -> Result<()> {
    // --secrets 随新 daemon 环境注入（#25.4；已在跑 daemon 以启动时口径为准）；
    // CLI 进程自己也载一份（评审 F1）：return 出口的渲染发生在 CLI 侧，
    // 不载则展示面零脱敏——取值看 daemon 启动时仓，展示看本次 CLI 旗标
    if let Some(f) = secrets
        .clone()
        .or_else(|| std::env::var("BROWSE_SECRETS").ok())
    {
        if let Err(e) = browse_core::load_secrets(&f) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        let mut envs: Vec<(&str, String)> = vec![("BROWSE_SECRETS", f)];
        envs.extend(client::skills_passthrough_env());
        client::ensure_daemon_with_env(&envs).await?;
    } else {
        client::ensure_daemon_with_env(&client::skills_passthrough_env()).await?;
    }
    // 显式连接意图先落引擎（up 面接受同样的旗标），再求值；proxy 三旗标
    // 同守卫透传（评审 F2：静默吞比报错更坑）
    if ws.is_some()
        || port.is_some()
        || chrome.is_some()
        || headless
        || pipe
        || profile.is_some()
        || proxy.is_some()
        || proxy_bypass.is_some()
        || isolated
        || !engine_args.is_empty()
    {
        client::engine_up(client::UpParams {
            headless,
            chrome,
            ws,
            port,
            pipe,
            profile,
            proxy,
            proxy_bypass,
            isolated,
            engine_args,
        })
        .await?;
    }
    // 空片段的裸调用分支已在 Mode::Eval 臂前置处理（帮助体 / 管道 / --repl），
    // 进到这里必带片段
    for snip in &snippets {
        run_snip(snip, new_tab, js, timeout_s).await;
    }
    Ok(())
}

async fn run_snip(snip: &str, new_tab: bool, js: bool, timeout_s: Option<u64>) {
    match client::eval(snip, new_tab, js, timeout_s).await {
        Ok(v) => {
            // 大值落盘（artifact 降级形态）：stdout 只回路径与预览
            let s = match browse_cli::render::render_or_drop_sync(&v) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("browse: 大值落盘失败（{e}），改打全量前 1KB");
                    browse_core::render_result(&v)
                        .chars()
                        .take(1024)
                        .collect::<String>()
                }
            };
            if !s.is_empty() {
                println!("{s}");
            }
        }
        Err(e) => {
            // 错误链过密钥面具（#25.4 评审 G4 防御面）：daemon 侧已脱敏，
            // 这里兜 CLI 自加的 context 层（HTTP 失败嵌响应文本）
            eprintln!(
                "{}",
                browse_core::js_host::mask_secrets_str(&format!("{e:#}"))
            );
            std::process::exit(1);
        }
    }
}

async fn serve_foreground(
    bind: String,
    ws: Option<String>,
    port: Option<u16>,
    chrome: Option<std::path::PathBuf>,
    headless: bool,
    pipe: bool,
    profile: Option<String>,
) -> Result<()> {
    let session = cdp::Session::new();
    let host = browse_core::JsHost::new(session.clone());
    let engine = browse_core::Engine::new(session);
    let mut spec = if let Some(ws) = ws {
        browse_core::EngineSpec::Attach { ws_url: ws }
    } else if let Some(p) = port {
        browse_core::EngineSpec::Port(p)
    } else {
        browse_core::EngineSpec::from_env(chrome, headless, pipe)
    };
    // 显式 --profile 顶掉 from_env 的 BROWSE_PROFILE 缺省
    if let (browse_core::EngineSpec::Auto { profile: p, .. }, Some(explicit)) =
        (&mut spec, profile.map(std::path::PathBuf::from))
    {
        *p = Some(explicit);
    }
    // 密钥文件 daemon 启动即载（#25.4）：改密钥 = 重启 daemon（口径在册）
    if let Some(f) = std::env::var_os("BROWSE_SECRETS") {
        browse_core::load_secrets(&f.to_string_lossy())?;
    }
    let daemon = browse_core::server::Daemon::new(host, engine, spec);
    // 补名表后台预热（REQ-020 G-3 收口）：独立线程建表，用户首个缺失函
    // 数错误不再付约 2 秒收割时延
    browse_rune::preheat_missing_fn_table();
    browse_core::server::serve(daemon, &bind).await
}

fn print_health(h: &serde_json::Value, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(h).unwrap_or_default());
        return;
    }
    let engine = h.get("engine").cloned().unwrap_or(serde_json::Value::Null);
    let (kind, detail) = match engine.get("Spawned") {
        Some(s) => (
            "spawned",
            format!(
                "pid {} headless={} channel={} profile {}",
                s.get("pid").map(|v| v.to_string()).unwrap_or_default(),
                s.get("headless").and_then(|v| v.as_bool()).unwrap_or(false),
                s.get("channel").and_then(|v| v.as_str()).unwrap_or("port"),
                s.get("profile_dir").and_then(|v| v.as_str()).unwrap_or("")
            ),
        ),
        _ => match engine.get("Attached") {
            Some(a) => (
                "attached",
                a.get("ws_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            ),
            _ => ("not-connected", String::new()),
        },
    };
    println!("daemon      http://{}", client::daemon_bind());
    println!(
        "instance    {}",
        h.get("name").and_then(|v| v.as_str()).unwrap_or("default")
    );
    // #59：daemon 宿主自描述 + CLI 跨宿主比对（Windows CLI 经 localhost
    // 转发命中 WSL daemon 时显式标注）
    let d_os = h
        .pointer("/daemon/os")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let d_host = h
        .pointer("/daemon/hostname")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !d_os.is_empty() {
        println!(
            "daemon 宿主 {d_os}/{d_host}（pid {}）",
            h.pointer("/daemon/pid")
                .map(|v| v.to_string())
                .unwrap_or_default()
        );
        // G1（评审）：与 DaemonDesc 同源取 os（cfg 三态对 freebsd 类目标
        // 会误兜底 linux）；G3（评审）：同 OS 异机（隧道/两台 linux）也
        // 告警，比对本机 hostname
        let cli_os = std::env::consts::OS;
        let cli_host = std::process::Command::new("hostname")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|h| !h.is_empty())
            .unwrap_or_default();
        if d_os != cli_os {
            println!(
                "⚠ 跨宿主：CLI({cli_os}) 与 daemon({d_os}/{d_host}) 不同机（localhost 转发场景）"
            );
        } else if !cli_host.is_empty() && d_host != "unknown" && cli_host != d_host {
            println!(
                "⚠ 同 OS 异机：CLI({cli_host}) 与 daemon({d_os}/{d_host}) 不是同一台（隧道或转发场景）"
            );
        }
    }
    println!(
        "uptime      {}s",
        h.get("uptime").and_then(|v| v.as_u64()).unwrap_or(0)
    );
    println!(
        "connected   {}",
        h.get("connected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    );
    // #59 引擎来源行：origin 枚举 + 宿主上下文 + spawn 时刻
    let prov = h.get("engineProvenance").cloned();
    let extra = prov.as_ref().map(|p| {
        let origin = p.get("origin").and_then(|v| v.as_str()).unwrap_or("");
        let host = p.get("hostContext").and_then(|v| v.as_str()).unwrap_or("");
        let at = p.get("spawnedAt").and_then(|v| v.as_u64()).unwrap_or(0);
        let since = if at > 0 {
            format!(" since {}", hms_utc(at))
        } else {
            String::new()
        };
        format!("@ {origin} {host}{since}")
    });
    let extra = extra.unwrap_or_default();
    println!("engine      {kind} {detail} {extra}");
    println!(
        "active tab  {}",
        h.get("activeTargetId")
            .and_then(|v| v.as_str())
            .unwrap_or("-")
    );
}

/// Unix 毫秒转 `HH:MM:SS UTC`（#59 人读面）：无时区依赖，纯除余；零
/// 值返回空串（无 spawn 时刻的附着态）。
fn hms_utc(ms: u64) -> String {
    let secs = ms / 1000;
    let days = secs / 86400;
    // 天数转月日：civil-from-days（无闰表，评审 G4 补日期面）
    let z = days as i64 + 719_468;
    let _era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!(
        "{:02}-{:02} {:02}:{:02}:{:02} UTC",
        m,
        d,
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60
    )
}

async fn run_tty(new_tab: bool, js: bool, timeout_s: Option<u64>) -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut buf = String::new();
    loop {
        let prompt = if buf.is_empty() { "js> " } else { "..> " };
        let line = match rl.readline(prompt) {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => {
                buf.clear();
                continue;
            }
            Err(ReadlineError::Eof) => break,
            Err(e) => return Err(e.into()),
        };
        if buf.is_empty() && matches!(line.trim(), "exit" | "quit" | ".exit") {
            break;
        }
        if line.trim() == "." {
            if !buf.trim().is_empty() {
                run_snip(&buf, new_tab, js, timeout_s).await;
            }
            buf.clear();
            continue;
        }
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&line);
        if snippet_complete(&buf) {
            run_snip(&buf, new_tab, js, timeout_s).await;
            buf.clear();
        }
    }
    if !buf.trim().is_empty() {
        run_snip(&buf, new_tab, js, timeout_s).await;
    }
    Ok(())
}

/// 管道批处理；返回实际求值的段数（零段 = 空管道，裸调用面据此出帮助体）。
///
/// 形态（#22）：方言走括号配平分段；`--js` 走整段 stdin 一次求值（JS 的
/// 语句边界不由括号配平决定）；`--b64` 先整段 base64 解码再进各自形态
/// （`cat x.js.b64 | browse --js --b64` 即管道版全量 JS）。
async fn run_stdin(new_tab: bool, js: bool, b64: bool, timeout_s: Option<u64>) -> Result<usize> {
    let mut raw = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut tokio::io::stdin(), &mut raw).await?;
    let text = if b64 {
        let t = raw.trim();
        if t.is_empty() {
            return Ok(0);
        }
        browse_cli::client::decode_arg_b64(t).unwrap_or_else(|e| {
            eprintln!("{e:#}");
            std::process::exit(2);
        })
    } else {
        raw
    };
    if js {
        // 全量 JS：整段一次求值（空段视同空管道，由裸调用面出帮助体）
        if text.trim().is_empty() {
            return Ok(0);
        }
        run_snip(&text, new_tab, js, timeout_s).await;
        return Ok(1);
    }
    // 行号口径注记（#33 G4）：本路径跳过空行攒 buf，报错行号相对实收
    // 片段（与 -e 直传的原始行号可能差前导空行数）
    let mut ran = 0usize;
    let mut buf = String::new();
    for line in text.lines() {
        let trimmed = line.trim_end_matches(['\r']);
        if trimmed.trim() == "." {
            if !buf.trim().is_empty() {
                run_snip(&buf, new_tab, js, timeout_s).await;
                ran += 1;
            }
            buf.clear();
            continue;
        }
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(trimmed);
        if snippet_complete(&buf) {
            run_snip(&buf, new_tab, js, timeout_s).await;
            ran += 1;
            buf.clear();
        }
    }
    if !buf.trim().is_empty() {
        run_snip(&buf, new_tab, js, timeout_s).await;
        ran += 1;
    }
    Ok(ran)
}

fn print_help() {
    // 帮助面由命令目录活树派生（surface::render_help，cli-docs 标准节序）；
    // --help、-h 与裸调用三入口共用本函数，节序与对齐的守卫在 surface_contract。
    println!("{}", browse_core::surface::render_help());
}

/// workspace status 的人读面（`--json` 走机器面直出）：何时用：
/// `browse workspace status` 未带 --json 时的渲染；安装态、git 概览
/// 与技能计数一行收束，未安装给 install 下一步，非 git 仓给托管
/// 面提示。边界：st 形不对时字段落 `?`/0 不 panic。
fn print_workspace_status(st: &serde_json::Value) {
    let root = st["root"].as_str().unwrap_or("?");
    if st["installed"] != serde_json::json!(true) {
        println!("workspace: 未安装（{root} 不存在）");
        if st["gitPresent"] != serde_json::json!(true) {
            println!("git: 未找到（下一步：装 git，或手工 git clone 种子仓到 {root}）");
        }
        println!("下一步：browse workspace install");
        return;
    }
    let remote = st["remote"].as_str();
    let branch = st["branch"].as_str().unwrap_or("?");
    let head = st["head"].as_str().unwrap_or("?");
    let dirty = st["dirtyChanges"].as_u64().unwrap_or(0);
    let dirty_s = if dirty > 0 {
        format!("，{dirty} 处未提交变更")
    } else {
        String::new()
    };
    // 非 git 仓（目录在但无 .git 或 origin 未登记）给可读提示（评审 G8）
    let remote_s = remote.unwrap_or("未登记（非 git 仓或无 origin）");
    println!("workspace: {root}（origin {remote_s}，{branch}@{head}{dirty_s}）");
    println!(
        "domain-skills: {} 站 {} 文件；page-skills: {} slug",
        st["domainSites"].as_u64().unwrap_or(0),
        st["domainFiles"].as_u64().unwrap_or(0),
        st["pageSlugs"].as_u64().unwrap_or(0),
    );
}
