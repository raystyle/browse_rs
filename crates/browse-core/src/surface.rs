//! 命令面目录（incur-rs 原则的方言版适配）：CLI 子命令、方言全局函数、
//! session 方法**只在这一处登记为数据**，JSON Schema 与 LLM 清单
//! （`llms.txt` / `llms-full.txt`）全部由 [`render_llms`] 系列派生函数
//! 从本目录生成（`browse --gen-surface docs/surface` 重生成），agent 侧
//! 发现通道是 `browse --llms`（与本投影同源直出），`tests/surface_contract.rs`
//! 锁漂移。
//!
//! 与 incur（derive 宏命令图）的差异：我们的接口面是方言片段而非结构化
//! 参数，故目录手写为 `const`；incur 的技能（skill）物种不取，agent 说明书
//! 由 `browse --llms` 直出承担。

use serde_json::{Value, json};

/// 命令在目录中的种类：CLI 子命令、方言全局函数或 session 方法。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CmdKind {
    /// CLI 子命令或旗标形态。
    Cli,
    /// 方言宿主全局函数。
    Global,
    /// `session.<method>` 方法族。
    Session,
}

impl CmdKind {
    fn label(self) -> &'static str {
        match self {
            CmdKind::Cli => "CLI",
            CmdKind::Global => "全局函数",
            CmdKind::Session => "session 方法",
        }
    }
}

/// 描述一个参数的形状：名、类型、必填与否与缺省。
pub struct ArgSpec {
    /// 参数名。
    pub name: &'static str,
    /// `string` / `number` / `boolean` / `object` / `any` / `id` /
    /// `int` / `array`（在册全集，评审 G-37：声明面与实参面曾漂移）。
    /// 派生 schema 的映射（私有 `schema_ty`）：`int` 出 `integer`、`id`
    /// 出 `string`、`any` 与未知值不设 type 约束（省键），其余直通。
    pub ty: &'static str,
    /// 是否必填。
    pub required: bool,
    /// 缺省值描述（可省）。
    pub default: Option<&'static str>,
}

/// 命令目录里一条命令的登记项。
pub struct CmdSpec {
    /// 调用名（全局函数裸名；session 方法不带前缀）。
    pub name: &'static str,
    /// 种类。
    pub kind: CmdKind,
    /// 人读签名（含默认值提示）。
    pub signature: &'static str,
    /// 参数表。
    pub args: &'static [ArgSpec],
    /// 一句话说明。
    pub description: &'static str,
    /// 可照抄示例（方言片段或 shell 行）。
    pub example: &'static str,
}

macro_rules! arg {
    ($n:expr, $t:expr, $r:expr) => {
        ArgSpec {
            name: $n,
            ty: $t,
            required: $r,
            default: None,
        }
    };
    ($n:expr, $t:expr, $r:expr, $d:expr) => {
        ArgSpec {
            name: $n,
            ty: $t,
            required: $r,
            default: Some($d),
        }
    };
}

