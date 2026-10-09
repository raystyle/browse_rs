//! 技能触发层（#50/#51/#56 + 清仓重建总台令 2026-10-09）：goto 导航回执
//! 与 fetch/detect 回执的条件附加面。技能本体存 workspace 单仓
//! （[`crate::workspace`]，github.com/raystyle/browse_workspace），形是
//! 「宿主侧 `mech.rn` + 页内侧 `probe.user.js`」**双脚本对**——本模块在
//! 回执里「点名」：热路径只消费各对 `probe.user.js` 的 `/* @detect */`
//! 块，按多根序收集拼成**一次**主世界求值，命中附加清单与 hint，未命中、
//! 无对或关闭时零键，回执不加页面技能键。

use serde_json::{Value, json};

/// 触发开关的关值判式（纯函数）：值为 `0` 或 `false` 才关（opt-out，
/// 未设或任何其他值都开）。与 `BROWSE_NO_ATTACH` 的 opt-in
/// （`1`/`true` 才生效）方向相反，镜像前代 bh 的 `BH_DOMAIN_SKILLS=0`。
fn is_off(v: Option<std::ffi::OsString>) -> bool {
    v.map(|s| s.to_string_lossy().trim().to_ascii_lowercase())
        .is_some_and(|s| s == "0" || s == "false")
}

/// #50 域名层是否开启：`BROWSE_DOMAIN_SKILLS` 未设或非 0/false 即开。
/// 逐调用读 env 不缓存（paths.rs 先例；也保 e2e 可注入临时仓）。
pub fn domain_skills_enabled() -> bool {
    !is_off(std::env::var_os("BROWSE_DOMAIN_SKILLS"))
}

/// #51 页面层是否开启：`BROWSE_PAGE_SKILLS` 未设或非 0/false 即开。
pub fn page_skills_enabled() -> bool {
    !is_off(std::env::var_os("BROWSE_PAGE_SKILLS"))
}

/// URL 到域名段（#50，纯函数）：先做 http(s) 门禁（[`cdp::session::url_host`]
/// 对 data:/about: 这类形返回的是 scheme 段，不能当 host 用），再取 host、
/// 剥 `www.` 前缀、取首个 `.` 前段。非 http(s) 或空 host 返回 `None`。
/// 多词域名不做特判（bbc.co.uk 的段是 bbc，够用口径）。
///
/// # Examples
///
/// ```
/// assert_eq!(browse_core::skills::domain_segment("https://x.com/a"), Some("x".into()));
/// assert_eq!(
///     browse_core::skills::domain_segment("http://www.github.com/"),
///     Some("github".into())
/// );
/// assert_eq!(browse_core::skills::domain_segment("data:text/html,x"), None);
/// ```
pub fn domain_segment(url: &str) -> Option<String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let host = cdp::session::url_host(url);
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let seg = host.split('.').next().unwrap_or("");
    if seg.is_empty() || !seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        None
    } else {
        Some(seg.to_string())
    }
}

/// 列 `<ws>/domain-skills/<段>/` 的对文件清单（纯函数）：成对（mech.rn
/// 加 probe.user.js）才有，排序保回执确定（read_dir 顺序不保证）；目录
/// 缺失或不成对返回空 vec。
///
/// # Examples
///
/// ```
/// // 缺目录返回空（不报错：未装仓是常态）
/// let v = browse_core::skills::list_domain_skills(
///     std::path::Path::new("/nonexistent-browse-ws"), "x");
/// assert!(v.is_empty());
/// ```
pub fn list_domain_skills(ws: &std::path::Path, segment: &str) -> Vec<String> {
    crate::workspace::domain_segment_files(ws, segment)
}

/// 域名层 hint 字段值（#50）：读全文命令字串。
pub fn domain_hint(segment: &str) -> String {
    format!("browse workspace site {segment}")
}

/// 清仓重建后的冻结 slug 名单（页面可控串注入面的白名单，评审 F3 律
/// 沿承）：合法值域只含首批真正写进 workspace 的双脚本对名字（爬虫五对
/// 加安全四对，grok 检索轮定名）。换名单即发版判据，仓侧不新增名字点不
/// 出新键（要自动点名的新 slug 攒批发版进此表）。
pub const FROZEN_PAGE_SLUGS: &[&str] = &[
    "response-tap",
    "scroll-until-end",
    "next-page",
    "spa-route",
    "link-table",
    "challenge-stop",
    "antibot-vendor",
    "fingerprint-watch",
    "policy-surface",
];

