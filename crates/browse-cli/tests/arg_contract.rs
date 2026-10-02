//! CLI argv 契约锁（#53）：0.12.1 的 G-F 收口把 `next` 必值口改成直出
//! exit 2 后，七处子命令旗标收集循环按「参数尽返 Err 收尾」旧契约写，
//! 全部假报用法错（`fetch` / `artifact` 三兄弟 / `ledger keygen` /
//! `issue new` / `issue list`），`snippets list` 的可选位也一并踩碎。
//! 本件用真二进制（`CARGO_BIN_EXE_browse`）锁双口分面：必值口缺值
//! exit 2，收集口参数尽即收尾。全部用例零网络（dry-run 与本地校验面）。

use std::process::{Command, Stdio};

/// 跑真二进制：stdin 恒 null（issue new 的空 body 会读管道，挂住测试）。
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
/// 本机 ~/.browse-rs（keygen 会写真密钥、snippets 读真库）。Drop 收渣
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

/// 七处旗标收集循环（含 `while let Some` 与 `loop { match }` 两形态）与
/// 可选位全部正常收尾，不再假报「旗标 需要一个值」。
#[test]
fn flag_collection_loops_terminate() {
    // issue new：循环走完进 dry-run 本地预览（零网络零签名）
    let (code, out, err) = run(
        &["issue", "new", "t", "--acceptance", "a", "--dry-run"],
        &[],
    );
    assert_eq!(code, 0, "issue new dry-run 应 0：{err}");
    assert!(out.contains("\"dryRun\": true"), "dry-run 回执：{out}");

    // artifact publish：循环走完命中 --kind 必填校验（非旗标缺值假报）
    let (code, _, err) = run(&["artifact", "publish", "--name", "x"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--kind 必填"), "应命中 kind 校验：{err}");

    // snippets list 裸调：可选位 [site] 缺省列全库（隔离态空库，空库提
    // 示在 stderr）
    let snips = TempState::new("snips");
    let (code, _, err) = run(&["snippets", "list"], &[("BROWSE_NAME", snips.path())]);
    assert_eq!(code, 0, "裸 snippets list 应 0");
    assert!(err.contains("片段库为空"), "空库回执：{err}");

    // ledger keygen 裸调：循环收尾后真落钥（隔离态，勿碰本机密档）
    let key = TempState::new("key");
    let (code, out, err) = run(&["ledger", "keygen"], &[("BROWSE_NAME", key.path())]);
    assert_eq!(code, 0, "keygen 应 0：{err}");
    assert!(out.contains("\"ok\": true"), "keygen 回执：{out}");

    // fetch / artifact attest / issue list 的循环首迭代（坏旗标与坏值都
    // 在网络前拦截）
    let (code, _, err) = run(&["fetch", "x", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");
    let (code, _, err) = run(&["issue", "list", "--limit", "abc"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--limit 要数字"), "{err}");

    // artifact publish 的 outcome 枚举（结构化字段，网络前拦截）
    let (code, _, err) = run(
        &[
            "artifact",
            "publish",
            "--name",
            "x",
            "--kind",
            "lesson",
            "--digest",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "--outcome",
            "bogus",
        ],
        &[],
    );
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--outcome 只认 success|failure"), "{err}");
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

/// G-F 的 Mode 层缺参处理器复活（0.12.1 里被解析层假报先拦成死代码）；
/// snippets show 从无守卫（#53 评审 F1），本批补齐同款。
#[test]
fn mode_level_missing_param_reachable() {
    let (code, _, err) = run(&["workspace", "site"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("workspace site 缺段"), "{err}");

    let (code, _, err) = run(&["snippets", "show"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("snippets show 缺 rel"), "{err}");
}

/// bail_arg 措辞中性化：非 issue 子命令的坏旗标不再误报「issue 参数不
/// 认识」。
#[test]
fn bail_arg_wording_is_subcommand_neutral() {
    let (code, _, err) = run(&["artifact", "attest", "id-x", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");
    assert!(!err.contains("issue 参数不认识"), "{err}");
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

/// #65 片段库根三序（旗标 > env > 内置缺省）与越根拦截：
/// `--root` 生效、env 生效、旗标盖过 env、无配置仍读内置
/// `<state_dir>/snippets`、越出配置根的相对/绝对路径仍拦（exit 1）。
#[test]
fn snippets_root_flag_env_default_and_escape_guard() {
    let st = TempState::new("root");
    let root = std::path::Path::new(st.path()).join("root");
    let site = root.join("x.test");
    std::fs::create_dir_all(&site).expect("建站点目录");
    std::fs::write(
        site.join("title.js"),
        "// 用途：取标题\nreturn document.title\n",
    )
    .expect("写片段");
    // 越根探针：库根外的真文件
    std::fs::write(
        std::path::Path::new(st.path()).join("outside.js"),
        "// 库外文件\n",
    )
    .expect("写库外探针");

    // 旗标根生效
    let (code, out, err) = run(
        &["snippets", "list", "--root", &root.to_string_lossy()],
        &[],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("x.test/title.js"), "旗标根应列出片段：{out}");
    assert!(out.contains("取标题"), "首行 // 摘要约定不变：{out}");

    // env 根生效（show 读全文）
    let (code, out, err) = run(
        &["snippets", "show", "x.test/title.js"],
        &[("BROWSE_SNIPPETS_ROOT", &root.to_string_lossy())],
    );
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("return document.title"),
        "env 根应读出全文：{out}"
    );

    // 旗标 > env：env 指空目录、旗标指真库，以旗标为准
    let empty = std::path::Path::new(st.path()).join("empty-root");
    std::fs::create_dir_all(&empty).expect("建空根");
    let (code, out, err) = run(
        &["snippets", "list", "--root", &root.to_string_lossy()],
        &[("BROWSE_SNIPPETS_ROOT", &empty.to_string_lossy())],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("x.test/title.js"), "旗标应盖过 env：{out}");

    // 越根拦截：相对 ../ 与绝对路径两形（exit 1，CTA 在 stderr）
    let (code, _, err) = run(
        &["snippets", "show", "../outside.js"],
        &[("BROWSE_SNIPPETS_ROOT", &root.to_string_lossy())],
    );
    assert_eq!(code, 1, "越根应拦（exit 1）：{err}");
    assert!(err.contains("越出片段库"), "{err}");
    let outside = std::path::Path::new(st.path()).join("outside.js");
    let (code, _, err) = run(
        &["snippets", "show", &outside.to_string_lossy()],
        &[("BROWSE_SNIPPETS_ROOT", &root.to_string_lossy())],
    );
    assert_eq!(code, 1, "绝对路径越根应拦：{err}");
    assert!(err.contains("越出片段库"), "{err}");

    // 缺省不变：无 env 无旗标仍读内置 <state_dir>/snippets
    let builtin = std::path::Path::new(st.path())
        .join("snippets")
        .join("y.site");
    std::fs::create_dir_all(&builtin).expect("建内置库目录");
    std::fs::write(builtin.join("task.js"), "// 用途：内置\nreturn 1\n").expect("写内置片段");
    let mut c = Command::new(env!("CARGO_BIN_EXE_browse"));
    c.args(["snippets", "list"])
        .stdin(Stdio::null())
        .env_remove("BROWSE_SNIPPETS_ROOT")
        .env("BROWSE_NAME", st.path());
    let out = c.output().expect("跑内置缺省形");
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("y.site/task.js"), "缺省应读内置库：{text}");
    assert!(
        !text.contains("x.test/title.js"),
        "外部根不应泄进缺省形：{text}"
    );
}

/// 评审一轮 F1/G1/G2/G5 修复锁：snippets 族坏旗标与多余位置位回 bail
/// exit 2（基线契约复位）；显式配置根不存在报配置错 exit 1（拼错根与
/// 真空库可分），内置缺省根不在册维持空库引导面（#44 契约）；裸 @
/// 出「@ 后缺文件路径」。
#[test]
fn snippets_arg_face_after_review_round1() {
    // F1：未知旗标（list 与 show 两臂）与 --root= 形回 bail exit 2
    let (code, _, err) = run(&["snippets", "list", "x", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");
    let (code, _, err) = run(&["snippets", "show", "x.js", "--root="], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --root="), "{err}");

    // G1：多余位置位用法错（首位置位胜出语义的另一半）
    let (code, _, err) = run(&["snippets", "list", "a", "b"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("site 过滤位"), "{err}");
    let (code, _, err) = run(&["snippets", "show", "a.js", "b.js"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("rel 位"), "{err}");

    // G2：显式根（env 形）不存在报配置错 exit 1；旗标形同款
    let (code, _, err) = run(
        &["snippets", "list"],
        &[("BROWSE_SNIPPETS_ROOT", "/no/such/root")],
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("片段库目录不存在"), "{err}");
    assert!(err.contains("核对 --root / BROWSE_SNIPPETS_ROOT"), "{err}");
    let (code, _, err) = run(&["snippets", "list", "--root", "/no/such/root"], &[]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("片段库目录不存在"), "{err}");

    // G2 对照：内置缺省根不在册维持「片段库为空」exit 0（#44 契约不破）
    let snips = TempState::new("g2");
    let (code, _, err) = run(&["snippets", "list"], &[("BROWSE_NAME", snips.path())]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("片段库为空"), "{err}");

    // G5：裸 @ 特例文案（不再落悬空冒号形）
    let (code, _, err) = run(&["--js", "@"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("@ 后缺文件路径"), "{err}");
}

/// 评审二轮 G10：显式根存在但不是目录（指到文件）报「不是目录」配置错
/// exit 1，与「目录不存在」分面。
#[test]
fn snippets_explicit_root_not_a_dir() {
    let st = TempState::new("g10");
    let file = std::path::Path::new(st.path()).join("a-file");
    std::fs::write(&file, "x").expect("写探针文件");
    let (code, _, err) = run(
        &["snippets", "list", "--root", &file.to_string_lossy()],
        &[],
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("片段库根不是目录"), "{err}");
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
    std::fs::write(root.join("miss.js"), "return {{ alpha }} + {{ beta }}\n").expect("写模板");
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
    std::fs::write(root.join("dup.js"), "{{ boom }}\n").expect("写库内同名");
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
    assert!(err.contains("Connection refused"), "填充毕进求值道：{err}");
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

    std::fs::write(&f, "pub async fn main() { Ok(\"{{ alpha }}\") }\n").expect("写模板脚本");
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

/// #67 三轮 G10：子命令收集环里「认识的全局旗标尾置」报位置 CTA（不再
/// 「参数不认识」的措辞矛盾），未知旗标保持原 bail 面；rune 与 snippets
/// 两族同口径。
#[test]
fn tail_global_flag_gets_position_cta() {
    let (code, _, err) = run(&["rune", "x.rn", "--headless"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("是全局旗标，要置于子命令与脚本位之前"),
        "{err}"
    );
    let (code, _, err) = run(&["snippets", "list", "x", "--json"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains("是全局旗标，要置于子命令与脚本位之前"),
        "{err}"
    );
    // 未知旗标保持原面（不误升位置 CTA）
    let (code, _, err) = run(&["rune", "x.rn", "--bogus"], &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("参数不认识 --bogus"), "{err}");
}

/// 目录审计 G-A7：KNOWN_GLOBAL_FLAGS 名册与 rune 子命令收集环的旁证
/// ——每个名册旗标尾置都应得「是全局旗标」位置 CTA（识别面），未知旗标
/// 走原 bail 面（对照）。名册加旗标漏同步解析臂时，本测试红。
#[test]
fn roster_flags_all_recognized_in_rune_tail_position() {
    // 取值型旗标（--bind/--ws 等）在收集环内以裸名触发 CTA 后即退出，
    // 无需跟值
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
        // 注：--vars/--root 在 rune 环内是合法子旗标（消费值），不属尾
        // 置 CTA 面，不入本守卫名单

        let (code, _, err) = run(&["rune", "x.rn", f], &[]);
        assert_eq!(code, 2, "旗标 {f}：{err}");
        assert!(
            err.contains(&format!("{f} 是全局旗标，要置于子命令与脚本位之前")),
            "旗标 {f} 未被名册识别（名册与解析臂漂移）：{err}"
        );
    }
}

/// 目录审计 G-2 收口：名册与全局解析臂双向比对（源抽取）——正向：名册
/// 每项都是全局循环臂键；反向：全局循环的旗标形臂键都在名册（子命令
/// 内环旗标不在全局段，不参与）。此前守卫只验名单到识别面单向。
#[test]
fn roster_matches_global_parse_arms_bidirectionally() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("读 main.rs 源");
    // 名册：const KNOWN_GLOBAL_FLAGS 块内的字符串字面量
    let roster_start = src
        .find("const KNOWN_GLOBAL_FLAGS: &[&str] = &[")
        .expect("名册在位")
        + "const KNOWN_GLOBAL_FLAGS: &[&str] = &[".len();
    let roster_end = src[roster_start..].find("];").expect("名册闭合") + roster_start;
    let roster: Vec<String> = src[roster_start..roster_end]
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect();
    assert!(roster.len() >= 30, "名册至少 30 项，实得 {}", roster.len());
    // 全局解析臂：第一个 `while let Some(a) = args.next_opt()` 循环段内的
    // `"--xxx" | "-x" =>` 臂键（到 `match a.as_str()` 的下一个子命令分派
    // 或 Mode 赋值为止——取该循环段的闭合花括号前文本）
    let loop_start = src
        .find("while let Some(a) = args.next_opt()")
        .expect("全局循环在位");
    let seg = &src[loop_start..loop_start + 6000];
    // 顶层臂按缩进判（12 空格 + 引号开头；子命令内环臂更深缩进不入）：
    // 常量提取用 fmt 的稳定缩进
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
    // 正向：名册每项都是臂键
    for f in &roster {
        assert!(arms.contains(f), "名册项 {f} 不在全局解析臂（正向漂移）");
    }
    // 反向：臂键每项都在名册
    for a in &arms {
        assert!(
            roster.contains(a),
            "解析臂 {a} 不在名册（反向漂移；子命令内环旗标误入全局段？）"
        );
    }
}

/// #64/#65 评审 G3 存量收口：list 面符号链接跳过（越根链接不进清单，
/// show 面已有越根守卫）。
#[test]
fn snippets_list_skips_symlinks() {
    let st = TempState::new("symlnk");
    let root = format!("{}/snippets", st.path());
    let outside = format!("{}/outside", st.path());
    std::fs::create_dir_all(format!("{}/site", root)).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(format!("{}/site/real.js", root), "// 真件").unwrap();
    std::fs::write(format!("{}/evil.js", outside), "// 链接目标").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        format!("{}/evil.js", outside),
        format!("{}/site/link.js", root),
    )
    .unwrap();
    let (code, out, err) = run(&["snippets", "list"], &[("BROWSE_NAME", st.path())]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("site/real.js"), "真件在列：{out}");
    assert!(!out.contains("link.js"), "符号链接应被跳过：{out}");
}