/// 全量命令目录，CLI/方言/session 三面的单一真相源。
pub const COMMANDS: &[CmdSpec] = &[
    // ---- CLI ----
    CmdSpec {
        name: "eval",
        kind: CmdKind::Cli,
        signature: "browse '<片段>'",
        args: &[
            arg!("snippet", "string", true, "@ 前缀即读文件内容为片段体"),
            arg!(
                "vars",
                "string",
                false,
                "--vars 键=值 可重复；@ 实参同给即模板道"
            ),
        ],
        description: "求值命令方言片段（单步原语，无控制流）。原生 CDP：session.Domain.method（652）。封装：goto、snapshot、clickRef 等。自动拉起 daemon 与引擎。实参以 @ 开头则读文件；配 --vars 则按 Jinja 渲染（缺省 <%= 键 %>）。无实参且 stdin 非终端则从管道读取。",
        example: "browse 'return await currentTab()'",
    },
    CmdSpec {
        name: "eval-e",
        kind: CmdKind::Cli,
        signature: "browse -e '<片段>'",
        args: &[arg!("snippet", "string", true)],
        description: "显式 --eval，与缺省求值等价。",
        example: "browse -e 'return 1'",
    },
    CmdSpec {
        name: "eval-stdin",
        kind: CmdKind::Cli,
        signature: "browse < 文件",
        args: &[],
        description: "从 stdin 批处理：括号配平即提交一段，变量跨段共享。",
        example: "printf '%s\\n' 'const t = await listPageTargets()' 'return t[0].url' | browse",
    },
    CmdSpec {
        name: "js-flag",
        kind: CmdKind::Cli,
        signature: "browse --js '<JS 源码|@<路径>>'",
        args: &[
            arg!("js", "string", true),
            arg!("stdin", "any", false, "空实参时整段管道读"),
            arg!(
                "vars",
                "string",
                false,
                "--vars 键=值 可重复；与 @ 实参同给即模板道"
            ),
        ],
        description: "在页面主世界求值 JS（Runtime.evaluate）。@ 文件、--vars 模板、空实参走管道。",
        example: r#"browse --js 'return (() => { const f = s => s.length; return f("ab"); })()'"#,
    },
    CmdSpec {
        name: "b64-flag",
        kind: CmdKind::Cli,
        signature: "browse --b64 '<base64>'",
        args: &[arg!("base64", "string", true, "或空参走管道整段解码")],
        description: "把片段或 --js 实参按 base64 解码后再求值。空实参读管道。",
        example: r#"browse --js --b64 $(printf '%s' '1+1' | base64)"#,
    },
    CmdSpec {
        name: "new-tab-flag",
        kind: CmdKind::Cli,
        signature: "browse --new-tab '<片段>'",
        args: &[arg!("snippet", "string", true)],
        description: "求值前先打开 about:blank 新标签。",
        example: "browse --new-tab 'return await snapshot()'",
    },
    CmdSpec {
        name: "connect-flag",
        kind: CmdKind::Cli,
        signature: "browse --connect <ws|host:port|端口> '<片段>'",
        args: &[
            arg!("target", "string", true),
            arg!("snippet", "string", false),
        ],
        description: "附着指定 WebSocket URL 或调试端口（host:port 形跨宿主直连，宿主 chrome 须 --remote-debugging-address 绑非回环）。",
        example: "browse --connect 9222 'return await listPageTargets()'",
    },
    CmdSpec {
        name: "up",
        kind: CmdKind::Cli,
        signature: "browse up [--headless] [--pipe] [--chrome <path>] [--profile <dir>] [--ws <url>] [--port <p>] [--proxy <url>] [--proxy-bypass <list>] [--engine-arg <a>] [--isolated] [--idle-timeout <ms>] [--cookies <域csv>]",
        args: &[
            arg!("headless", "boolean", false, "false"),
            arg!("pipe", "boolean", false, "false"),
            arg!("chrome", "string", false, "发现序"),
            arg!("profile", "string", false, "固定 engine-profile"),
            arg!("ws", "string", false),
            arg!("port", "number", false),
            arg!("proxy", "string", false, "无"),
            arg!("proxy-bypass", "string", false, "无"),
            arg!("engine-arg", "string", false, "无"),
            arg!("isolated", "boolean", false, "false"),
            arg!("idle-timeout", "number", false, "3600000"),
            arg!("cookies", "string", false, "无（不给即不热迁）"),
        ],
        description: "启动引擎。优先附着已开浏览器，否则 spawn clean-chrome。--profile 指定用户数据目录；--isolated 退出即删；--headless 无头。",
        example: "browse up --headless --profile ~/profiles/proj-a",
    },
    CmdSpec {
        name: "self-update",
        kind: CmdKind::Cli,
        signature: "browse update",
        args: &[],
        description: "自更新 browse 二进制。镜像优先，失败回落 GitHub；校验 sha256。",
        example: "browse update",
    },
    CmdSpec {
        name: "down",
        kind: CmdKind::Cli,
        signature: "browse down",
        args: &[],
        description: "停止 daemon。只结束本工具 spawn 的引擎。",
        example: "browse down",
    },
    CmdSpec {
        name: "status",
        kind: CmdKind::Cli,
        signature: "browse status [--json]",
        args: &[arg!("json", "boolean", false, "false")],
        description: "显示 daemon、引擎、实例名和活动标签。--json 机器可读。",
        example: "browse status --json",
    },
    CmdSpec {
        name: "llms-flag",
        kind: CmdKind::Cli,
        signature: "browse --llms [--full|--json]",
        args: &[
            arg!("full", "boolean", false, "false"),
            arg!("json", "boolean", false, "false"),
        ],
        description: "打印 agent 手册。--full 完整目录，--json Schema。不启动 daemon。",
        example: "browse --llms",
    },
    CmdSpec {
        name: "repl-flag",
        kind: CmdKind::Cli,
        signature: "browse --repl",
        args: &[],
        description: "进入交互 REPL。裸调用只打印帮助。",
        example: "browse --repl",
    },
    CmdSpec {
        name: "version-flag",
        kind: CmdKind::Cli,
        signature: "browse --version | -V",
        args: &[],
        description: "打印版本号。",
        example: "browse --version",
    },
    CmdSpec {
        name: "chrome-install",
        kind: CmdKind::Cli,
        signature: "browse chrome install <版本> [部署目录]",
        args: &[
            arg!("version", "string", true),
            arg!("fromDir", "string", false, "缺省走 R2 镜像下载"),
        ],
        description: "安装自研 Chromium（clean-chrome）并设为当前版本。默认从镜像下载；给出目录则本地导入。",
        example: "browse chrome install 155.0.8059.39-gen8-r5",
    },
    CmdSpec {
        name: "chrome-list",
        kind: CmdKind::Cli,
        signature: "browse chrome list",
        args: &[],
        description: "列出已安装的 Chromium 版本和当前 pin。",
        example: "browse chrome list",
    },
    CmdSpec {
        name: "chrome-use",
        kind: CmdKind::Cli,
        signature: "browse chrome use <版本>",
        args: &[arg!("version", "string", true)],
        description: "把当前引擎切到已安装的版本。",
        example: "browse chrome use 155.0.8059.39-gen8-r5",
    },
    CmdSpec {
        name: "chrome-update",
        kind: CmdKind::Cli,
        signature: "browse chrome update",
        args: &[],
        description: "升级到镜像 latest 并切换使用。",
        example: "browse chrome update",
    },
    CmdSpec {
        name: "chrome-remove",
        kind: CmdKind::Cli,
        signature: "browse chrome remove <版本>",
        args: &[arg!("version", "string", true)],
        description: "删除已安装版本。当前 pin 指向的版本须先 chrome use 切走。",
        example: "browse chrome remove 155.0.8059.39-gen8-r5",
    },
    CmdSpec {
        name: "chrome-doctor",
        kind: CmdKind::Cli,
        signature: "browse chrome doctor",
        args: &[],
        description: "检查托管 Chromium 是否在位、文件是否完整、Linux 运行库（ldd 直查）零缺失、pin 是否有效。",
        example: "browse chrome doctor",
    },
    CmdSpec {
        name: "fetch",
        kind: CmdKind::Cli,
        signature: "browse fetch <url> [--html] [--markdown|-m] [--timeout <s>]",
        args: &[
            arg!("url", "string", true),
            arg!(
                "markdown",
                "boolean",
                false,
                "false（mdka 真转换：渲染 DOM 或 raw body 转 markdown 字段）"
            ),
            arg!(
                "html",
                "boolean",
                false,
                "false（渲染后 DOM HTML：引擎腿直取 outerHTML 回 html 字段）"
            ),
            arg!("timeout", "number", false, "15"),
        ],
        description: "一次性抓取 URL。默认 HTTP；正文空、墙页或过短则改走引擎。--html 返回渲染 DOM；--markdown 转为 Markdown。",
        example: "browse fetch https://example.com",
    },
    CmdSpec {
        name: "events",
        kind: CmdKind::Cli,
        signature: "browse events [--since <seq>] [--last <n>] [--clear]",
        args: &[
            arg!(
                "since",
                "number",
                false,
                "缺省不用游标（与 last 互斥优先）：只取序号严格大于它的（nextSeq 作下一轮游标）"
            ),
            arg!("last", "number", false, "20（最近 N 条；0 = 全部）"),
            arg!(
                "clear",
                "boolean",
                false,
                "false（先清环再回执；序号不回卷，消费游标不失位）"
            ),
        ],
        description: "读取 userscript 上报的事件环。脚本 POST /events；满 1000 条丢最旧。",
        example: "browse events --since 41",
    },
    CmdSpec {
        name: "rune",
        kind: CmdKind::Cli,
        signature: "browse rune <script.rn|@名> [--vars k=v] [--root <dir>]",
        args: &[
            arg!(
                "script",
                "string",
                true,
                "@ 前缀走 scripts 根(.rn 两试)或字面路径"
            ),
            arg!(
                "vars",
                "string",
                false,
                "模板道（Jinja 渲染源码后编译，同命令小程序）"
            ),
            arg!(
                "root",
                "string",
                false,
                "--root 旗标 > workspace scripts/（只对 @ 名生效）"
            ),
        ],
        description: "运行图灵完备的 Rune 编排脚本。入口 pub async fn main()（不支持返回类型注解；错误传播用 ?，回值用 Ok(..)/Err(..)）。原生 CDP：cdp::raw(\"Domain.method\", params)。页面：goto/eval/eval_handle/call/release、js/js_lib（主世界）、dialect（方言全量桥）、requests/snapshot/request_detail。事件：wait_event(method, 毫秒；方言 waitFor 是秒,单位不同勿混)。工具：render/stdin。宿主模块：json、time（time::sleep 可用）。--vars k=v 按 Jinja 渲染，默认定界 <%= 键 %>。无实参时 stdin 作为脚本；有实参时 stdin 作为 cdp::stdin()。browse rune stop 停止执行。",
        example: "browse rune @x.test/fill-form --vars q=hello",
    },
    CmdSpec {
        name: "workspace-status",
        kind: CmdKind::Cli,
        signature: "browse workspace status [--json]",
        args: &[arg!("json", "boolean", false, "false")],
        description: "显示技能仓安装态、git 与技能计数。",
        example: "browse workspace status",
    },
    CmdSpec {
        name: "workspace-install",
        kind: CmdKind::Cli,
        signature: "browse workspace install",
        args: &[],
        description: "把默认技能仓 clone 到 ~/.browse-rs/workspace。",
        example: "browse workspace install",
    },
    CmdSpec {
        name: "workspace-update",
        kind: CmdKind::Cli,
        signature: "browse workspace update",
        args: &[],
        description: "git pull --ff-only 更新技能仓。有未提交修改则拒绝。",
        example: "browse workspace update",
    },
    CmdSpec {
        name: "workspace-add",
        kind: CmdKind::Cli,
        signature: "browse workspace add <路径>",
        args: &[arg!("path", "string", true)],
        description: "登记自定义技能仓。目录须存在；多根时序首优先。",
        example: "browse workspace add ~/my-skills",
    },
    CmdSpec {
        name: "workspace-remove",
        kind: CmdKind::Cli,
        signature: "browse workspace remove <路径>",
        args: &[arg!("path", "string", true)],
        description: "取消登记自定义技能仓。默认仓不受影响。",
        example: "browse workspace remove ~/my-skills",
    },
    CmdSpec {
        name: "userscript",
        kind: CmdKind::Cli,
        signature: "browse userscript <install|list|enable|disable|remove|update|path|eval>",
        args: &[arg!("id", "string", false)],
        description: "管理按油猴脚本标准实现的 userscript：install、list、enable、disable、remove、update、path、eval。更改后加 --restart 使引擎重读。",
        example: "browse userscript install ~/probe.user.js --restart",
    },
    CmdSpec {
        name: "cdp",
        kind: CmdKind::Cli,
        signature: "browse cdp <Method> [--params JSON]",
        args: &[
            arg!("method", "string", true),
            arg!("params", "string", false),
        ],
        description: "发送一条原生 CDP。方法名为 Domain.method。列出方法：browse cdpMethods。",
        example: "browse cdp Runtime.evaluate --params '{\"expression\":\"1+1\"}'",
    },
    CmdSpec {
        name: "cdp-methods",
        kind: CmdKind::Cli,
        signature: "browse cdpMethods [Domain]",
        args: &[arg!("domain", "string", false, "全部")],
        description: "列出原生 CDP 方法，不启动引擎。可按域过滤。方言等价：await cdpMethods(domain?)。",
        example: "browse cdpMethods Network",
    },
    CmdSpec {
        name: "workspace-list",
        kind: CmdKind::Cli,
        signature: "browse workspace list",
        args: &[],
        description: "列出技能仓中的域名段与页面 slug。",
        example: "browse workspace list",
    },
    CmdSpec {
        name: "workspace-site",
        kind: CmdKind::Cli,
        signature: "browse workspace site <段>[/<文件>]",
        args: &[
            arg!("段", "string", true),
            arg!("文件", "string", false, "缺省打该段清单"),
        ],
        description: "读取域名技能。只给段名返回 README；给 段/文件 读该文件。",
        example: "browse workspace site github",
    },
    CmdSpec {
        name: "workspace-page",
        kind: CmdKind::Cli,
        signature: "browse workspace page <slug>",
        args: &[arg!("slug", "string", true)],
        description: "读取页面技能 README。",
        example: "browse workspace page challenge-stop",
    },
    CmdSpec {
        name: "serve",
        kind: CmdKind::Cli,
        signature: "browse --serve [--bind host:port]",
        args: &[arg!("bind", "string", false, "127.0.0.1:9880")],
        description: "前台运行 daemon。POST /eval、/rune、/engine/up、/quit；GET /health。",
        example: "browse --serve --bind 127.0.0.1:9880",
    },
    // ---- 全局函数 ----
    CmdSpec {
        name: "listPageTargets",
        kind: CmdKind::Global,
        signature: "listPageTargets()",
        args: &[],
        description: "列出可附着的 page target。own=true 的才能 closeTab。",
        example: "const tabs = await listPageTargets()",
    },
    CmdSpec {
        name: "resolveWsUrl",
        kind: CmdKind::Global,
        signature: "resolveWsUrl(opts?)",
        args: &[arg!("opts", "object", false, "{port:9222}")],
        description: "把 wsUrl、port 或 profileDir 解析成 WebSocket URL，不连接。",
        example: "return await resolveWsUrl({port: 9222})",
    },
    CmdSpec {
        name: "detectBrowsers",
        kind: CmdKind::Global,
        signature: "detectBrowsers()",
        args: &[],
        description: "探测本机已开启远程调试的浏览器。",
        example: "return await detectBrowsers()",
    },
    CmdSpec {
        name: "chromeInstall",
        kind: CmdKind::Global,
        signature: "chromeInstall(opts?)",
        args: &[arg!("opts", "object", false, "{fromDir}|{version}")],
        description: "安装自研 Chromium（clean-chrome）并 pin。fromDir 本地导入；只给 version 则从镜像下载。",
        example: "await chromeInstall({version: \"152.0.7977.84\"})",
    },
    CmdSpec {
        name: "chromeList",
        kind: CmdKind::Global,
        signature: "chromeList()",
        args: &[],
        description: "列出已安装版本和当前 pin。",
        example: "return await chromeList()",
    },
    CmdSpec {
        name: "chromeUse",
        kind: CmdKind::Global,
        signature: "chromeUse(version)",
        args: &[arg!("version", "string", true)],
        description: "把 pin 切到已安装版本。",
        example: "await chromeUse(\"152.0.7977.84\")",
    },
    CmdSpec {
        name: "chromeUpdate",
        kind: CmdKind::Global,
        signature: "chromeUpdate()",
        args: &[],
        description: "升级到镜像 latest 并切换 pin。",
        example: "return await chromeUpdate()",
    },
    CmdSpec {
        name: "chromeRemove",
        kind: CmdKind::Global,
        signature: "chromeRemove(version)",
        args: &[arg!("version", "string", true)],
        description: "删除已安装版本。当前 pin 须先切走。",
        example: "await chromeRemove(\"152.0.7977.84\")",
    },
    CmdSpec {
        name: "chromeDoctor",
        kind: CmdKind::Global,
        signature: "chromeDoctor()",
        args: &[],
        description: "检查托管 Chromium 部署。",
        example: "return await chromeDoctor()",
    },
    CmdSpec {
        name: "cdpMethods",
        kind: CmdKind::Global,
        signature: "cdpMethods(domain?)",
        args: &[arg!("domain", "string", false, "全部")],
        description: "列出原生 CDP 全部 652 个方法，可按域过滤。",
        example: "return await cdpMethods(\"Network\")",
    },
    CmdSpec {
        name: "snapshot",
        kind: CmdKind::Global,
        signature: "snapshot(opts?)",
        args: &[
            arg!("ref", "string", false, "单元素子树（部分展开）"),
            arg!("depth", "number", false, "限深层数（可见树根为第 1 层）"),
            arg!(
                "pierce",
                "boolean",
                false,
                "false（穿透 iframe/shadow/OOPIF）"
            ),
        ],
        description: "无障碍树快照。节点带短 ref（e1、e2…）。opts.depth 限深；opts.pierce 含 iframe 与 shadow DOM。",
        example: "const s = await snapshot({depth: 2})",
    },
    CmdSpec {
        name: "findRefs",
        kind: CmdKind::Global,
        signature: "findRefs(query, opts?)",
        args: &[
            arg!("query", "string", true),
            arg!("insensitive", "boolean", false, "false"),
            arg!("context", "number", false, "2（祖先链层数）"),
        ],
        description: "按 name/value 子串检索无障碍树，返回命中节点及其祖先。有命中才替换引用表。",
        example: "const f = await findRefs(\"Sign in\", {context: 1})",
    },
    CmdSpec {
        name: "console",
        kind: CmdKind::Global,
        signature: "console(opts?)",
        args: &[
            arg!("since", "number", false, "0（seq 游标）"),
            arg!(
                "minLevel",
                "string",
                false,
                "verbose（error<warning<log/info<debug<verbose）"
            ),
        ],
        description: "读取当前标签的控制台消息。opts.minLevel 过滤级别。",
        example: "return await console({minLevel: \"error\"})",
    },
    CmdSpec {
        name: "jsErrors",
        kind: CmdKind::Global,
        signature: "jsErrors(since?)",
        args: &[arg!("since", "number", false, "0（seq 游标）")],
        description: "读取当前标签未捕获的 JS 异常。",
        example: "return await jsErrors()",
    },
    CmdSpec {
        name: "requests",
        kind: CmdKind::Global,
        signature: "requests(opts?)",
        args: &[
            arg!("since", "number", false, "0（seq 游标）"),
            arg!("filter", "string", false, "url 子串过滤"),
        ],
        description: "当前标签的网络响应摘要。详情用 requestDetail，响应体用 responseBody。",
        example: "return await requests({filter: \"/api/\"})",
    },
    CmdSpec {
        name: "requestDetail",
        kind: CmdKind::Global,
        signature: "requestDetail(indexOrRequestId, {since, filter}?)",
        args: &[
            arg!("indexOrRequestId", "any", true),
            arg!("since", "number", false, "0（与 requests 同窗）"),
        ],
        description: "单条网络响应详情。可用列表序号或 requestId。",
        example: "return await requestDetail(0)",
    },
    CmdSpec {
        name: "detect",
        kind: CmdKind::Global,
        signature: "detect()",
        args: &[],
        description: "判断页面状态：ok、loading、blank、login-wall、blocked、rate-limited、challenged、stalled。",
        example: "return await detect()",
    },
    CmdSpec {
        name: "cookies",
        kind: CmdKind::Global,
        signature: "cookies(domain?)",
        args: &[arg!("domain", "string", false)],
        description: "列出 cookie。无参为当前页；给 domain 则按该域过滤。",
        example: "return await cookies(\"example.com\")",
    },
    CmdSpec {
        name: "cookieGet",
        kind: CmdKind::Global,
        signature: "cookieGet(name)",
        args: &[arg!("name", "string", true)],
        description: "按名称取一条 cookie。不存在返回 null。",
        example: "return await cookieGet(\"session\")",
    },
    CmdSpec {
        name: "cookieSet",
        kind: CmdKind::Global,
        signature: "cookieSet(name, value, opts?)",
        args: &[
            arg!("name", "string", true),
            arg!("value", "string", true),
            arg!("domain", "string", false, "缺省当前页域"),
            arg!("path", "string", false),
            arg!("expires", "number", false, "Unix 秒"),
            arg!("sameSite", "string", false),
        ],
        description: "写入一条 cookie。缺 domain 用当前页。",
        example: "await cookieSet(\"k\", \"v\", {domain: \"example.com\"})",
    },
    CmdSpec {
        name: "cookieDelete",
        kind: CmdKind::Global,
        signature: "cookieDelete(name, domain?)",
        args: &[
            arg!("name", "string", true),
            arg!("domain", "string", false, "缺省当前页域"),
        ],
        description: "按名称删除 cookie。可指定 domain。",
        example: "await cookieDelete(\"k\", \"example.com\")",
    },
    CmdSpec {
        name: "cookiesClear",
        kind: CmdKind::Global,
        signature: "cookiesClear()",
        args: &[],
        description: "清空浏览器全部 cookie。",
        example: "await cookiesClear()",
    },
    CmdSpec {
        name: "localGet",
        kind: CmdKind::Global,
        signature: "localGet(key)",
        args: &[arg!("key", "string", true)],
        description: "读取 localStorage 键。不存在返回 null。",
        example: "return await localGet(\"theme\")",
    },
    CmdSpec {
        name: "localSet",
        kind: CmdKind::Global,
        signature: "localSet(key, value)",
        args: &[arg!("key", "string", true), arg!("value", "string", true)],
        description: "写入 localStorage 键。",
        example: "await localSet(\"theme\", \"dark\")",
    },
    CmdSpec {
        name: "localDelete",
        kind: CmdKind::Global,
        signature: "localDelete(key)",
        args: &[arg!("key", "string", true)],
        description: "删除 localStorage 键。",
        example: "await localDelete(\"theme\")",
    },
    CmdSpec {
        name: "localClear",
        kind: CmdKind::Global,
        signature: "localClear()",
        args: &[],
        description: "清空当前域 localStorage。",
        example: "await localClear()",
    },
    CmdSpec {
        name: "sessionGet",
        kind: CmdKind::Global,
        signature: "sessionGet(key)",
        args: &[arg!("key", "string", true)],
        description: "读取 sessionStorage 键。",
        example: "return await sessionGet(\"tmp\")",
    },
    CmdSpec {
        name: "sessionSet",
        kind: CmdKind::Global,
        signature: "sessionSet(key, value)",
        args: &[arg!("key", "string", true), arg!("value", "string", true)],
        description: "写入 sessionStorage 键。",
        example: "await sessionSet(\"tmp\", \"1\")",
    },
    CmdSpec {
        name: "sessionDelete",
        kind: CmdKind::Global,
        signature: "sessionDelete(key)",
        args: &[arg!("key", "string", true)],
        description: "删除 sessionStorage 键。",
        example: "await sessionDelete(\"tmp\")",
    },
    CmdSpec {
        name: "sessionClear",
        kind: CmdKind::Global,
        signature: "sessionClear()",
        args: &[],
        description: "清空当前域 sessionStorage。",
        example: "await sessionClear()",
    },
    CmdSpec {
        name: "highlight",
        kind: CmdKind::Global,
        signature: "highlight(ref, {label}?)",
        args: &[
            arg!("ref", "string", true),
            arg!("label", "string", false, "徽标文本（annotate 形态）"),
        ],
        description: "给元素画高亮框。label 显示编号。highlightClear 清除。",
        example: "await highlight(\"e3\", {label: \"e3\"})",
    },
    CmdSpec {
        name: "highlightClear",
        kind: CmdKind::Global,
        signature: "highlightClear()",
        args: &[],
        description: "清除全部高亮框。",
        example: "await highlightClear()",
    },
    CmdSpec {
        name: "annotate",
        kind: CmdKind::Global,
        signature: "annotate(refs)",
        args: &[arg!("refs", "array", true, "ref 字符串数组")],
        description: "给一批 ref 画高亮框。失败则全部撤销。",
        example: "await annotate([\"e1\", \"e3\"])",
    },
    CmdSpec {
        name: "grantPermissions",
        kind: CmdKind::Global,
        signature: "grantPermissions(perms, origin?)",
        args: &[
            arg!(
                "perms",
                "array",
                true,
                "CDP 枚举数组（geolocation/notifications/microphone/camera/…）"
            ),
            arg!("origin", "string", false, "缺省当前页 origin"),
        ],
        description: "授予页面权限，不再弹出提示。",
        example: r#"await grantPermissions(["geolocation", "notifications"])"#,
    },
    CmdSpec {
        name: "semanticSnapshot",
        kind: CmdKind::Global,
        signature: "semanticSnapshot(opts?)",
        args: &[
            arg!("format", "string", false, "tree（默认）/markdown"),
            arg!("depth", "number", false),
            arg!("selector", "string", false, "CSS 作用域"),
            arg!("box", "boolean", false, "带坐标"),
        ],
        description: "引擎层语义快照。需要扩展域版 clean-chrome。",
        example: r#"return await semanticSnapshot({format: "markdown"})"#,
    },
    CmdSpec {
        name: "subscribeChanges",
        kind: CmdKind::Global,
        signature: "subscribeChanges({selector, kinds?})",
        args: &[
            arg!(
                "selector",
                "string",
                true,
                "CSS 子树作用域（引擎冻结面必填）"
            ),
            arg!(
                "kinds",
                "string",
                false,
                "逗号串：childList,attributes,characterData"
            ),
        ],
        description: "订阅 DOM 子树变更。selector 必填。需要扩展域版 clean-chrome。",
        example: "const s = await subscribeChanges({selector: \"#feed\"})",
    },
    CmdSpec {
        name: "unsubscribeChanges",
        kind: CmdKind::Global,
        signature: "unsubscribeChanges(subscriptionId)",
        args: &[arg!("subscriptionId", "string", true)],
        description: "取消 DOM 变更订阅。",
        example: "await unsubscribeChanges(\"1:abc\")",
    },
    CmdSpec {
        name: "engineWaitForResponse",
        kind: CmdKind::Global,
        signature: "engineWaitForResponse(pattern)",
        args: &[arg!("pattern", "string", true)],
        description: "引擎内按 pattern 等待响应。登记即返回 waitId。需要扩展域版 clean-chrome。",
        example: "const w = await engineWaitForResponse(\"url:/api/\")",
    },
    CmdSpec {
        name: "engineCancelWait",
        kind: CmdKind::Global,
        signature: "engineCancelWait(waitId)",
        args: &[arg!("waitId", "string", true)],
        description: "取消引擎层响应等待。",
        example: "await engineCancelWait(\"1:abc\")",
    },
    CmdSpec {
        name: "screenshotDiff",
        kind: CmdKind::Global,
        signature: "screenshotDiff(opts?)",
        args: &[
            arg!("threshold", "number", false),
            arg!(
                "selector",
                "string",
                false,
                "CSS 子树作用域（同 semanticSnapshot）"
            ),
        ],
        description: "引擎内截图差分，返回 {changed, ratio}。需要扩展域版 clean-chrome。",
        example: "return await screenshotDiff()",
    },
    CmdSpec {
        name: "cloneCookies",
        kind: CmdKind::Global,
        signature: "cloneCookies(domains)",
        args: &[arg!("domains", "array", true, "域名字符串数组")],
        description: "从已附着浏览器按域复制 cookie 到当前引擎。源浏览器只读。",
        example: r#"await cloneCookies(["example.com"])"#,
    },
    CmdSpec {
        name: "downloads",
        kind: CmdKind::Global,
        signature: "downloads(since?)",
        args: &[arg!("since", "number", false, "0（seq 游标）")],
        description: "列出下载项及进度。",
        example: "return await downloads()",
    },
    CmdSpec {
        name: "downloadPath",
        kind: CmdKind::Global,
        signature: "downloadPath(guid, timeoutS?)",
        args: &[
            arg!("guid", "string", true),
            arg!("timeoutS", "number", false, "20"),
        ],
        description: "等待指定下载完成并返回落盘路径。",
        example: "return await downloadPath(\"<guid>\")",
    },
    CmdSpec {
        name: "screenshot",
        kind: CmdKind::Global,
        signature: "screenshot(path?, full?, opts?)",
        args: &[
            arg!("path", "string", false, "state 目录时间戳名"),
            arg!("full", "boolean", false, "false"),
            arg!("opts", "object", false, "{format,quality,ifChanged}"),
        ],
        description: "截取当前页到文件。opts.ref 截元素；opts.format png 或 jpeg。",
        example: r#"return await screenshot(null, false, {ifChanged: true})"#,
    },
    CmdSpec {
        name: "pdf",
        kind: CmdKind::Global,
        signature: "pdf(path?)",
        args: &[arg!("path", "string", false, "<state>/pdfs/…pdf")],
        description: "把当前页存为 PDF（无头）。",
        example: "return await pdf()",
    },
    CmdSpec {
        name: "newTab",
        kind: CmdKind::Global,
        signature: "newTab(url?)",
        args: &[arg!("url", "string", false, "about:blank")],
        description: "打开新标签并设为活动。可带 url。",
        example: "await newTab(\"https://example.com\")",
    },
    CmdSpec {
        name: "switchTab",
        kind: CmdKind::Global,
        signature: "switchTab(targetId)",
        args: &[arg!("targetId", "string", true)],
        description: "切换活动标签。",
        example: "await switchTab(tabs[0].targetId)",
    },
    CmdSpec {
        name: "currentTab",
        kind: CmdKind::Global,
        signature: "currentTab()",
        args: &[],
        description: "返回当前活动标签。无活动则 null。",
        example: "return await currentTab()",
    },
    CmdSpec {
        name: "closeTab",
        kind: CmdKind::Global,
        signature: "closeTab(targetId?)",
        args: &[arg!("targetId", "string", false, "当前活动 tab")],
        description: "关闭标签。只允许关闭本工具打开的标签。",
        example: "await closeTab((await currentTab()).targetId)",
    },
    CmdSpec {
        name: "goto",
        kind: CmdKind::Global,
        signature: "goto(url, opts?)",
        args: &[
            arg!("url", "string", true),
            arg!("opts", "object", false, "{timeout:15}"),
        ],
        description: "导航到 URL 并等待加载完成。opts.waitIdle 再等网络静默。",
        example: "return await goto(\"https://example.com\", {waitIdle: true})",
    },
    CmdSpec {
        name: "goBack",
        kind: CmdKind::Global,
        signature: "goBack(delta?)",
        args: &[arg!("delta", "number", false, "1")],
        description: "历史后退。",
        example: "return await goBack(2)",
    },
    CmdSpec {
        name: "goForward",
        kind: CmdKind::Global,
        signature: "goForward(delta?)",
        args: &[arg!("delta", "number", false, "1")],
        description: "历史前进。",
        example: "return await goForward()",
    },
    CmdSpec {
        name: "reload",
        kind: CmdKind::Global,
        signature: "reload(opts?)",
        args: &[arg!("opts", "object", false, "{}")],
        description: "刷新当前页。opts.ignoreCache 强制刷新。",
        example: "return await reload({ignoreCache: true})",
    },
    CmdSpec {
        name: "mouseMove",
        kind: CmdKind::Global,
        signature: "mouseMove(x, y)",
        args: &[arg!("x", "number", true), arg!("y", "number", true)],
        description: "移动鼠标到坐标。",
        example: "await mouseMove(120, 40)",
    },
    CmdSpec {
        name: "mouseDown",
        kind: CmdKind::Global,
        signature: "mouseDown(button?, x?, y?)",
        args: &[
            arg!(
                "button",
                "string",
                false,
                "left（right/middle/back/forward）"
            ),
            arg!("x", "number", false, "缺省最近 mouseMove 落点"),
            arg!("y", "number", false),
        ],
        description: "按下鼠标键。",
        example: "await mouseDown()",
    },
    CmdSpec {
        name: "mouseUp",
        kind: CmdKind::Global,
        signature: "mouseUp(button?, x?, y?)",
        args: &[
            arg!("button", "string", false, "left"),
            arg!("x", "number", false, "缺省最近落点"),
            arg!("y", "number", false),
        ],
        description: "释放鼠标键。",
        example: "await mouseUp()",
    },
    CmdSpec {
        name: "mouseWheel",
        kind: CmdKind::Global,
        signature: "mouseWheel(dx, dy)",
        args: &[arg!("dx", "number", true), arg!("dy", "number", true)],
        description: "滚轮滚动。",
        example: "await mouseWheel(0, 600)",
    },
    CmdSpec {
        name: "dropFiles",
        kind: CmdKind::Global,
        signature: "dropFiles(ref, paths)",
        args: &[
            arg!("ref", "string", true),
            arg!("paths", "array", true, "daemon 侧绝对路径"),
        ],
        description: "向 file 输入框灌入本地文件。",
        example: "await dropFiles(\"e3\", [\"/tmp/a.png\"])",
    },
    CmdSpec {
        name: "clickAt",
        kind: CmdKind::Global,
        signature: "clickAt(x, y, opts?)",
        args: &[
            arg!("x", "number", true),
            arg!("y", "number", true),
            arg!(
                "button",
                "string",
                false,
                "left（right/middle/back/forward）"
            ),
            arg!("clickCount", "number", false, "1（2 即双击语义）"),
        ],
        description: "在视口坐标点击。",
        example: "await clickAt(120, 40)",
    },
    CmdSpec {
        name: "fillInput",
        kind: CmdKind::Global,
        signature: "fillInput(selector, text, submit?)",
        args: &[
            arg!("selector", "string", true),
            arg!("text", "string", true),
            arg!(
                "submit",
                "any",
                false,
                "false（true 或 {submit:true} 填完顺带 Enter）"
            ),
        ],
        description: "按 CSS 选择器填写输入框。",
        example: "await fillInput(\"#q\", \"hello\", {submit: true})",
    },
    CmdSpec {
        name: "clickRef",
        kind: CmdKind::Global,
        signature: "clickRef(ref, opts?)",
        args: &[
            arg!("ref", "string", true),
            arg!(
                "opts",
                "object",
                false,
                "{button:left,clickCount:1,waitNav:false,timeout:10}"
            ),
        ],
        description: "按 snapshot 短 ref 点击。被遮挡则拒绝。",
        example: "await clickRef(\"e3\", {waitNav: true})",
    },
    CmdSpec {
        name: "fillRef",
        kind: CmdKind::Global,
        signature: "fillRef(ref, text, submit?)",
        args: &[
            arg!("ref", "string", true),
            arg!("text", "string", true),
            arg!(
                "submit",
                "any",
                false,
                "false（true 或 {submit:true} 填完顺带 Enter）"
            ),
        ],
        description: "按短 ref 填写输入框。",
        example: "await fillRef(\"e2\", \"hello\", true)",
    },
    CmdSpec {
        name: "selectOption",
        kind: CmdKind::Global,
        signature: "selectOption(ref, value)",
        args: &[arg!("ref", "string", true), arg!("value", "string", true)],
        description: "选择下拉项（value 或可见文本）。",
        example: "return await selectOption(\"e4\", \"Beta\")",
    },
    CmdSpec {
        name: "checkRef",
        kind: CmdKind::Global,
        signature: "checkRef(ref)",
        args: &[arg!("ref", "string", true)],
        description: "勾选 checkbox 或 radio。",
        example: "return await checkRef(\"e5\")",
    },
    CmdSpec {
        name: "uncheckRef",
        kind: CmdKind::Global,
        signature: "uncheckRef(ref)",
        args: &[arg!("ref", "string", true)],
        description: "取消勾选 checkbox。",
        example: "return await uncheckRef(\"e5\")",
    },
    CmdSpec {
        name: "pressKey",
        kind: CmdKind::Global,
        signature: "pressKey(key)",
        args: &[arg!("key", "string", true)],
        description: "按键。支持 Enter 与 Control+a 组合。",
        example: "await pressKey(\"Control+a\")",
    },
    CmdSpec {
        name: "dialogStatus",
        kind: CmdKind::Global,
        signature: "dialogStatus()",
        args: &[],
        description: "当前对话框状态。",
        example: "return await dialogStatus()",
    },
    CmdSpec {
        name: "dialogAccept",
        kind: CmdKind::Global,
        signature: "dialogAccept(text?)",
        args: &[arg!("text", "string", false)],
        description: "接受当前对话框。",
        example: "await dialogAccept(\"yes\")",
    },
    CmdSpec {
        name: "dialogDismiss",
        kind: CmdKind::Global,
        signature: "dialogDismiss()",
        args: &[],
        description: "关闭当前对话框。",
        example: "await dialogDismiss()",
    },
    CmdSpec {
        name: "fetch",
        kind: CmdKind::Global,
        signature: "fetch(url, opts?)",
        args: &[
            arg!("url", "string", true),
            arg!(
                "opts",
                "object",
                false,
                "{html: false, markdown: false, timeout: 15}"
            ),
        ],
        description: "一次性抓取 URL。语义与 browse fetch 相同。",
        example: r#"return await fetch("https://example.com", {markdown: true})"#,
    },
    CmdSpec {
        name: "routeBlock",
        kind: CmdKind::Global,
        signature: "routeBlock(pattern)",
        args: &[arg!("pattern", "string", true)],
        description: "拦截匹配 glob 的请求。",
        example: "await routeBlock(\"*://ads.example.com/*\")",
    },
    CmdSpec {
        name: "routeMock",
        kind: CmdKind::Global,
        signature: "routeMock(pattern, body, opts?)",
        args: &[
            arg!("pattern", "string", true),
            arg!("body", "string", true),
            arg!(
                "opts",
                "object",
                false,
                "{status:200, contentType:\"text/html\"}"
            ),
        ],
        description: "对匹配请求返回本地应答。",
        example: "await routeMock(\"http://mock.test/api*\", \"{\\\"ok\\\":1}\", {contentType: \"application/json\"})",
    },
    CmdSpec {
        name: "routeClear",
        kind: CmdKind::Global,
        signature: "routeClear()",
        args: &[],
        description: "清除全部拦截规则。",
        example: "await routeClear()",
    },
    CmdSpec {
        name: "waitLoad",
        kind: CmdKind::Global,
        signature: "waitLoad(s?)",
        args: &[arg!("s", "number", false, "10")],
        description: "等待 document.readyState 为 complete。",
        example: "await waitLoad(8)",
    },
    CmdSpec {
        name: "waitForResponse",
        kind: CmdKind::Global,
        signature: "waitForResponse(pattern, s?)",
        args: &[
            arg!("pattern", "string", true),
            arg!("s", "number", false, "15"),
        ],
        description: "等待 URL 匹配 glob 的响应完成。",
        example: r#"return await waitForResponse("https://x.test/api*")"#,
    },
    CmdSpec {
        name: "pageEval",
        kind: CmdKind::Global,
        signature: "pageEval(js)",
        args: &[arg!("js", "string", true)],
        description: "在页面求值 JS，等价于 browse --js。",
        example: r#"return await pageEval("(() => { const xs = [1,2,3]; return xs.map(x => x * 2).join(','); })()")"#,
    },
    CmdSpec {
        name: "responseBody",
        kind: CmdKind::Global,
        signature: "responseBody(requestId)",
        args: &[arg!("requestId", "string", true)],
        description: "按 requestId 读取响应体。",
        example: r#"return await responseBody(rid)"#,
    },
    CmdSpec {
        name: "hoverRef",
        kind: CmdKind::Global,
        signature: "hoverRef(ref)",
        args: &[arg!("ref", "string", true)],
        description: "悬停到元素中心。",
        example: r#"await hoverRef("e3")"#,
    },
    CmdSpec {
        name: "hoverAt",
        kind: CmdKind::Global,
        signature: "hoverAt(x, y)",
        args: &[arg!("x", "number", true), arg!("y", "number", true)],
        description: "悬停到视口坐标。",
        example: "await hoverAt(120, 40)",
    },
    CmdSpec {
        name: "dblclickRef",
        kind: CmdKind::Global,
        signature: "dblclickRef(ref)",
        args: &[arg!("ref", "string", true)],
        description: "双击元素。",
        example: r#"await dblclickRef("e5")"#,
    },
    CmdSpec {
        name: "dragRef",
        kind: CmdKind::Global,
        signature: "dragRef(srcRef, dstRef)",
        args: &[
            arg!("srcRef", "string", true),
            arg!("dstRef", "string", true),
        ],
        description: "从源元素拖到目标元素。",
        example: r#"await dragRef("e2", "e4")"#,
    },
    CmdSpec {
        name: "keydown",
        kind: CmdKind::Global,
        signature: "keydown(key)",
        args: &[arg!("key", "string", true)],
        description: "按下键不松开。",
        example: r#"await keydown("Shift")"#,
    },
    CmdSpec {
        name: "keyup",
        kind: CmdKind::Global,
        signature: "keyup(key)",
        args: &[arg!("key", "string", true)],
        description: "松开按键。",
        example: r#"await keyup("Shift")"#,
    },
    CmdSpec {
        name: "typeRef",
        kind: CmdKind::Global,
        signature: "typeRef(ref, text)",
        args: &[arg!("ref", "string", true), arg!("text", "string", true)],
        description: "向元素逐字符输入。",
        example: r#"await typeRef("e7", "hello")"#,
    },
    CmdSpec {
        name: "emulateMedia",
        kind: CmdKind::Global,
        signature: "emulateMedia(opts)",
        args: &[
            arg!("colorScheme", "string", false, "dark/light"),
            arg!("reducedMotion", "string", false, "reduce/no-preference"),
            arg!("forcedColors", "string", false, "active/none"),
            arg!(
                "prefersContrast",
                "string",
                false,
                "more/less/no-preference"
            ),
            arg!("media", "string", false, "screen/print"),
        ],
        description: "仿真媒质特征（prefers-color-scheme 等）。",
        example: "await emulateMedia({colorScheme: \"dark\", media: \"print\"})",
    },
    CmdSpec {
        name: "emulateMediaClear",
        kind: CmdKind::Global,
        signature: "emulateMediaClear()",
        args: &[],
        description: "清除媒质仿真。",
        example: "await emulateMediaClear()",
    },
    CmdSpec {
        name: "emulate",
        kind: CmdKind::Global,
        signature: "emulate(opts)",
        args: &[arg!("opts", "object", true)],
        description: "仿真视口、mobile 或 userAgent。",
        example: "return await emulate({viewport:{width:390,height:844}, mobile:true})",
    },
    CmdSpec {
        name: "setInitScript",
        kind: CmdKind::Global,
        signature: "setInitScript(code)",
        args: &[arg!("code", "string", true)],
        description: "在每个新文档加载前注入脚本。空串清除。",
        example: r#"await setInitScript("window.__no_banner = 1")"#,
    },
    CmdSpec {
        name: "exportStorageState",
        kind: CmdKind::Global,
        signature: "exportStorageState()",
        args: &[],
        description: "导出 cookie 与当前页 localStorage。",
        example: "return await exportStorageState()",
    },
    CmdSpec {
        name: "importStorageState",
        kind: CmdKind::Global,
        signature: "importStorageState(state 或 path)",
        args: &[arg!("state", "any", true)],
        description: "导入 exportStorageState 的会话。",
        example: "return await importStorageState(st)",
    },
    CmdSpec {
        name: "waitIdle",
        kind: CmdKind::Global,
        signature: "waitIdle(s?)",
        args: &[arg!("s", "number", false, "10")],
        description: "等待网络静默。",
        example: "await waitIdle(5)",
    },
    CmdSpec {
        name: "recordStart",
        kind: CmdKind::Global,
        signature: "recordStart(opts?)",
        args: &[arg!("opts", "object", false, "{everyNthFrame:1}")],
        description: "开始录屏。",
        example: "return await recordStart({everyNthFrame: 2})",
    },
    CmdSpec {
        name: "recordChapter",
        kind: CmdKind::Global,
        signature: "recordChapter(title)",
        args: &[arg!("title", "string", true)],
        description: "在录制中插入章节标记。",
        example: "await recordChapter(\"登录流程\")",
    },
    CmdSpec {
        name: "recordStop",
        kind: CmdKind::Global,
        signature: "recordStop()",
        args: &[],
        description: "停止录制。",
        example: "return await recordStop()",
    },
    CmdSpec {
        name: "hostFunctions",
        kind: CmdKind::Global,
        signature: "hostFunctions()",
        args: &[],
        description: "列出本宿主提供的函数。",
        example: "return await hostFunctions()",
    },
    CmdSpec {
        name: "print",
        kind: CmdKind::Global,
        signature: "print(x)",
        args: &[arg!("x", "any", true)],
        description: "把值写到 daemon 标准错误。",
        example: "await print(tabs)",
    },
    CmdSpec {
        name: "JSON.parse",
        kind: CmdKind::Global,
        signature: "JSON.parse(s)",
        args: &[arg!("s", "string", true)],
        description: "把 JSON 文本解析成值。",
        example: r#"return JSON.parse(body).title"#,
    },
    CmdSpec {
        name: "JSON.stringify",
        kind: CmdKind::Global,
        signature: "JSON.stringify(v, indent?)",
        args: &[arg!("v", "any", true), arg!("indent", "int", false)],
        description: "把值序列化为 JSON 字符串。",
        example: "return JSON.stringify(tabs, 2)",
    },
    CmdSpec {
        name: "value-methods",
        kind: CmdKind::Global,
        signature: "值.slice(start,end?) 等",
        args: &[],
        description: "字符串：slice、split、includes、startsWith、endsWith、trim、toUpperCase、toLowerCase。数组：slice、join、includes、concat。",
        example: r#"return body.slice(0, 200)"#,
    },
    // ---- session 方法 ----
    CmdSpec {
        name: "connect",
        kind: CmdKind::Session,
        signature: "session.connect(opts?)",
        args: &[arg!("opts", "object", false, "{port:9222, timeoutMs:5000}")],
        description: "按 wsUrl、port 或 profileDir 连接引擎。",
        example: "await session.connect({port: 9222, timeoutMs: 30000})",
    },
    CmdSpec {
        name: "use",
        kind: CmdKind::Session,
        signature: "session.use(targetId)",
        args: &[arg!("targetId", "string", true)],
        description: "附着指定标签并设为活动。",
        example: "await session.use(tabs[0].targetId)",
    },
    CmdSpec {
        name: "close",
        kind: CmdKind::Session,
        signature: "session.close()",
        args: &[],
        description: "断开连接，不关闭浏览器。",
        example: "await session.close()",
    },
    CmdSpec {
        name: "setActiveSession",
        kind: CmdKind::Session,
        signature: "session.setActiveSession(sessionId?)",
        args: &[arg!("sessionId", "string", false)],
        description: "设置活动 sessionId。",
        example: "await session.setActiveSession(sid)",
    },
    CmdSpec {
        name: "getActiveSession",
        kind: CmdKind::Session,
        signature: "session.getActiveSession()",
        args: &[],
        description: "返回活动 sessionId。",
        example: "return await session.getActiveSession()",
    },
    CmdSpec {
        name: "isConnected",
        kind: CmdKind::Session,
        signature: "session.isConnected()",
        args: &[],
        description: "连接是否存活。",
        example: "return await session.isConnected()",
    },
    CmdSpec {
        name: "waitFor",
        kind: CmdKind::Session,
        signature: "session.waitFor(method, undefined, s?)",
        args: &[
            arg!("method", "string", true),
            arg!("s", "number", false, "15"),
        ],
        description: "从事件缓冲取出指定 method 的下一条事件。超时参秒（rn 面 cdp::wait_event 是毫秒,两原语单位不同勿混）。",
        example: "await session.waitFor(\"Page.frameNavigated\", undefined, 15)",
    },
    CmdSpec {
        name: "waitJs",
        kind: CmdKind::Session,
        signature: "session.waitJs(expression, s?)",
        args: &[
            arg!("expression", "string", true),
            arg!("s", "number", false, "10"),
        ],
        description: "轮询页内 JS 谓词，直到为真。",
        example: "await session.waitJs(\"document.querySelector('#x') !== null\", 5)",
    },
    CmdSpec {
        name: "call",
        kind: CmdKind::Session,
        signature: "session.call(method, params?)",
        args: &[
            arg!("method", "string", true),
            arg!("params", "object", false, "{}"),
        ],
        description: "调用一条原生 CDP。方法名为 Domain.method。",
        example: "await session.call(\"Page.navigate\", {url: \"https://example.com\"})",
    },
    CmdSpec {
        name: "peekEvents",
        kind: CmdKind::Session,
        signature: "session.peekEvents(method, n?)",
        args: &[
            arg!("method", "string", true),
            arg!("n", "number", false, "1"),
        ],
        description: "查看事件缓冲，不取出。",
        example: "return await session.peekEvents(\"Network.requestWillBeSent\", 5)",
    },
    CmdSpec {
        name: "peekEventsSince",
        kind: CmdKind::Session,
        signature: "session.peekEventsSince(method, sinceSeq, n?)",
        args: &[
            arg!("method", "string", true),
            arg!("sinceSeq", "number", true),
            arg!("n", "number", false, "1"),
        ],
        description: "按 seq 增量查看事件。",
        example: "return await session.peekEventsSince(\"Network.requestWillBeSent\", 42, 10)",
    },
    CmdSpec {
        name: "findEvents",
        kind: CmdKind::Session,
        signature: "session.findEvents(method, path, value, n?)",
        args: &[
            arg!("method", "string", true),
            arg!("path", "string", true),
            arg!("value", "any", true),
            arg!("n", "number", false, "1"),
        ],
        description: "按字段过滤查看事件。",
        example: "return await session.findEvents(\"Network.responseReceived\", \"params.response.status\", 404, 1)",
    },
];

