//! CLI argv 契约锁（#53）：0.12.1 的 G-F 收口把 `next` 必值口改成直出
//! exit 2 后，子命令旗标收集循环按「参数尽返 Err 收尾」旧契约写，全部
//! 假报用法错（`fetch` / `userscript` 族等收集环）。
//! 本件用真二进制（`CARGO_BIN_EXE_browse`）锁双口分面：必值口缺值
//! exit 2，收集口参数尽即收尾。全部用例零网络（本地校验面）。
//! 注：issue/artifact/ledger 三族已移除（总台令 2026-10-21 ledger 全线
//! 淘汰），其用例随之退场，收集环契约由存活子命令续锁。

use std::process::{Command, Stdio};

/// 跑真二进制：stdin 恒 null（读管道的子命令会挂住测试）。
fn run(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_browse"));
    c.args(args).stdin(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().expect("browse 二进制在位");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// BROWSE_NAME 传绝对路径即整仓 state_dir（G7 口径）：测试态隔离不碰
/// 本机 ~/.browse-rs（keygen 会写真密钥、workspace 读真仓）。Drop 收渣
/// （#53 评审 G-lite：反复跑不留临时目录）。
struct TempState(String);

impl TempState {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!(
            "browse-arg-contract-{}-{tag}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&d).expect("建临时 state 目录");
        Self(d.to_string_lossy().into_owned())
    }

    fn path(&self) -> &str {
        &self.0
    }
}

impl Drop for TempState {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 旗标收集循环（含 `while let Some` 与 `loop { match }` 两形态）与
/// 可选位全部正常收尾，不再假报「旗标 需要一个值」。
#[test]
fn flag_collection_loops_terminate() {
    // fetch 的循环首迭代（坏旗标在网络前拦截）
    let (code, _, err) = run(&["fetch", "x", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");

    // userscript 族收集环（多 id 收集形态）：坏旗标用法错点名收到的旗标
    let (code, _, err) = run(&["userscript", "enable", "x", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("只收 id/名与 --restart"), "{err}");
    assert!(err.contains("--bogus"), "报错应点名旗标：{err}");
}

/// 必值口（Args::next）缺值仍直出 exit 2——分面只救收集口，不放松守卫。
#[test]
fn required_value_face_still_exits_2() {
    let (code, _, err) = run(&["--eval"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--eval 需要一个值"), "{err}");

    let (code, _, err) = run(&["fetch"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("fetch <url> 需要一个值"), "{err}");
}

/// G-F 的 Mode 层缺参处理器复活（0.12.1 里被解析层假报先拦成死代码）。
#[test]
fn mode_level_missing_param_reachable() {
    let (code, _, err) = run(&["workspace", "site"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("workspace site 缺段"), "{err}");
}

/// bail_arg 措辞中性化：坏旗标报错点名收到的旗标与本子命令，不误报到
/// 别的子命令头上。
#[test]
fn bail_arg_wording_is_subcommand_neutral() {
    let (code, _, err) = run(&["userscript", "disable", "id-x", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("userscript disable"),
        "报错应点名本子命令：{err}"
    );
    assert!(err.contains("--bogus"), "报错应点名旗标：{err}");
}

/// #64 `@` 文件实参形态：读失败报错带 CTA 直出 exit 2（网络前拦截，
/// 零 daemon）；方言与 `--js` 两形态通用。
#[test]
fn at_file_arg_read_failure_exits_2_with_cta() {
    let (code, _, err) = run(&["--js", "@/no/such/browse-contract.js"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("@实参读文件失败"), "{err}");
    assert!(err.contains("下一步"), "CTA 应带下一步：{err}");
    assert!(
        err.contains("/no/such/browse-contract.js"),
        "报错应点名路径：{err}"
    );

    // 方言形态（无 --js）同款
    let (code, _, err) = run(&["-e", "@/no/such/browse-contract.js"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("@实参读文件失败"), "{err}");
}

/// #64 共存形：argv 上有 `@` 实参时管道不被消费（`@` 错误先于 stdin
/// 形态命中），两道不互踩；管道数据不落入 `@` 展开道。
#[test]
fn at_file_arg_coexists_with_stdin_pipe() {
    let mut c = Command::new(env!("CARGO_BIN_EXE_browse"));
    // stderr 须显式 piped：spawn 后的 wait_with_output 只回收自管道来的
    // 句柄，继承态收不到子进程报错
    c.args(["--js", "@/no/such/browse-contract.js"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = c.spawn().expect("起 browse");
    use std::io::Write as _;
    if let Some(mut stdin) = child.stdin.take() {
        // 喂了管道也不抢道：argv @ 展开失败先行退出，管道形不触发
        let _ = stdin.write_all(b"return 999");
    }
    let out = child.wait_with_output().expect("收 browse");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(
        err.contains("@实参读文件失败"),
        "argv @ 先于管道形态：{err}"
    );
}

/// #66 模板道契约：--vars 形状与激活规则（只配 @ 形态、键=值 形）、
/// 未填键网络前拦 exit 2 点名、根第一等解析序（库内与 cwd 同名时库内
/// 胜出）、渲染成臂经 --ws 死口证（填充毕进求值道，连接拒零 daemon）；
/// 不给 --vars 的 @ 实参走 #64 纯透传（既有 #64 用例即零回退锁）。
#[test]
fn template_vars_arg_contract() {
    // 激活与形状：网络前拦 exit 2
    let (code, _, err) = run(&["--js", "return 1", "--vars", "n=1"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--vars 只配"), "{err}");
    let (code, _, err) = run(&["--js", "@/no/such/x", "--vars", "nokey"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--vars 要 键=值"), "{err}");

    let st = TempState::new("tmpl");
    let base = std::path::Path::new(st.path());
    let root = base.join("root");
    std::fs::create_dir_all(&root).expect("建根");

    // 未填键：exit 2、点名全部未填键、已给键不点名（附着证明）、零 daemon
    std::fs::write(root.join("miss.js"), "return <%= alpha %> + <%= beta %>\n").expect("写模板");
    let (code, _, err) = run(
        &[
            "--js",
            "@miss",
            "--root",
            &root.to_string_lossy(),
            "--vars",
            "alpha=1",
        ],
        &[],
    );
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("模板未填占位符"), "{err}");
    assert!(err.contains("beta"), "未填键点名：{err}");
    assert!(!err.contains("alpha"), "已给键不点名：{err}");

    // 根第一等：库内与 cwd 同名（库内含未填占位符、cwd 是合法正文），
    // 命中库内即模板未填错；若字面道先解析则会走到引擎连接（可判别）
    std::fs::write(root.join("dup.js"), "<%= boom %>\n").expect("写库内同名");
    let cwd = base.join("cwd");
    std::fs::create_dir_all(&cwd).expect("建 cwd 位");
    std::fs::write(cwd.join("dup.js"), "return 1\n").expect("写 cwd 同名");
    let mut c = Command::new(env!("CARGO_BIN_EXE_browse"));
    c.args([
        "--js",
        "@dup",
        "--vars",
        "x=1",
        "--root",
        &root.to_string_lossy(),
    ])
    .current_dir(&cwd)
    .stdin(Stdio::null());
    let out = c.output().expect("跑根序判别");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(2),
        "根先解析应命中库内未填错：{err}"
    );
    assert!(
        err.contains("模板未填占位符") && err.contains("boom"),
        "{err}"
    );

    // 渲染成臂：全键填齐，--ws 死口证填充毕（模板错误不会出现，连接拒
    // 即到达求值道；显式 ws 意图不拉 daemon，零 spawn）
    std::fs::write(root.join("ok.js"), "return {{ n }} * 7\n").expect("写模板");
    let (code, _, err) = run(
        &[
            "--js",
            "@ok",
            "--root",
            &root.to_string_lossy(),
            "--vars",
            "n=6",
            "--ws",
            "ws://127.0.0.1:1",
        ],
        &[],
    );
    assert_eq!(code, 1, "死口连接拒属执行失败非模板错：{err}");
    assert!(
        err.contains("Connection refused")
            || err.contains("积极拒绝")
            || err.contains("os error 10061")
            // 死口在本机形可能是黑洞(镜像网火墙丢包不 RST):#68 的 15 秒
            // 握手界超时同样证明填充毕、已进连接求值道(非模板错)
            || err.contains("握手超时"),
        "填充毕进求值道：{err}"
    );
    assert!(!err.contains("模板"), "模板面应已通过：{err}");
}

/// 评审 #66 一轮采纳面：G1 守卫（--root 求值面漏 --vars 显式用法错，
/// 不再静默失效被误读成文件不存在）；五核一直断言（字面大括号文件 +
/// 不给 --vars 原样透传，落 --b64 字母表校验报错且原文可见——不经
/// 模板道的直接证据，不再靠推论）。
#[test]
fn template_g1_guard_and_literal_passthrough() {
    let st = TempState::new("g1");
    let base = std::path::Path::new(st.path());
    let root = base.join("root");
    std::fs::create_dir_all(&root).expect("建根");
    std::fs::write(root.join("t.js"), "return 1\n").expect("写模板");

    // G1：--root 给了、--vars 没给、非 snippets 子命令 = 用法错 exit 2
    let (code, _, err) = run(&["--js", "@t", "--root", &root.to_string_lossy()], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--root 在求值面只配"), "{err}");

    // 字面大括号 + 不给 --vars（无 --root 避 G1）：原文透传落 b64 校验
    // （判别面：报 b64 字母表错而非「模板未填占位符」，即未过模板道）
    let lit = base.join("lit.js");
    std::fs::write(&lit, "{{ x }}{{ y }}\n").expect("写字面文件");
    let (code, _, err) = run(&["--js", &format!("@{}", lit.display()), "--b64"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("不是 base64 字母表"), "透传落 b64 校验：{err}");
    assert!(!err.contains("模板"), "不经模板道：{err}");
}

/// #67 rune 子命令 argv 契约：缺实参 exit 2；@ 名未命中列候选（零
/// daemon，客户端解析期拦）；--vars 坏形与模板未填键同 #66 道（零
/// daemon 前拦 exit 2 点名）。执行正路走 e2e（真 chrome）。
#[test]
fn rune_arg_contract() {
    let (code, _, err) = run(&["rune"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("rune 缺脚本实参"), "{err}");

    let (code, _, err) = run(&["rune", "@/no/such/script"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no/such/script 未命中"), "{err}");

    let st = TempState::new("rune");
    let f = std::path::Path::new(st.path()).join("t.rn");
    std::fs::write(&f, "pub async fn main() { Ok(1) }\n").expect("写脚本");
    let (code, _, err) = run(&["rune", &f.to_string_lossy(), "--vars", "nokey"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--vars 要 键=值"), "{err}");

    std::fs::write(&f, "pub async fn main() { Ok(\"<%= alpha %>\") }\n").expect("写模板脚本");
    let (code, _, err) = run(&["rune", &f.to_string_lossy(), "--vars", "x=1"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("模板未填占位符") && err.contains("alpha"),
        "{err}"
    );

    // 评审 #67 一轮 G4：未知旗标 bail、多余脚本位用法错（含脚本位后
    // --vars 正常形的对照——内环代收不误伤）
    let (code, _, err) = run(&["rune", "--bogus", "a.rn"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");
    let (code, _, err) = run(&["rune", "a.rn", "b.rn"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("只要一个脚本位"), "{err}");
    let (code, _, err) = run(&["rune", &f.to_string_lossy(), "--vars", "x=1"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("模板未填占位符"),
        "脚本位后的 --vars 正常收（不走 bail）：{err}"
    );
}

/// 尾置合法批（R4 上游缺陷 b 修复）：--llms 手册承诺「全局旗标尾置合法
/// （无关子命令静默忽略）」，实装对齐——尾置全局旗标静默收（不再 exit
/// 2），未知旗标保持原 bail 面。
#[test]
fn tail_global_flag_accepted() {
    // 布尔型尾置静默收：chrome doctor --json 正常出 JSON（exit 0）
    let (code, out, err) = run(&["chrome", "doctor", "--json"], &[]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("\"healthy\""), "{out}");
    // rune 环同律：布尔尾置过（脚本位后的 --headless 不再拒）
    let (code, _, err) = run(&["rune", "x.rn", "--headless"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(
        !err.contains("是全局旗标，要置于子命令与脚本位之前"),
        "尾置全局旗标不应再报位置错：{err}"
    );
    // 未知旗标保持原面（不误升位置 CTA）
    let (code, _, err) = run(&["rune", "x.rn", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");
}

/// 尾置合法批：取值型尾置全局旗标连值收（值不落成位置位），且静默不改
/// 子命令形态。
#[test]
fn tail_value_flag_consumes_its_value() {
    // events 环：--last 尾置后再尾置 --secrets f.toml，值被吃掉不落
    // 「--secrets 不是实参」
    let (code, _, err) = run(&["events", "--last", "1", "--secrets", "f.toml"], &[]);
    assert_eq!(code, 0, "{err}");
    // rune 环：--chrome 的值被全局道吃掉——若没吃，foo 会落「只要一个
    // 脚本位」位错；吃对了走脚本读腿（x.rn 不在，读失败）
    let (code, _, err) = run(&["rune", "x.rn", "--chrome", "foo"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("读脚本失败"), "{err}");
    assert!(!err.contains("只要一个脚本位"), "{err}");
}

/// 目录审计 G-A7（尾置合法批改版）：名册旗标尾置的旁证——每个名册旗标
/// 尾置都走全局道（布尔型静默收后落脚本读腿；取值型裸名落「需要一个
/// 值」；-h/-V/--llms 直出 exit 0），绝不允许「参数不认识」（名册漏同步
/// 解析臂时本测试红）。未知旗标走原 bail 面（对照）。注：--vars/--root/
/// --timeout 在 rune 环内是合法子旗标（消费值，REQ-027 评审 F-2 内环
/// 代收），不属尾置面；--serve 尾置与前置同语义会真起 daemon（绑定副
/// 作用），不入本守卫名单。
#[test]
fn roster_flags_all_recognized_in_rune_tail_position() {
    for f in [
        "--eval",
        "-e",
        "--gen-surface",
        "--bind",
        "--connect",
        "--new-tab",
        "--ws",
        "--port",
        "--chrome",
        "--profile",
        "--headless",
        "--pipe",
        "--proxy",
        "--proxy-bypass",
        "--engine-arg",
        "--isolated",
        "--idle-timeout",
        "--cookies",
        "--secrets",
        "--js",
        "--b64",
        "--json",
        "--llms",
        "--repl",
        "--full",
        "-h",
        "--help",
        "-V",
        "--version",
    ] {
        let (code, _, err) = run(&["rune", "x.rn", f], &[]);
        assert!(code == 0 || code == 2, "旗标 {f}：{err}");
        assert!(
            !err.contains("参数不认识"),
            "旗标 {f} 未被全局道识别（名册与解析臂漂移）：{err}"
        );
        if code == 2 {
            // 合法 exit 2 只有三种：取值型裸名缺值（需要一个值）；布尔型
            // 静默收后脚本读腿失败（读脚本失败）；rune 面的语义级拒收
            // （--new-tab/--cookies 前置同拒，非位置错）
            assert!(
                err.contains("需要一个值")
                    || err.contains("读脚本失败")
                    || err.contains("尚不支持 rune 面"),
                "旗标 {f} 的 exit 2 来路不明：{err}"
            );
        }
    }
}

/// 目录审计 G-2 收口（尾置合法批改版）：全局旗标臂的真源已是
/// `Globals::apply` 的 match（主环与子命令内环同一道），单源无第二真
/// 相——本守卫刮 apply 臂键做存在性与计数检查：名单里每个旗标都在臂
/// 内（新旗标漏进 apply 时名单先红），臂键数与名单一致（多出的未登记
/// 旗标红）。
#[test]
fn global_arms_cover_guard_roster() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("读 main.rs 源");
    let apply_start = src
        .find("fn apply(&mut self, a: &str, args: &mut Args) -> bool {")
        .expect("apply 在位");
    let apply_end = src[apply_start..]
        .find(
            "
    }
}

#[tokio::main]",
        )
        .map(|i| i + apply_start)
        .expect("apply 闭合");
    let seg = &src[apply_start..apply_end];
    // 臂键（8 空格缩进 + 引号开头 + =>；fn 体内 match 臂的 fmt 稳定缩进）
    let mut arms: Vec<String> = Vec::new();
    for line in seg.lines() {
        let trimmed = line.trim_start();
        if line.len() - trimmed.len() == 12 && trimmed.starts_with('"') && line.contains("=>") {
            for part in trimmed.split("=>").next().unwrap_or("").split('"') {
                if part.starts_with('-') {
                    arms.push(part.to_string());
                }
            }
        }
    }
    for f in [
        "--eval",
        "-e",
        "--gen-surface",
        "--serve",
        "--bind",
        "--connect",
        "--new-tab",
        "--ws",
        "--port",
        "--chrome",
        "--timeout",
        "--profile",
        "--headless",
        "--pipe",
        "--proxy",
        "--proxy-bypass",
        "--engine-arg",
        "--isolated",
        "--idle-timeout",
        "--cookies",
        "--secrets",
        "--js",
        "--vars",
        "--root",
        "--vars-syntax",
        "--b64",
        "--json",
        "--llms",
        "--repl",
        "--full",
        "-h",
        "--help",
        "-V",
        "--version",
    ] {
        assert!(
            arms.contains(&f.to_string()),
            "全局旗标 {f} 不在 Globals::apply 臂（名单与解析道漂移）"
        );
    }
    // 计数守卫：apply 臂键全在名单（多出的未登记旗标红）
    assert_eq!(
        arms.len(),
        34,
        "apply 臂键数变化（{arms:?}）：新旗标须同步本守卫名单与文档面"
    );
}

#[test]
fn cdp_methods_cli_lists_without_engine() {
    let (code, out, err) = run(&["cdpMethods"], &[]);
    assert_eq!(code, 0, "browse cdpMethods：{err}");
    assert!(out.contains("Page.navigate"), "{out}");
    assert!(
        out.contains("Fetch.enable"),
        "Fetch 域应在原生 CDP 清单：{out}"
    );
    let (code, out, err) = run(&["cdpMethods", "Fetch"], &[]);
    assert_eq!(code, 0, "browse cdpMethods Fetch：{err}");
    assert!(out.contains("Fetch.enable"), "{out}");
    assert!(!out.contains("Page.navigate"), "{out}");
    let (code, _, err) = run(&["cdp", "methods", "Network"], &[]);
    assert_eq!(code, 0, "browse cdp methods：{err}");
}