/// 一个成对的页面技能（清仓重建总台令 2026-10-09）：多根序首胜的原子
/// 对——`page-skills/<slug>/` 下 `mech.rn` 与 `probe.user.js` 同时在才算。
/// `detect_js` 是从 probe.user.js 抽出的 `/* @detect */` 块正文（无块为
/// 空串：对仍可被 page/list 读到，但热路径不拼批）。
pub struct SkillPair {
    /// 技能名（目录名）。
    pub slug: String,
    /// `/* @detect */` 块正文（无块空串）。
    pub detect_js: String,
}

/// 枚举各根 `page-skills/` 的成对目录（纯函数，单测用）：目录同时有
/// `mech.rn` 与 `probe.user.js` 才算技能；同 slug 序首根整对胜（禁止跨根
/// 拼对，保回执确定性）；结果按 slug 排序。何时用：判定热路径拼批与
/// `browse workspace list/page` 的对清册共用此口径。
pub fn collect_pairs_multi(roots: &[std::path::PathBuf]) -> Vec<SkillPair> {
    let mut seen: Vec<String> = Vec::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for root in roots {
        let ps = root.join("page-skills");
        let Ok(rd) = std::fs::read_dir(&ps) else {
            continue;
        };
        let mut dirs: Vec<std::path::PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for d in dirs {
            let Some(name) = d.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if seen.contains(&name.to_string()) || !is_pair_dir(&d) {
                continue;
            }
            seen.push(name.to_string());
            let detect = std::fs::read_to_string(d.join("probe.user.js"))
                .map(|src| extract_detect_block(&src))
                .unwrap_or_default();
            pairs.push((name.to_string(), detect));
        }
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
        .into_iter()
        .map(|(slug, detect_js)| SkillPair { slug, detect_js })
        .collect()
}

/// 目录是否成对（`mech.rn` 与 `probe.user.js` 同时在；缺一跳过，daemon
/// 日志不刷——静默跳过是该目录不构成技能的语义本身）。
fn is_pair_dir(d: &std::path::Path) -> bool {
    d.join("mech.rn").is_file() && d.join("probe.user.js").is_file()
}

/// 抽 `/* @detect */` 到 `/* @enddetect */` 之间的正文（纯函数）：无标记
/// 对回空串。块内约定是 `function browseDetect(facts) {...}`，命中返回
/// `{slug, confidence}`，否则 `null`。
fn extract_detect_block(src: &str) -> String {
    let Some(open) = src.find("/* @detect */") else {
        return String::new();
    };
    let body_start = open + "/* @detect */".len();
    let Some(close) = src[body_start..].find("/* @enddetect */") else {
        return String::new();
    };
    src[body_start..body_start + close].trim().to_string()
}

/// 序首 `_prelude.js`（facts 游走脚本，page-skills 层）：先命中根整胜，
/// 全根未命中回空串（拼批以空 facts 跑，检测块仍可只靠自己的
/// querySelector）。改游走不发版（判定正文全在仓里）。
pub fn load_prelude_multi(roots: &[std::path::PathBuf]) -> String {
    for root in roots {
        let p = root.join("page-skills").join("_prelude.js");
        if p.is_file() {
            if let Ok(body) = std::fs::read_to_string(&p) {
                return body;
            }
            static WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            if WARNED.set(()).is_ok() {
                eprintln!("[browse] _prelude.js 在而不可读，该根按未命中处理");
            }
        }
    }
    String::new()
}

/// 拼批判定 JS（纯函数，单测锁形）：prelude 在 facts 上填事实（约定可写
/// `facts.framework = {name, version}`），各对的检测块在独立 IIFE 里跑
/// （各块自带 `browseDetect` 作用域，互不撞名），块自调约定函数并把非空
/// 返回收进 slugs；单块抛错只弃该块。恒返 `{framework, slugs}` 骨架。
pub fn build_batch_js(prelude: &str, pairs: &[SkillPair]) -> String {
    let mut out = String::from(
        "(() => {\n  const out = { framework: null, slugs: [] };\n  const facts = {};\n",
    );
    if !prelude.is_empty() {
        out.push_str("  try {\n");
        out.push_str(prelude);
        out.push_str("\n  } catch (e) {}\n");
    }
    out.push_str("  try { if (facts.framework) out.framework = facts.framework; } catch (e) {}\n");
    for p in pairs {
        if p.detect_js.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "  try {{ const r = (function(facts) {{ {}\n    return (typeof browseDetect === 'function') ? browseDetect(facts) : null; }})(facts); if (r && r.slug) out.slugs.push(r); }} catch (e) {{}}\n",
            p.detect_js
        ));
    }
    out.push_str("  return out;\n})()");
    out
}