/// 返回目录的运行时 JSON，即 hostFunctions 探针的返回体。
pub fn catalog_json() -> Value {
    Value::Array(
        COMMANDS
            .iter()
            .map(|c| {
                json!({
                    "name": c.name,
                    "kind": c.kind.label(),
                    "signature": c.signature,
                    "description": c.description,
                    "example": c.example,
                    "args": c.args.iter().map(|a| json!({
                        "name": a.name,
                        "type": a.ty,
                        "required": a.required,
                        "default": a.default,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// [`ArgSpec::ty`] 到 JSON Schema type 的映射（评审 G-r5 + 四轮 G）：
/// 方言目录值域里的 `int`/`id` 不是合法 JSON Schema type，派生面映射
/// `integer`/`string`；`any` 是任意值（含 null），schema 面不设 type
/// 约束（返回 `None` 省键）；其余直通。
fn schema_ty(ty: &str) -> Option<&'static str> {
    match ty {
        "int" => Some("integer"),
        "id" => Some("string"),
        "any" => None,
        "string" => Some("string"),
        "number" => Some("number"),
        "boolean" => Some("boolean"),
        "object" => Some("object"),
        "array" => Some("array"),
        // 目录外的未知值（防将来漂移）：不设约束，不产非法 type
        _ => None,
    }
}

fn input_schema(c: &CmdSpec) -> Value {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for a in c.args {
        let mut prop = serde_json::Map::new();
        if let Some(t) = schema_ty(a.ty) {
            prop.insert("type".to_string(), json!(t));
        }
        prop.insert(
            "description".to_string(),
            if let Some(d) = a.default {
                json!(format!("缺省 {d}"))
            } else {
                json!("")
            },
        );
        props.insert(a.name.to_string(), Value::Object(prop));
        if a.required {
            required.push(json!(a.name));
        }
    }
    json!({
        "type": "object",
        "properties": props,
        "required": required,
    })
}

fn def_name(c: &CmdSpec) -> String {
    match c.kind {
        CmdKind::Session => format!("session.{name}", name = c.name),
        CmdKind::Cli => format!("cli.{name}", name = c.name),
        CmdKind::Global => c.name.to_string(),
    }
}

/// 渲染 JSON Schema 包：每命令一个 definition，输入按目录、输出是运行时 JSON。
pub fn render_schema() -> Value {
    let mut defs = serde_json::Map::new();
    for c in COMMANDS {
        defs.insert(
            def_name(c),
            json!({
                "type": "object",
                "description": format!("{} {}", c.signature, c.description),
                "input": input_schema(c),
                "output": "runtime JSON（回值随页面与 CDP 而定）",
                "example": c.example,
            }),
        );
    }
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "browse-rs 命令面",
        "description": "由 crates/browse-core/src/surface.rs 目录派生；browse --gen-surface 重生成，tests/surface_contract.rs 锁漂移",
        "definitions": defs,
    })
}

/// 渲染 `--llms` agent 手册：版本、定位、子命令表、通用旗标、常用例。
/// 由 [`COMMANDS`] 派生。行数帽 120。
pub fn render_manual() -> String {
    let mut out = String::from("# browse\n\n");
    out.push_str(&format!("版本 {}。\n", env!("CARGO_PKG_VERSION")));

    out.push_str(
        "\n定位：浏览器驾驶 CLI。自研 Chromium（clean-chrome）含油猴标准运行时、远程调试免确认、无命令行黄条。命令方言为单步原语；Rune 为图灵完备编排（--vars 模板）；stdin 管道输入脚本；原生 CDP 与封装并存。\n",
    );
    out.push_str(
        "常用：goto / snapshot / findRefs / clickRef / fillRef / pageEval；多标签 listPageTargets、switchTab；rune 跑脚本；workspace 读技能。\n\n",
    );
    out.push_str(
        "## 读序\n\n页面操作走方言片段，引擎走 up/down/status/chrome，脚本走 rune 与 userscript。\
         全局旗标尾置合法。操作回执信封带 engineContext（与 /health 同源，agent 断言执行环境\
         用）；stderr 在终端显引擎身份行（BROWSE_ENGINE_CONTEXT=1/0 强制开/关）。\n\n",
    );
    out.push_str("## 子命令\n\n| 形态 | 说明 |\n| --- | --- |\n");
    for c in COMMANDS
        .iter()
        .filter(|c| c.kind == CmdKind::Cli && !c.signature.starts_with("browse --"))
    {
        out.push_str(&format!("| `{}` | {} |\n", c.signature, c.description));
    }
    out.push_str("\n## 通用旗标\n\n| 旗标 | 说明 |\n| --- | --- |\n");
    for c in COMMANDS
        .iter()
        .filter(|c| c.kind == CmdKind::Cli && c.signature.starts_with("browse --"))
    {
        out.push_str(&format!("| `{}` | {} |\n", c.signature, c.description));
    }
    out.push_str(
        "\n## 退出码\n\n| 码 | 义 |\n| --- | --- |\n| 0 | 成功 |\n\
         | 1 | 执行失败 |\n| 2 | 用法错 |\n",
    );
    out.push_str("\n## 常用例\n\n```bash\n");
    let mut seen: Vec<&str> = Vec::new();
    for c in COMMANDS.iter().filter(|c| c.kind == CmdKind::Cli) {
        let display = help_display_name(c);
        if !c.example.starts_with("browse --serve")
            && c.example != display
            && !seen.contains(&c.example)
        {
            seen.push(c.example);
            out.push_str(&format!("{}\n", c.example));
        }
    }
    out.push_str("```\n");
    out
}

// Options 节旗标：不单独成目录条目，与目录旗标
// 合并按字典序渲染；描述与 [`COMMANDS`] 条目无重复。
/// 帮助面 Options 节的伴生旗标表：不入命令目录的维护/伴生旗标与一句
/// 说明，与 [`COMMANDS`] 旗标条目合并渲染（字典序）。
pub const COMPANION_FLAGS: &[(&str, &str)] = &[
    (
        "--timeout <s>",
        "执行超时秒（default: 60）。rune、--js 与方言求值适用。",
    ),
    (
        "--bind <host:port>",
        "daemon 监听地址。默认 127.0.0.1:9880。",
    ),
    ("--chrome <path>", "引擎可执行文件路径。"),
    ("--engine-arg <a>", "传给引擎的附加参数，可重复。"),
    ("--eval, -e <片段>", "显式求值，与缺省形态相同。"),
    ("--full", "输出完整命令目录。"),
    (
        "--vars <k=v>",
        "模板变量，可重复。与 @ 文件同用时按 Jinja 渲染。",
    ),
    ("--root <dir>", "脚本与模板的查找根目录。"),
    ("--headless", "无头模式。"),
    ("--help, -h", "显示帮助。"),
    (
        "--idle-timeout <ms>",
        "引擎闲置回收毫秒（default: 3600000，0 关闭）。",
    ),
    ("--isolated", "使用临时用户目录，引擎退出即删除。"),
    ("--json", "JSON 输出。"),
    ("--pipe", "通过管道连接引擎。"),
    ("--proxy <url>", "引擎 HTTP 代理。"),
    ("--proxy-bypass <list>", "代理绕过列表。"),
    (
        "--vars-syntax <classic|erb>",
        "模板定界（default: erb 为 <%= %>；classic 为 {{ }}）。",
    ),
    (
        "--secrets <file>",
        "dotenv 密钥文件。脚本用 secrets.NAME 读取。",
    ),
    ("--port <p>", "远程调试端口。"),
    (
        "--profile <dir>",
        "用户数据目录。默认 ~/.browse-rs/engine-profile。",
    ),
    (
        "--cookies <域csv>",
        "从已开浏览器复制指定域的 cookie 到新引擎。",
    ),
    ("--ws <url>", "DevTools WebSocket 地址。"),
];

// 展示宽度：CJK 与全角区记 2，其余记 1（Commands/Options 列对齐用）。
fn disp_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            if ('\u{3000}'..='\u{9fff}').contains(&c) || ('\u{ff00}'..='\u{ffef}').contains(&c) {
                2
            } else {
                1
            }
        })
        .sum()
}

/// 帮助面 Commands 分组表：组标题到目录显示名（[`help_display_name`]
/// 派生形）的显式映射。`help_groups_cover_catalog` 守卫锁全条目归组，
/// 目录加新 CLI 条目必须同步归组，否则测试红。
pub const HELP_GROUPS: &[(&str, &[&str])] = &[
    (
        "求值与抓取",
        &[
            "browse '<片段>'",
            "browse -e '<片段>'",
            "browse < 文件",
            "browse rune <script.rn|@名>",
            "browse fetch <url>",
            "browse events",
        ],
    ),
    (
        "引擎管理",
        &[
            "browse up",
            "browse down",
            "browse status",
            "browse chrome install <版本>",
            "browse chrome list",
            "browse chrome use <版本>",
            "browse chrome update",
            "browse chrome remove <版本>",
            "browse chrome doctor",
        ],
    ),
    (
        "片段与知识仓",
        &[
            "browse workspace status",
            "browse workspace install",
            "browse workspace update",
            "browse workspace list",
            "browse workspace site <段>[/<文件>]",
            "browse workspace page <slug>",
            "browse workspace add <路径>",
            "browse workspace remove <路径>",
        ],
    ),
    (
        "userscript 与 CDP",
        &[
            "browse userscript <install|list|enable|disable|remove|update|path|eval>",
            "browse cdp <Method>",
            "browse cdpMethods",
        ],
    ),
    ("自更新", &["browse update"]),
];

/// 目录条目在帮助面与分组表里的显示名：签名截到首个可选段与首个内联
/// 旗标，只留命令路径与位置参数。
pub fn help_display_name(c: &CmdSpec) -> &str {
    c.signature
        .split(" [")
        .next()
        .unwrap_or(c.signature)
        .split(" --")
        .next()
        .unwrap_or(c.signature)
}

/// 帮助面短述：description 首子句机械派生（首个全角括号、冒号、分号或
/// 句号前截断）。帮助与 llms/schema 共用同一描述真源、按面分层深度，
/// 不另立第二份手维护文案。
fn short_desc(d: &str) -> &str {
    d.split(['（', '：', '；', '。'])
        .next()
        .unwrap_or(d)
        .trim()
        .trim_end_matches('，')
}

/// 旗标行描述：短述加默认值回填（首子句截断会丢括号内的 default 信
/// 息，从全文机械提取补尾，不手维护）。
fn flag_row_desc(full: &str) -> String {
    let mut s = short_desc(full).to_string();
    if !s.contains("default:")
        && let Some(rest) = full.split("default: ").nth(1)
    {
        let end = rest.find(['，', '；', '）']).unwrap_or(rest.len());
        s.push_str(&format!("（default: {}）", &rest[..end]));
    }
    s
}

/// 渲染 `--help` 与裸调用共用的帮助面（cli-docs 标准节序）：头行
/// name@版本加一句定位、Usage synopsis、Commands（[`COMMANDS`] 活树按
/// [`HELP_GROUPS`] 分组派生，组内名对齐、描述走首子句短述）、Options
/// （目录旗标加伴生旗标，字典序对齐、短述带默认值回填）、片段方言、
/// 油猴脚本（引擎侧能力指路，无命令面）、环境变量、退出码。
/// `help_covers_catalog` 与 `help_groups_cover_catalog` 守卫测试锁
/// 命令树全覆盖、全归组与版本注入。
///
/// # Panics
///
/// 分组表 [`HELP_GROUPS`] 含目录外显示名时（守卫测试前置拦截，正常
/// 构建不触达）。
///
/// ```
/// let help = browse_core::surface::render_help();
/// assert!(help.contains("Usage:"));
/// assert!(help.contains(&format!("browse@{}", env!("CARGO_PKG_VERSION"))));
/// assert!(help.contains("Commands:"));
/// ```
pub fn render_help() -> String {
    let mut out = format!("browse@{}  浏览器驾驶 CLI\n\n", env!("CARGO_PKG_VERSION"));
    out.push_str("Usage: browse [options] '<方言片段>'\n       browse <command> [options]\n\n");

    // Commands：目录 CLI 形态条目按分组表渲染，组内名按最长名对齐
    out.push_str("Commands:\n");
    for (group, names) in HELP_GROUPS {
        out.push_str(&format!("{group}：\n"));
        let entries: Vec<&CmdSpec> = names
            .iter()
            .map(|n| {
                COMMANDS
                    .iter()
                    .find(|c| c.kind == CmdKind::Cli && help_display_name(c) == *n)
                    .unwrap_or_else(|| panic!("分组表名不在目录：{n}"))
            })
            .collect();
        let gw = entries
            .iter()
            .map(|c| disp_width(help_display_name(c)))
            .max()
            .unwrap_or(0);
        for c in entries {
            let n = help_display_name(c);
            out.push_str(&format!(
                "  {n}{}  {}\n",
                " ".repeat(gw - disp_width(n)),
                short_desc(c.description)
            ));
        }
    }

    // Options：目录旗标条目（短述带默认值回填）加伴生旗标，合并字典序。
    // 旗标名取签名里旗标 token 连取值占位（遇可选 [ 与片段实参 '< 截止）。
    let mut flags: Vec<(String, String)> = COMMANDS
        .iter()
        .filter(|c| c.kind == CmdKind::Cli && c.signature.starts_with("browse --"))
        .map(|c| {
            let name = c
                .signature
                .split(' ')
                .skip(1)
                .take_while(|t| !t.starts_with('[') && !t.starts_with('\''))
                .collect::<Vec<_>>()
                .join(" ");
            (name, flag_row_desc(c.description))
        })
        .collect();
    flags.extend(
        COMPANION_FLAGS
            .iter()
            .map(|(n, d)| ((*n).to_string(), flag_row_desc(d))),
    );
    flags.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let fw = flags.iter().map(|(n, _)| disp_width(n)).max().unwrap_or(0);
    out.push_str("\nOptions:\n");
    for (n, d) in &flags {
        out.push_str(&format!("  {n}{}  {d}\n", " ".repeat(fw - disp_width(n))));
    }

    out.push_str(
        "\n片段方言：\n  const tabs = await listPageTargets()\n  await session.use(tabs[0].targetId)\n  await goto(\"https://example.com\", {waitIdle: true})\n  支持 await/const/return、字面量与成员下标；模板字符串 raw 可多行内嵌页面代码；函数与控制流走 --js；等加载用 goto()/waitLoad()。\n  完整语言面、全局函数与 session 方法清单：browse --llms\n",
    );
    out.push_str(
        "\nuserscript：\n  按油猴脚本标准实现。browse userscript install 写入用户数据目录，按 @match 注入。管理页 chrome://userscripts。更改后加 --restart。\n",
    );
    out.push_str(
        "\nEnvironment Variables:\n  BROWSE_PORT              daemon 端口（default: 9880）\n  BROWSE_NAME              实例名。派生状态目录与端口 9900-9999\n  BROWSE_CHROME            Chromium 可执行文件。文件不存在则继续后续发现序\n  BROWSE_PROFILE           用户数据目录（default: engine-profile）\n  BROWSE_CDP_WS            DevTools WebSocket 地址\n  BROWSE_NO_ATTACH=1       跳过附着，直接启动隔离实例\n  BROWSE_NO_AUTO_DIALOG=1  不自动处理对话框\n  BROWSE_EVAL_TIMEOUT      求值超时秒（default: 60）\n  BROWSE_RUNE_TIMEOUT      rune 超时秒（default: 60）\n  BROWSE_IDLE_TIMEOUT      引擎闲置回收毫秒（default: 3600000，0 关闭）\n  BROWSE_PROXY             引擎 HTTP 代理\n  BROWSE_PROXY_BYPASS      代理绕过列表\n  BROWSE_ENGINE_ARGS       附加引擎参数，空格分隔\n  BROWSE_SECRETS           dotenv 密钥文件\n  BROWSE_WORKSPACE         技能仓根（default: ~/.browse-rs/workspace）\n  BROWSE_SNIPPETS_ROOT     脚本查找根\n  BROWSE_DOMAIN_SKILLS=0   关闭回执中的域名技能提示\n  BROWSE_PAGE_SKILLS=0     关闭回执中的页面技能提示\n  BROWSE_CHROME_MIRROR     Chromium 镜像\n  BROWSE_CHROME_LATEST     chrome update 的 latest 指针\n  BROWSE_RELEASE_MIRROR    browse 自更新镜像\n",
    );
    out.push_str("\n退出码：\n  0 成功 / 1 执行失败 / 2 用法错\n");
    out
}
/// 渲染紧凑 LLM 清单 `llms.txt`（索引层，一行一命令）。
pub fn render_llms() -> String {
    let mut out = String::from(
        "# browse 命令清单\n\n命令方言：browse '<片段>'。Rune：browse rune。\
         userscript 按 @match 注入。完整手册：browse --llms。\n\n",
    );
    for kind in [CmdKind::Cli, CmdKind::Global, CmdKind::Session] {
        out.push_str(&format!(
            "## {}\n\n| 命令 | 说明 |\n| --- | --- |\n",
            kind.label()
        ));
        for c in COMMANDS.iter().filter(|c| c.kind == kind) {
            out.push_str(&format!("| `{}` | {} |\n", c.signature, c.description));
        }
        out.push('\n');
    }
    out.push_str(
        "完整版见 llms-full.txt；机器可读契约见 browse.schema.json；\
         Rust API 文档见 docs/aidoc/llms.txt；二进制直出 browse --llms\
         （--full / --json 同源）。\n",
    );
    out
}

/// 渲染完整 LLM 清单 `llms-full.txt`：索引加逐命令参数与示例。
pub fn render_llms_full() -> String {
    let mut out = render_llms();
    out.push_str("\n---\n");
    for c in COMMANDS {
        out.push_str(&format!("\n## `{}`\n\n{}\n\n", c.signature, c.description));
        if !c.args.is_empty() {
            out.push_str("参数：\n");
            for a in c.args {
                let d = a.default.map(|d| format!("，缺省 {d}")).unwrap_or_default();
                out.push_str(&format!(
                    "- `{}`（{}{}）{}\n",
                    a.name,
                    a.ty,
                    d,
                    if a.required { "，必填" } else { "" }
                ));
            }
        }
        out.push_str(&format!("\n```js\n{}\n```\n", c.example));
    }
    out
}

/// 把派生文件写进目录（维护命令 `browse --gen-surface <dir>` 用）：
/// `browse.schema.json` / `llms.txt` / `llms-full.txt`。
///
/// # Errors
///
/// 写盘失败（目录不可写）。
pub fn write_surface_files(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(
        dir.join("browse.schema.json"),
        serde_json::to_vec_pretty(&render_schema()).unwrap_or_default(),
    )?;
    std::fs::write(dir.join("llms.txt"), render_llms())?;
    std::fs::write(dir.join("llms-full.txt"), render_llms_full())?;
    Ok(())
}