/// 把探测结果变成回执附加键对（#51，纯函数）：framework 形合法出
/// `("framework", {name,version})`；slugs 过滤后非空出 `page_skills` 与
/// `page_skills_hint`（各 slug 的读全文命令用 `；` 连接）；全空返回
/// 空 vec（调用方零插入）。**白名单是硬边界（评审 F3）**：探测产自页内
/// 而页面可控（覆写全局或伪造判据脚本回执都做得到），故这里逐条校验
/// slug 在 [`FROZEN_PAGE_SLUGS`]、confidence 是两档枚举、去重保序
/// （天然封顶名单长），framework 的 name/version 形与长度也校验；
/// 非法条目静默丢弃，全非法即零键。
///
/// # Examples
///
/// ```
/// let probe = serde_json::json!({
///     "framework": {"name": "vue", "version": "3.5"},
///     "slugs": [{"slug": "challenge-stop", "confidence": "CONFIRMED"}]
/// });
/// let f = browse_core::skills::page_fields(&probe);
/// assert_eq!(f.len(), 3, "framework 加 page_skills 加 hint: {f:?}");
///
/// // 页面伪造形：名单外 slug 与非法 confidence 全被滤掉（评审 F3）
/// let forged = serde_json::json!({
///     "framework": {"name": "pwn"},
///     "slugs": [
///         {"slug": "../../etc/passwd", "confidence": "CONFIRMED"},
///         {"slug": "challenge-stop", "confidence": "MAYBE"},
///         {"slug": "antibot-vendor", "confidence": "CONFIRMED"}
///     ]
/// });
/// let f = browse_core::skills::page_fields(&forged);
/// let skills = f.iter().find(|(k, _)| *k == "page_skills").unwrap();
/// assert_eq!(skills.1, serde_json::json!([{"slug":"antibot-vendor","confidence":"CONFIRMED"}]));
/// ```
pub fn page_fields(probe: &Value) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    if let Some(fw) = valid_framework(probe.get("framework")) {
        out.push(("framework", fw));
    }
    let mut kept: Vec<(&str, &str)> = Vec::new();
    for e in probe
        .get("slugs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(slug), Some(conf)) = (
            e.get("slug").and_then(Value::as_str),
            e.get("confidence").and_then(Value::as_str),
        ) else {
            continue;
        };
        if !FROZEN_PAGE_SLUGS.contains(&slug)
            || !matches!(conf, "CONFIRMED" | "PLAUSIBLE")
            || kept.iter().any(|(s, _)| *s == slug)
        {
            continue;
        }
        kept.push((slug, conf));
    }
    out.extend(page_skills_keys(&kept));
    out
}

/// （slug, confidence）对到 `page_skills` 加 `page_skills_hint` 键对的
/// 统一构建（crate 内缝）：探测腿（[`page_fields`]）与 detect 判读腿
/// （[`verdict_fields`]）共用，保两事件源的回执形逐字节同构。空入参
/// 返回空 vec（调用方零插入）。
fn page_skills_keys(pairs: &[(&str, &str)]) -> Vec<(&'static str, Value)> {
    if pairs.is_empty() {
        return Vec::new();
    }
    let kept: Vec<Value> = pairs
        .iter()
        .map(|(s, c)| json!({ "slug": s, "confidence": c }))
        .collect();
    let cmds: Vec<String> = pairs
        .iter()
        .map(|(s, _)| format!("browse workspace page {s}"))
        .collect();
    vec![
        ("page_skills", json!(kept)),
        ("page_skills_hint", json!(cmds.join("；"))),
    ]
}

/// framework 形校验（评审 F3 白名单的 framework 腿）：name 是非空短串
///（字母数字与连字符，<= 24 字节），version 是串（<= 64 字节）或
/// null/缺省；形不对返回 `None`（不附 framework 键）。
fn valid_framework(v: Option<&Value>) -> Option<Value> {
    let fw = v.filter(|f| !f.is_null())?;
    let name = fw.get("name")?.as_str()?;
    if name.is_empty()
        || name.len() > 24
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    let version = match fw.get("version") {
        None | Some(Value::Null) => Value::Null,
        Some(s @ Value::String(_)) => {
            let sv = s.as_str()?;
            if sv.len() > 64 {
                return None;
            }
            s.clone()
        }
        Some(_) => return None,
    };
    Some(json!({ "name": name, "version": version }))
}

/// URL 到域名层键对（#56，fetch 腿的口径源）：与 goto 同口径的域名段
/// 匹配（[`domain_segment`] 加 [`list_domain_skills`]），但收 ws 目录
/// 与 url 显式入参——fetch 的两形态（HTTP 直出与引擎升级）都在 CLI 侧
/// 持 URL，点名不依赖引擎与 Session。何时用：回执含 url 且只想要域名
/// 层点名的调用方。边界：关闭（`BROWSE_DOMAIN_SKILLS`）或未命中返回
/// 空 vec，调用方零插入。
///
/// # Examples
///
/// ```
/// let dir = std::env::temp_dir().join(format!("browse-sk56-{}", std::process::id()));
/// let seg = dir.join("domain-skills").join("x");
/// std::fs::create_dir_all(&seg).unwrap();
/// std::fs::write(seg.join("mech.rn"), "pub async fn main() {}").unwrap();
/// std::fs::write(seg.join("probe.user.js"), "// probe").unwrap();
/// let f = browse_core::skills::url_domain_fields(&dir, "https://www.x.com/a");
/// assert_eq!(f.len(), 2, "命中出两键: {f:?}");
/// assert!(f.iter().all(|(k, _)| k.starts_with("domain_skills")));
/// assert!(browse_core::skills::url_domain_fields(&dir, "data:text/html,x").is_empty());
/// let _ = std::fs::remove_dir_all(&dir);
/// ```
pub fn url_domain_fields(ws: &std::path::Path, url: &str) -> Vec<(&'static str, Value)> {
    if !domain_skills_enabled() {
        return Vec::new();
    }
    match domain_segment(url) {
        Some(seg) => {
            let files = list_domain_skills(ws, &seg);
            if files.is_empty() {
                return Vec::new();
            }
            vec![
                ("domain_skills", json!(files)),
                ("domain_skills_hint", json!(domain_hint(&seg))),
            ]
        }
        None => Vec::new(),
    }
}

/// detect() 判读到 page-skill slug 的映射表（#56，纯函数）：challenged
/// 映射 bot-shield 与 captcha，login-wall 映射 login-wall，其余判读
/// （rate-limited/blocked/stalled/blank/loading/ok）无映射返回空。置信
/// 档恒 PLAUSIBLE：判读是关键词与网络行为推断，非 DOM 特征实证（对照
/// 探针腿的 CONFIRMED 档要 cf-chl DOM 这类硬证据）。slug 值域即
/// [`FROZEN_PAGE_SLUGS`]，表驱动静态对不越白名单。
///
/// # Examples
///
/// ```
/// assert_eq!(
///     browse_core::skills::verdict_page_slugs("challenged"),
///     &[("challenge-stop", "PLAUSIBLE")]
/// );
/// assert!(browse_core::skills::verdict_page_slugs("login-wall").is_empty());
/// assert!(browse_core::skills::verdict_page_slugs("ok").is_empty());
/// ```
pub fn verdict_page_slugs(verdict: &str) -> &'static [(&'static str, &'static str)] {
    match verdict {
        // 清仓重建映射:挑战判读指向新对 challenge-stop(PLAUSIBLE 档恒定,
        // 判读是推断非 DOM 实证);login-wall 无新对承接,零映射
        "challenged" => &[("challenge-stop", "PLAUSIBLE")],
        _ => &[],
    }
}

/// detect() 回执的技能附加入口（#56）：判读经 [`verdict_page_slugs`]
/// 映射后出 `page_skills` 加 `page_skills_hint`（与探测腿同形）；
/// 关闭（`BROWSE_PAGE_SKILLS`）或判读无映射返回空 vec，调用方零插入。
/// 调用点在 [`crate::js_host`] 的 detect 判读构建处。
///
/// # Examples
///
/// ```
/// let f = browse_core::skills::verdict_fields("challenged");
/// assert_eq!(f.len(), 2, "page_skills 加 hint: {f:?}");
/// assert!(browse_core::skills::verdict_fields("loading").is_empty());
/// ```
pub fn verdict_fields(verdict: &str) -> Vec<(&'static str, Value)> {
    if !page_skills_enabled() {
        return Vec::new();
    }
    page_skills_keys(verdict_page_slugs(verdict))
}

/// #63 多根域名层键对：序首命中段（有文件）的根整胜，未命中零键。
pub fn url_domain_fields_multi(
    roots: &[std::path::PathBuf],
    url: &str,
) -> Vec<(&'static str, Value)> {
    if !domain_skills_enabled() {
        return Vec::new();
    }
    match domain_segment(url) {
        Some(seg) => {
            let files = crate::workspace::domain_segment_files_multi(roots, &seg);
            if files.is_empty() {
                return Vec::new();
            }
            vec![
                ("domain_skills", json!(files)),
                ("domain_skills_hint", json!(domain_hint(&seg))),
            ]
        }
        None => Vec::new(),
    }
}

/// goto 回执的技能附加入口（crate 内缝，#50/#51/#56）：按开关分流各层；
/// 任何失败（目录列举失败、探测超时或求值错或解析错）静默返回，绝不
/// 让 goto 失败；未命中或关闭时一键不加（回执与现状逐字节一致）。
/// 调用点在 [`crate::semantic::goto`] 的回执构建处，elapsedMs 在此
/// 之前已固化（探测耗时不算进导航时长）。
pub(crate) async fn augment_goto(s: &std::sync::Arc<cdp::Session>, receipt: &mut Value) {
    let url = receipt
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    // #63 多根序：自定义仓（workspaces.json 配置）盖默认仓，序首命中
    let roots = crate::paths::workspace_roots();
    for (k, v) in url_domain_fields_multi(&roots, &url) {
        if let Some(obj) = receipt.as_object_mut() {
            obj.insert(k.to_string(), v);
        }
    }
    if page_skills_enabled() {
        // 双脚本对判定（清仓重建总台令 2026-10-09）：枚举各根
        // page-skills 成对目录的 @detect 块拼一次主世界求值（cdp::js
        // ——框架全局与元素 expando 只在页面世界可见），序首 _prelude.js
        // 供 facts；无对零键且不发起求值（内置大探针已随清仓退役），
        // 判据增改走 workspace 仓免 browse 发版；失败、环境错或超 8 秒界
        // 静默降级零键；白名单仍在本仓（页面可控输出经
        // FROZEN_PAGE_SLUGS 滤，信任模型不变）
        let probe = run_probe_rn(s).await;
        if let Some(p) = probe
            && let Some(obj) = receipt.as_object_mut()
        {
            for (k, v) in page_fields(&p) {
                obj.insert(k.to_string(), v);
            }
        }
    }
}

/// 跑特征判据 rn 脚本：回执 ok 值（{framework, slugs} 对象）或 None
/// （脚本缺失回缺省、编译/执行/环境错均静默降级——判据脚本属机制配方，
/// 坏脚本不许让 goto 失败）。VM 双限额与 30 秒单调用界走 rune 面缺省。
async fn run_probe_rn(s: &std::sync::Arc<cdp::Session>) -> Option<Value> {
    let roots = crate::paths::workspace_roots();
    let pairs = collect_pairs_multi(&roots);
    if pairs.iter().all(|p| p.detect_js.is_empty()) {
        // 无对（或全对无 @detect 块）：零键零求值——内置大探针已随清仓
        // 退役，不再对页面发起任何判定调用
        return None;
    }
    let prelude = load_prelude_multi(&roots);
    let batch = build_batch_js(&prelude, &pairs);
    // 8 秒外层界（评审 G-D，对齐旧探针界）：挂死页不再拖 goto 到 rune 面
    // 30 秒单调用界
    let probe = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        browse_rune::run(
            std::sync::Arc::clone(s),
            &wrap_probe_rn(&batch),
            browse_rune::DEFAULT_CAPS,
            browse_rune::RunOpts::default(),
        ),
    )
    .await;
    match probe {
        Ok(Ok(receipt)) => {
            // 脚本可见失败（评审 G-2）：main 返 Err 走 {"error": …} 回执面，
            // 与 run 层 Err 同款首败留痕（workspace 自定义判据的运行期失败
            // 不再完全无感），之后同样静默降级零键
            if receipt.get("error").is_some() {
                static WARNED_RUN: std::sync::OnceLock<()> = std::sync::OnceLock::new();
                if WARNED_RUN.set(()).is_ok() {
                    eprintln!(
                        "[browse] page 判据脚本可见失败（receipt.error；后续静默降级零键；检查 workspace 对应 probe.user.js 的 @detect 块）：{}",
                        serde_json::to_string(&receipt["error"]).unwrap_or_default()
                    );
                }
                return None;
            }
            receipt.get("ok").cloned()
        }
        Ok(Err(_e)) => {
            // 编译/VM/环境错（评审 G-C）：坏 workspace 脚本首败留痕，之后
            // 静默（判据脚本不许拖 goto，也不许刷日志）
            static WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            if WARNED.set(()).is_ok() {
                eprintln!(
                    "[browse] page 判据脚本执行失败（后续静默降级零键；检查 workspace 对应 probe.user.js 的 @detect 块）"
                );
            }
            None
        }
        Err(_) => {
            // 外层界丢弃在飞调用 future：内层清登记路径被一并丢弃，挂死页
            // 的 pending 登记成僵尸——32 秒后扫老项回收（评审 F-1 配套，
            // 正常调用早已应答出表）
            let ses = std::sync::Arc::clone(s);
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(32)).await;
                let n = ses
                    .reap_pending_older_than(std::time::Duration::from_secs(30))
                    .await;
                if n > 0 {
                    eprintln!("[browse] page 判据超时回收 pending 僵尸 {n} 条");
                }
            });
            None
        }
    }
}

/// 把拼批判定 JS 包成 rune 体（`cdp::js` 走**主世界**：框架全局与元素
/// expando 只在页面世界可见，评审 F-1 律沿承）。定界器按正文里最长 `#`
/// 连跑升阶（workspace 提供的正文是运行期数据，不设编译期断言——升阶
/// 定界对任意正文安全）。
fn wrap_probe_rn(js: &str) -> String {
    let hashes = "#".repeat(max_hash_run(js) + 1);
    format!(
        "// 双脚本对拼批判定（workspace page-skills 各对 @detect 块）\n// 体走主世界（cdp::js）：框架全局与元素 expando 只在页面世界可见\npub async fn main() {{\n    let v = cdp::js(r{h}\"return {js}\"{h}, #{{}}).await?;\n    Ok(v)\n}}",
        h = hashes
    )
}

/// 正文里最长 `#` 连跑长度（`wrap_probe_rn` 的定界器升阶依据）。
fn max_hash_run(s: &str) -> usize {
    let (mut best, mut cur) = (0usize, 0usize);
    for ch in s.chars() {
        if ch == '#' {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// is_off：0/false 关，未设、1、任意串都开。
    #[test]
    fn switch_off_values() {
        assert!(is_off(Some("0".into())));
        assert!(is_off(Some("false".into())));
        assert!(is_off(Some("FALSE".into())));
        assert!(!is_off(None));
        assert!(!is_off(Some("1".into())));
        assert!(!is_off(Some("off".into())));
    }

    /// domain_segment：命中、去 www.、首段、大小写、非 http(s)。
    #[test]
    fn segment_extraction() {
        assert_eq!(domain_segment("https://x.com/a"), Some("x".into()));
        assert_eq!(
            domain_segment("http://www.github.com/"),
            Some("github".into())
        );
        assert_eq!(
            domain_segment("https://news.ycombinator.com/item?id=1"),
            Some("news".into())
        );
        assert_eq!(domain_segment("https://X.COM"), Some("x".into()));
        assert_eq!(domain_segment("data:text/html,<p>x</p>"), None);
        assert_eq!(domain_segment("about:blank"), None);
        assert_eq!(domain_segment("https://"), None);
    }

    /// list_domain_skills：成对才有、排序稳定。
    #[test]
    fn listing_pairs_only() {
        let dir = std::env::temp_dir().join(format!("browse-sk-{}", std::process::id()));
        let seg = dir.join("domain-skills").join("x");
        std::fs::create_dir_all(&seg).unwrap();
        // 半对不算
        std::fs::write(seg.join("probe.user.js"), "// x").unwrap();
        assert!(list_domain_skills(&dir, "x").is_empty(), "不成对零清单");
        // 成对即列三件(或两件)
        std::fs::write(seg.join("mech.rn"), "pub async fn main() {}").unwrap();
        let files = list_domain_skills(&dir, "x");
        assert_eq!(
            files,
            vec!["mech.rn".to_string(), "probe.user.js".to_string()],
            "{files:?}"
        );
        assert!(list_domain_skills(&dir, "nope").is_empty(), "缺段空表");
        let _ = std::fs::remove_dir_all(&dir);
    }
    /// 双脚本对枚举：成对才算、序首根整对胜、无 @detect 块对保留但
    /// detect_js 空、_prelude 序首命中。
    #[test]
    fn pair_enumeration_first_root_and_completeness() {
        let dir = std::env::temp_dir().join(format!("browse-pairs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let a = dir.join("a");
        let b = dir.join("b");
        // 根 a:完整对(带 detect 块) + 半对(缺 mech.rn,不算)
        let ca = a.join("page-skills").join("challenge-stop");
        std::fs::create_dir_all(&ca).unwrap();
        std::fs::write(ca.join("mech.rn"), "pub async fn main() {}").unwrap();
        std::fs::write(
            ca.join("probe.user.js"),
            "/* @detect */\nfunction browseDetect(facts) {\n  return { slug: 'challenge-stop', confidence: 'CONFIRMED' };\n}\n/* @enddetect */\n",
        )
        .unwrap();
        let half = a.join("page-skills").join("half-pair");
        std::fs::create_dir_all(&half).unwrap();
        std::fs::write(half.join("probe.user.js"), "// 只有探针没有编排").unwrap();
        // 根 a 的 _prelude
        std::fs::write(
            a.join("page-skills").join("_prelude.js"),
            "facts.framework = { name: 'react', version: null };",
        )
        .unwrap();
        // 根 b:同名对(detect 块不同,应被序首根 a 遮住) + 独立对
        let cb = b.join("page-skills").join("challenge-stop");
        std::fs::create_dir_all(&cb).unwrap();
        std::fs::write(cb.join("mech.rn"), "// b 版").unwrap();
        std::fs::write(
            cb.join("probe.user.js"),
            "/* @detect */ function browseDetect(f) {} /* @enddetect */",
        )
        .unwrap();
        let nb = b.join("page-skills").join("antibot-vendor");
        std::fs::create_dir_all(&nb).unwrap();
        std::fs::write(nb.join("mech.rn"), "// x").unwrap();
        // 无 detect 块的完整对
        std::fs::write(nb.join("probe.user.js"), "// 常驻监听型,无判定块").unwrap();

        let roots = vec![a.clone(), b.clone()];
        let pairs = collect_pairs_multi(&roots);
        let slugs: Vec<&str> = pairs.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs, vec!["antibot-vendor", "challenge-stop"], "{slugs:?}");
        let cs = pairs.iter().find(|p| p.slug == "challenge-stop").unwrap();
        assert!(
            cs.detect_js.contains("CONFIRMED"),
            "序首根 a 的块胜出: {}",
            cs.detect_js
        );
        assert!(
            pairs
                .iter()
                .find(|p| p.slug == "antibot-vendor")
                .unwrap()
                .detect_js
                .is_empty(),
            "无块对保留但空"
        );
        // prelude 序首命中
        assert_eq!(
            load_prelude_multi(&roots),
            "facts.framework = { name: 'react', version: null };"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 拼批与包裹：批含 prelude 与各块、块 IIFE 自调、定界器随 `#` 连跑
    /// 升阶;白名单过滤(page_fields)仍拒名单外 slug。
    #[test]
    fn batch_build_and_wrap_escalation() {
        let pairs = vec![
            SkillPair { slug: "a".into(), detect_js: "function browseDetect(facts) { return facts.x ? { slug: 'challenge-stop', confidence: 'CONFIRMED' } : null; }".into() },
        ];
        let batch = build_batch_js("facts.x = 1;", &pairs);
        assert!(batch.starts_with("(() => {"), "{batch}");
        assert!(batch.contains("facts.x = 1;"), "prelude 在批内");
        assert!(batch.contains("browseDetect(facts)"), "块自调约定函数");
        assert!(batch.ends_with("})()"), "{batch}");
        // 定界器升阶:含 "## 的正文用 r### 包(正文是裸表达式,return 由包裹层补)
        let hot = "\"##\" + 'x';";
        let wrapped = wrap_probe_rn(hot);
        assert!(wrapped.contains("r###\""), "升阶定界: {wrapped}");
        assert!(
            wrapped.contains("return \"##\" + 'x';"),
            "return 前缀恰好一层: {wrapped}"
        );
        assert!(wrapped.contains("pub async fn main"), "{wrapped}");
        // 白名单硬边界沿承:批输出的 slug 仍过 page_fields 滤
        let probe = serde_json::json!({
            "framework": null,
            "slugs": [
                {"slug": "challenge-stop", "confidence": "CONFIRMED"},
                {"slug": "spa", "confidence": "CONFIRMED"}
            ]
        });
        let f = page_fields(&probe);
        let skills = f.iter().find(|(k, _)| *k == "page_skills").unwrap();
        assert_eq!(
            skills.1.as_array().unwrap().len(),
            1,
            "旧名 spa 被滤: {skills:?}"
        );
    }

    /// page_fields：空骨架零键；framework 单独可出；slugs 出两键带命令。
    #[test]
    fn probe_fields_shapes() {
        let empty = serde_json::json!({"framework": null, "slugs": []});
        assert!(page_fields(&empty).is_empty(), "空骨架零键");

        let fw_only =
            serde_json::json!({"framework": {"name": "react", "version": null}, "slugs": []});
        let f = page_fields(&fw_only);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].0, "framework");

        let full = serde_json::json!({
            "framework": {"name": "vue", "version": "3.5"},
            "slugs": [
                {"slug": "next-page", "confidence": "CONFIRMED"},
                {"slug": "policy-surface", "confidence": "CONFIRMED"}
            ]
        });
        let f = page_fields(&full);
        assert_eq!(f.len(), 3, "framework 加 page_skills 加 hint: {f:?}");
        let hint = f.iter().find(|(k, _)| *k == "page_skills_hint").unwrap();
        assert!(
            hint.1
                .as_str()
                .unwrap_or("")
                .contains("browse workspace page next-page")
                && hint.1.as_str().unwrap_or("").contains("；"),
            "hint 是读全文命令的连接: {hint:?}"
        );
    }

    /// #56 判读映射表：命中判读的 slug 全在冻结名单内（表驱动静态对
    /// 不越白名单的零漂移守卫），未映射判读返回空。
    #[test]
    fn verdict_mapping_whitelisted() {
        for verdict in [
            "challenged",
            "rate-limited",
            "blocked",
            "stalled",
            "login-wall",
            "blank",
            "loading",
            "ok",
            "nonsense",
        ] {
            for (slug, conf) in verdict_page_slugs(verdict) {
                assert!(
                    FROZEN_PAGE_SLUGS.contains(slug),
                    "判读 {verdict} 的 slug {slug} 不在冻结名单"
                );
                assert!(
                    matches!(*conf, "CONFIRMED" | "PLAUSIBLE"),
                    "判读 {verdict} 的 {slug} 置信档非法: {conf}"
                );
            }
        }
        assert!(verdict_page_slugs("rate-limited").is_empty());
        assert!(verdict_page_slugs("ok").is_empty());
    }

    /// #56 url_domain_fields：命中出两键（清单与 goto 同口径），未命中
    /// 站点与非 http(s) 形零键。
    #[test]
    fn url_domain_fields_shapes() {
        let dir = std::env::temp_dir().join(format!("browse-sk56-{}", std::process::id()));
        let seg = dir.join("domain-skills").join("x");
        std::fs::create_dir_all(&seg).unwrap();
        std::fs::write(seg.join("mech.rn"), "pub async fn main() {}").unwrap();
        std::fs::write(seg.join("probe.user.js"), "// probe").unwrap();
        let f = url_domain_fields(&dir, "https://www.x.com/a");
        assert_eq!(f.len(), 2, "命中出 domain_skills 加 hint: {f:?}");
        assert_eq!(f[0].0, "domain_skills");
        assert_eq!(
            f[0].1,
            json!(["mech.rn", "probe.user.js"]),
            "清单与 goto 同口径（对文件）"
        );
        assert!(
            f[1].1
                .as_str()
                .is_some_and(|h| h == "browse workspace site x"),
            "hint 是读全文命令: {f:?}"
        );
        assert!(
            url_domain_fields(&dir, "https://nope.com/").is_empty(),
            "未知识站点零键"
        );
        assert!(
            url_domain_fields(&dir, "data:text/html,x").is_empty(),
            "非 http(s) 零键"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
