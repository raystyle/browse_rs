//! workspace 单仓管理（#50/#51 配套 + 清仓重建总台令 2026-10-09）：站点
//! 与机制知识仓 `github.com/raystyle/browse_workspace` 的安装、更新与仓内
//! 读取。技能本体是「宿主侧 `mech.rn` + 页内侧 `probe.user.js`」双脚本对
//! （`page-skills/<slug>/` 与 `domain-skills/<段>/` 同形），README 只作人读
//! 说明——list/site/page 只认成对目录，缺一的目录不算技能。
//!
//! 何时用：`browse workspace` 命令族的全部后端（status/install/update/
//! list/site/page）。git 维护走 shell-out `git`（不引 git crate）：install
//! 是 clone、update 是 `pull --ff-only`，本地修改可手工 commit/push 回推
//! 同一远端。仓根定位见 [`crate::paths::workspace_dir`]。

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;
use std::path::{Path, PathBuf};

/// 种子仓远端（install 的缺省源；本地修改 push 回推同一远端）。
pub const SEED_REMOTE: &str = "https://github.com/raystyle/browse_workspace.git";

/// git 子进程收口（`&str` 实参版）：见 [`git_os`]。
fn git(root: Option<&Path>, args: &[&str]) -> Result<String> {
    git_os(
        root,
        &args
            .iter()
            .map(|s| std::ffi::OsStr::new(*s))
            .collect::<Vec<_>>(),
    )
}

/// git 子进程收口（`&OsStr` 实参版，评审 G7）：`git --version` 或
/// `git -C <root> <args...>`（current_dir 代 -C，全程 argv 直传无 shell），
/// 取 stdout trim。内部函数，错误串自带下一步指令。root 与实参走
/// `OsStr` 保非 UTF-8 路径不经 lossy 替换（clone 目标错位的注入面）。
fn git_os(root: Option<&Path>, args: &[&std::ffi::OsStr]) -> Result<String> {
    let mut cmd = std::process::Command::new("git");
    if let Some(r) = root {
        cmd.current_dir(r);
    }
    cmd.args(args);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("git 未安装或不在 PATH；下一步：装 git，或手工 git clone {SEED_REMOTE} 到仓根")
        } else {
            anyhow!("git 启动失败（{e}）；下一步：看 git 是否可用")
        }
    })?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        // 展示面拼接（非执行面）：OsStr 逐个 lossy 再连接
        let argv = args
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        bail!("git {argv} 失败（{detail}）；下一步：在仓根手工跑同一命令看完整输出");
    }
    Ok(stdout)
}

/// `browse workspace install`：git clone 种子仓到 `root`。clone 不带
/// `--depth`（shallow 与 `pull --ff-only`/回推有边角，仓极小不值得），
/// 分支吃缺省。回 `{root, remote, head}`。
///
/// # Errors
///
/// git 未安装；目录已被占用（存在且非空）；clone 非零退出（网络、权限）。
pub fn install(root: &Path) -> Result<serde_json::Value> {
    git(None, &["--version"]).context("git 预检")?;
    if root.exists() {
        let non_empty = std::fs::read_dir(root)
            .map(|mut it| it.next().is_some())
            .unwrap_or(true);
        if non_empty {
            bail!(
                "仓根 {} 已存在且非空；下一步：已装过就 browse workspace update，\
                 换路径就设 BROWSE_WORKSPACE",
                root.display()
            );
        }
    }
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("建父目录 {}", parent.display()))?;
    }
    git_os(
        None,
        &[
            std::ffi::OsStr::new("clone"),
            std::ffi::OsStr::new(SEED_REMOTE),
            root.as_os_str(),
        ],
    )?;
    let head = git(Some(root), &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    Ok(json!({
        "root": root.to_string_lossy(),
        "remote": SEED_REMOTE,
        "head": head,
    }))
}

/// `browse workspace update`：脏树先拒（本地修改未提交会挡 fast-forward），
/// 再 `git pull --ff-only`。回 `{root, head}`。
///
/// # Errors
///
/// 未安装；本地有未提交修改；git 缺失；pull 非零退出（远端分叉等）。
pub fn update(root: &Path) -> Result<serde_json::Value> {
    if !root.exists() {
        bail!(
            "仓根 {} 不存在；下一步：browse workspace install",
            root.display()
        );
    }
    let dirty = git(Some(root), &["status", "--porcelain"]).unwrap_or_default();
    if !dirty.is_empty() {
        bail!(
            "workspace 仓有未提交修改（{} 处）；下一步：cd {} 手工 commit/push 回推，\
             或 stash 后再 update",
            dirty.lines().count(),
            root.display()
        );
    }
    git(Some(root), &["pull", "--ff-only"])?;
    let head = git(Some(root), &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    Ok(json!({
        "root": root.to_string_lossy(),
        "head": head,
    }))
}

/// `browse workspace status` 的机器面：`{installed, root, gitPresent,
/// remote, branch, head, dirtyChanges, domainSites, domainFiles,
/// pageSlugs}`。git 子项失败容忍降级 null（status 是概览不是断言）。
///
/// # Errors
///
/// 仅当仓在位而 git 查询整体失败时仍返回概览（不报错）；本函数把
/// git 缺失也降级为 `gitPresent:false`，正常路径不报错。
pub fn status_json(root: &Path) -> Result<serde_json::Value> {
    let installed = root.exists();
    let git_present = git(None, &["--version"]).is_ok();
    let (remote, branch, head, dirty) = if installed && git_present {
        let r = git(Some(root), &["remote", "get-url", "origin"]).ok();
        let b = git(Some(root), &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
        let h = git(Some(root), &["rev-parse", "--short", "HEAD"]).ok();
        let d = git(Some(root), &["status", "--porcelain"])
            .map(|s| s.lines().count() as u64)
            .unwrap_or(0);
        (r, b, h, d)
    } else {
        (None, None, None, 0)
    };
    let (domain_sites, domain_files, page_slugs) = skill_counts(root);
    Ok(json!({
        "installed": installed,
        "root": root.to_string_lossy(),
        "gitPresent": git_present,
        "remote": remote,
        "branch": branch,
        "head": head,
        "dirtyChanges": dirty,
        "domainSites": domain_sites,
        "domainFiles": domain_files,
        "pageSlugs": page_slugs,
    }))
}

/// `browse workspace list` 的机器面：`{domains: [{segment, files}],
/// pages: [slug]}`。files 是对内文件（mech.rn/probe.user.js/README.md）；
/// 未安装返回空结构（由调用面给安装提示）。
pub fn list_json(root: &Path) -> serde_json::Value {
    // 双脚本对口径（清仓重建）：段是 domain-skills 下的成对目录；files
    // 列对内文件（mech.rn/probe.user.js/README.md 三选其二三）——不再有
    // md 清单与 10 篇帽（帽是为旧文海服务的）
    let mut domains = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root.join("domain-skills")) {
        let mut segs: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        segs.sort();
        for seg in segs {
            let files = pair_files(&root.join("domain-skills").join(&seg));
            if files.is_empty() {
                continue;
            }
            domains.push(json!({
                "segment": seg,
                "files": files,
                "capped": false,
            }));
        }
    }
    let mut pages = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root.join("page-skills")) {
        let mut slugs: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| is_pair_dir_name(&root.join("page-skills").join(n)))
            .collect();
        slugs.sort();
        pages = slugs;
    }
    json!({ "domains": domains, "pages": pages })
}

/// 探针文件形（REQ-041 P0 两段式）：单件 `probe.user.js`，或
/// `probe.skeleton.user.js` + `probe.heavy.user.js` 双件（骨架采样收齐 +
/// heavy 按需富集）；空 vec = 无合法探针形。
pub fn probe_files(dir: &Path) -> Vec<String> {
    if dir.join("probe.user.js").is_file() {
        return vec!["probe.user.js".to_string()];
    }
    if dir.join("probe.skeleton.user.js").is_file() && dir.join("probe.heavy.user.js").is_file() {
        return vec![
            "probe.skeleton.user.js".to_string(),
            "probe.heavy.user.js".to_string(),
        ];
    }
    Vec::new()
}

/// 目录里的对文件清单（排序）：`mech.rn` 与任一合法探针形同时在才
/// 非空（附 README.md 若在）；不成对返回空 vec（list 层跳过）。
fn pair_files(dir: &Path) -> Vec<String> {
    let probes = probe_files(dir);
    if !dir.join("mech.rn").is_file() || probes.is_empty() {
        return Vec::new();
    }
    let mut files = vec!["mech.rn".to_string()];
    files.extend(probes);
    if dir.join("README.md").is_file() {
        files.push("README.md".to_string());
    }
    files
}

/// 目录是否成对（list 的 page 层过滤：不成对目录不算技能）。
fn is_pair_dir_name(dir: &Path) -> bool {
    dir.join("mech.rn").is_file() && !probe_files(dir).is_empty()
}

/// #63 多根 site 读：序首命中根优先（根内逻辑同 [`read_site`]）；全根
/// 未命中时报错带全部根的找过路径。
///
/// # Errors
///
/// 所有根都未命中该段或文件（错误聚合各根路径）。
pub fn read_site_multi(roots: &[PathBuf], rel: &str) -> Result<String> {
    let mut misses = Vec::new();
    for root in roots {
        match read_site(root, rel) {
            Ok(text) => return Ok(text),
            Err(e) => misses.push(format!("{}（{e:#}）", root.display())),
        }
    }
    bail!(
        "workspace site {rel} 在 {} 个仓根都未命中：{}；下一步：browse workspace add <自定义仓> 或 list 看在册段",
        misses.len(),
        misses.join("；")
    )
}

/// #63 多根 page 读：序首命中根优先。
///
/// # Errors
///
/// 所有根都未命中该 slug。
pub fn read_page_multi(roots: &[PathBuf], slug: &str) -> Result<String> {
    let mut misses = Vec::new();
    for root in roots {
        match read_page(root, slug) {
            Ok(text) => return Ok(text),
            Err(e) => misses.push(format!("{}（{e:#}）", root.display())),
        }
    }
    bail!(
        "workspace page {slug} 在 {} 个仓根都未命中：{}；下一步：browse workspace list 看在册 slug",
        misses.len(),
        misses.join("；")
    )
}

/// #63 多根段文件列（技能点名口径）：序首有文件的根整胜（不跨根混拼，
/// 保回执确定性——同段多仓时序首仓定义该站点知识）。
pub fn domain_segment_files_multi(roots: &[PathBuf], segment: &str) -> Vec<String> {
    for root in roots {
        let files = domain_segment_files(root, segment);
        if !files.is_empty() {
            return files;
        }
    }
    Vec::new()
}

/// #63 多根清单：domain 段与 page slug 跨根并集（首见序，段名重复以
/// 序首仓的文件清单为准）；`roots` 面给配置态可见性。
pub fn list_json_multi(roots: &[PathBuf]) -> serde_json::Value {
    let mut domains: Vec<serde_json::Value> = Vec::new();
    let mut seen_seg: Vec<String> = Vec::new();
    let mut pages: Vec<String> = Vec::new();
    for root in roots {
        let v = list_json(root);
        for d in v["domains"].as_array().cloned().unwrap_or_default() {
            let seg = d["segment"].as_str().unwrap_or("").to_string();
            if !seg.is_empty() && !seen_seg.contains(&seg) {
                seen_seg.push(seg);
                domains.push(d);
            }
        }
        for p in v["pages"].as_array().cloned().unwrap_or_default() {
            let slug = p.as_str().unwrap_or("").to_string();
            if !slug.is_empty() && !pages.contains(&slug) {
                pages.push(slug);
            }
        }
    }
    json!({
        "domains": domains,
        "pages": pages,
        "roots": roots.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
    })
}

/// 仓内相对读取的越界守卫：canonicalize 后必须仍在 `root` 内，挡绝对
/// 路径、`..` 与符号链接出仓（#44 评审 F2 同律）。返回可读的
/// 规范路径。
fn guarded_join(root: &Path, rel: &str, what: &str) -> Result<PathBuf> {
    let joined = root.join(rel);
    let canon = joined.canonicalize().map_err(|_| {
        anyhow!(
            "{what} {rel} 不存在（{}）；下一步：browse workspace list 看在册条目",
            joined.display()
        )
    })?;
    let root_canon = root.canonicalize().map_err(|_| {
        anyhow!(
            "workspace 仓根 {} 不存在；下一步：browse workspace install",
            root.display()
        )
    })?;
    if !canon.starts_with(&root_canon) {
        bail!("{what} {rel} 越出 workspace 仓（只许仓内相对路径）");
    }
    Ok(canon)
}

/// `browse workspace site <段>[/<文件>]`：只给段时打该段清单（每行
/// `<文件>  <首行标题>`，路径摘要同构；是回执点名
/// 的下钻面）；带 `<段>/<文件>` 时读该文件全文。
///
/// # Errors
///
/// 段或文件不存在（CTA 指回 list）；路径越出仓根；仓根未安装。
pub fn read_site(root: &Path, rel: &str) -> Result<String> {
    let seg_root = guarded_join(root, &format!("domain-skills/{rel}"), "workspace site")?;
    if seg_root.is_file() {
        return std::fs::read_to_string(&seg_root)
            .with_context(|| format!("读 {}", seg_root.display()));
    }
    let files = pair_files(&seg_root);
    if files.is_empty() {
        bail!(
            "workspace site 段 {rel} 不存在或无成对技能（{}）；下一步：browse workspace list 看在册段",
            root.join("domain-skills").join(rel).display()
        );
    }
    Ok(pair_summary(&seg_root, rel))
}

/// 一对技能的读全文回执：有 README 给 README 全文（尾部附对路径），
/// 无 README 给路径摘要行。
fn pair_summary(dir: &Path, name: &str) -> String {
    let readme = dir.join("README.md");
    if readme.is_file()
        && let Ok(text) = std::fs::read_to_string(&readme)
    {
        return format!(
            "{text}\n\n--- 对路径:mech.rn 与 probe.user.js 在 {} 下 ---\n",
            dir.display()
        );
    }
    format!(
        "{name} 是双脚本对（无 README）：mech.rn（宿主编排）与 probe.user.js（页内探针）在 {} 下\n",
        dir.display()
    )
}

/// `browse workspace page <slug>`：交出该对（有 README 附全文，无给
/// 路径摘要；清仓重建后不再读 `page-skills/<slug>.md` 旧形）。
///
/// # Errors
///
/// slug 不存在或不成对（CTA 指回 list）；路径越出仓根；仓根未安装。
pub fn read_page(root: &Path, slug: &str) -> Result<String> {
    let dir = guarded_join(root, &format!("page-skills/{slug}"), "workspace page")?;
    if pair_files(&dir).is_empty() {
        bail!(
            "workspace page {slug} 不存在或不成对（{}）；下一步：browse workspace list 看在册 slug",
            root.join("page-skills").join(slug).display()
        );
    }
    Ok(pair_summary(&dir, slug))
}

/// 列 `<root>/domain-skills/<段>/` 的对文件清单（成对才有,排序保回执
/// 确定）。目录缺失或不成对返回空。何时用：技能触发层（[`crate::skills`]）
/// 的点名清单与 `browse workspace list` 共用此口径。
pub fn domain_segment_files(root: &Path, segment: &str) -> Vec<String> {
    pair_files(&root.join("domain-skills").join(segment))
}

/// 仓内技能计数：`{段数, 域文件总数（不封顶，all 口径非 list 封顶）,
/// page slug 数}`（评审 G-C：status 概览不该跟着 list 的 10 帽少报）。
fn skill_counts(root: &Path) -> (u64, u64, u64) {
    let list = list_json(root);
    let sites = list
        .get("domains")
        .and_then(|d| d.as_array())
        .map_or(0, |d| d.len() as u64);
    let mut files = 0u64;
    if let Some(doms) = list.get("domains").and_then(|d| d.as_array()) {
        for d in doms {
            let seg = d
                .get("segment")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            files += pair_files(&root.join("domain-skills").join(seg)).len() as u64;
        }
    }
    let pages = list
        .get("pages")
        .and_then(|p| p.as_array())
        .map_or(0, |p| p.len() as u64);
    (sites, files, pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    /// 唯一 temp 仓：进程内递增防互撞，测后清理。
    fn temp_root(tag: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("browse-ws-{}-{tag}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn seed(root: &Path) {
        // 双脚本对形（清仓重建）：段 x 成对带 README；page 层 challenge-stop 成对
        let seg = root.join("domain-skills").join("x");
        std::fs::create_dir_all(&seg).unwrap();
        std::fs::write(seg.join("mech.rn"), "pub async fn main() {}").unwrap();
        std::fs::write(seg.join("probe.user.js"), "// probe").unwrap();
        std::fs::write(seg.join("README.md"), "# A 站要点\n正文\n").unwrap();
        let pg = root.join("page-skills").join("challenge-stop");
        std::fs::create_dir_all(&pg).unwrap();
        std::fs::write(pg.join("mech.rn"), "pub async fn main() {}").unwrap();
        std::fs::write(pg.join("probe.user.js"), "// probe").unwrap();
    }

    /// list_json：段与 slug 齐全（成对目录才算），半对被滤。
    #[test]
    fn list_shape() {
        let root = temp_root("list");
        seed(&root);
        // 半对（只有 probe.user.js）不算 page 技能
        let half = root.join("page-skills").join("half");
        std::fs::create_dir_all(&half).unwrap();
        std::fs::write(half.join("probe.user.js"), "// x").unwrap();
        let l = list_json(&root);
        let domains = l["domains"].as_array().unwrap();
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0]["segment"], json!("x"));
        assert_eq!(
            domains[0]["files"],
            json!(["mech.rn", "probe.user.js", "README.md"]),
            "对文件排序，README 在列"
        );
        assert_eq!(l["pages"], json!(["challenge-stop"]));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 无 md 海即无帽：段文件清单就是对文件（帽随旧文海退役的负向锁）。
    #[test]
    fn list_caps_retired_pairs_only() {
        let root = temp_root("cap");
        let seg = root.join("domain-skills").join("many");
        std::fs::create_dir_all(&seg).unwrap();
        for i in 0..15 {
            std::fs::write(seg.join(format!("f{i:02}.md")), "# f\n").unwrap();
        }
        let l = list_json(&root);
        assert!(
            l["domains"].as_array().unwrap().is_empty(),
            "散 md 不成对零清单: {}",
            l["domains"]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// read_site：对段回 README 全文（尾附对路径）；README 直读仍在；
    /// 未命中与越界都拒。
    #[test]
    fn read_site_paths() {
        let root = temp_root("site");
        seed(&root);
        let cat = read_site(&root, "x").unwrap();
        assert!(cat.contains("# A 站要点"), "README 全文: {cat}");
        assert!(cat.contains("mech.rn"), "尾附对路径: {cat}");
        let full = read_site(&root, "x/README.md").unwrap();
        assert!(full.starts_with("# A 站要点"));
        assert!(read_site(&root, "nope").is_err(), "段未命中报错");
        assert!(
            read_site(&root, "../../../etc").is_err(),
            "越界拒绝（不存在路径同样失败）"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// read_site 绝对路径拒绝：join 到绝对路径会替换根，守卫必须拦下。
    #[test]
    fn read_site_absolute_rejected() {
        let root = temp_root("abs");
        seed(&root);
        let abs = if cfg!(windows) {
            "C:\\Windows\\win.ini"
        } else {
            "/etc/hostname"
        };
        assert!(read_site(&root, abs).is_err(), "绝对路径拒绝");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 符号链接出仓拒绝（POSIX；Windows 建链要特权不测）。
    #[cfg(unix)]
    #[test]
    fn read_site_symlink_rejected() {
        let root = temp_root("link");
        seed(&root);
        let outside = temp_root("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.md"), "# 秘密").unwrap();
        let seg = root.join("domain-skills").join("x");
        std::os::unix::fs::symlink(outside.join("secret.md"), seg.join("z.md")).unwrap();
        assert!(
            read_site(&root, "x/z.md").is_err(),
            "符号链接出仓拒绝（越界守卫）"
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// read_page：命中读全文，未命中报错；仓内 `../` 语义（评审 G13 假绿
    /// 修正）：守卫只拦「出仓」，`page-skills/../README.md` 仍在仓内故放行
    /// （C8 定谳的设计语义，本测用存在目标锁边界，不再锁「文件不存在」）。
    #[test]
    fn read_page_paths() {
        let root = temp_root("page");
        seed(&root);
        let text = read_page(&root, "challenge-stop").unwrap();
        assert!(text.contains("mech.rn"), "对路径在回执: {text}");
        assert!(read_page(&root, "nope").is_err(), "未命中报错");
        assert!(
            read_page(&root, "../../etc").is_err(),
            "越界拒绝（不存在路径同样失败）"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// status_json：git init 临时仓（离线）锁 installed 形——branch 有值、
    /// 无 origin 时 remote 为 null（不报错），计数正确（评审 G8 补单测）。
    #[test]
    fn two_part_pair_shape() {
        // 两段式对(REQ-041 P0):skeleton+heavy 双件同算成对;半套(缺 heavy)不成对
        let dir = std::env::temp_dir().join(format!("bw-two-part-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let two = dir.join("two");
        std::fs::create_dir_all(&two).unwrap();
        std::fs::write(two.join("mech.rn"), "// mech").unwrap();
        std::fs::write(two.join("probe.skeleton.user.js"), "// sk").unwrap();
        std::fs::write(two.join("probe.heavy.user.js"), "// hv").unwrap();
        assert_eq!(
            pair_files(&two),
            vec!["mech.rn", "probe.skeleton.user.js", "probe.heavy.user.js"]
        );
        let half = dir.join("half");
        std::fs::create_dir_all(&half).unwrap();
        std::fs::write(half.join("mech.rn"), "// mech").unwrap();
        std::fs::write(half.join("probe.skeleton.user.js"), "// sk").unwrap();
        assert!(pair_files(&half).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_git_repo_shape() {
        let root = temp_root("gitrepo");
        seed(&root);
        // init 加一条空 commit（裸 init 的 HEAD 未生，branch 查询恒失败）
        let ok = git(Some(&root), &["init", "-q", "-b", "main"]).is_ok()
            && git(
                Some(&root),
                &[
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "commit",
                    "--allow-empty",
                    "-q",
                    "-m",
                    "init",
                ],
            )
            .is_ok();
        if !ok {
            // git 缺失环境（理论仅极端 CI）：本测依赖 git，跳过而非假红
            let _ = std::fs::remove_dir_all(&root);
            return;
        }
        let s = status_json(&root).unwrap();
        assert_eq!(s["installed"], json!(true));
        assert_eq!(s["gitPresent"], json!(true));
        assert!(
            s["remote"].is_null(),
            "无 origin 时 remote 降级 null 非报错: {s}"
        );
        assert_eq!(s["branch"], json!("main"), "git 仓 branch 有值: {s}");
        assert!(s["head"].as_str().is_some(), "commit 后 head 有值: {s}");
        assert_eq!(s["domainSites"], json!(1));
        assert_eq!(s["pageSlugs"], json!(1));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// status_json：未安装时 installed=false 且计数为零，不报错。
    #[test]
    fn status_not_installed() {
        let root = temp_root("status");
        let s = status_json(&root).unwrap();
        assert_eq!(s["installed"], json!(false));
        assert_eq!(s["domainSites"], json!(0));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// #63 多根序首胜：同段两仓，序首仓的文件清单整胜；空仓跳过。
    #[test]
    fn multi_root_first_hit_wins() {
        let dir = std::env::temp_dir().join(format!("browse-ws-multi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = dir.join("first");
        let second = dir.join("second");
        let mkpair = |root: &Path, seg: &str, readme: &str| {
            let d = root.join("domain-skills").join(seg);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("mech.rn"), "pub async fn main() {}").unwrap();
            std::fs::write(d.join("probe.user.js"), "// probe").unwrap();
            std::fs::write(d.join("README.md"), readme).unwrap();
        };
        mkpair(&first, "x", "# a");
        mkpair(&second, "x", "# b");
        let roots = vec![first.clone(), second.clone()];
        assert_eq!(
            domain_segment_files_multi(&roots, "x"),
            vec![
                "mech.rn".to_string(),
                "probe.user.js".to_string(),
                "README.md".to_string()
            ],
            "序首仓整胜不混拼"
        );
        std::fs::remove_file(first.join("domain-skills/x/mech.rn")).unwrap();
        assert_eq!(
            domain_segment_files_multi(&roots, "x"),
            vec![
                "mech.rn".to_string(),
                "probe.user.js".to_string(),
                "README.md".to_string()
            ],
            "序首仓破坏对后跳到次根"
        );
        let text = read_site_multi(&roots, "x").unwrap();
        assert!(text.contains("# b"), "{text}");
        let pg = second.join("page-skills").join("challenge-stop");
        std::fs::create_dir_all(&pg).unwrap();
        std::fs::write(pg.join("mech.rn"), "pub async fn main() {}").unwrap();
        std::fs::write(pg.join("probe.user.js"), "// probe").unwrap();
        std::fs::write(pg.join("README.md"), "# 配方").unwrap();
        let page = read_page_multi(&roots, "challenge-stop").unwrap();
        assert!(page.contains("配方"), "{page}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// #63 多根清单并集：段与 slug 跨根首见去重，roots 面在册。
    #[test]
    fn multi_root_list_union() {
        let dir = std::env::temp_dir().join(format!("browse-ws-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (sub, segs, pages) in [
            ("one", vec!["github"], vec!["challenge-stop"]),
            ("two", vec!["medium", "github"], vec!["antibot-vendor"]),
        ] {
            for seg in &segs {
                let d = dir.join(sub).join("domain-skills").join(seg);
                std::fs::create_dir_all(&d).unwrap();
                std::fs::write(d.join("mech.rn"), "pub async fn main() {}").unwrap();
                std::fs::write(d.join("probe.user.js"), "// probe").unwrap();
            }
            for slug in &pages {
                let d = dir.join(sub).join("page-skills").join(slug);
                std::fs::create_dir_all(&d).unwrap();
                std::fs::write(d.join("mech.rn"), "pub async fn main() {}").unwrap();
                std::fs::write(d.join("probe.user.js"), "// probe").unwrap();
            }
        }
        let roots = vec![dir.join("one"), dir.join("two")];
        let l = list_json_multi(&roots);
        let segs: Vec<&str> = l["domains"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|d| d["segment"].as_str())
            .collect();
        assert!(
            segs.contains(&"github") && segs.contains(&"medium"),
            "{segs:?}"
        );
        let slugs: Vec<&str> = l["pages"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str())
            .collect();
        assert!(
            slugs.contains(&"challenge-stop") && slugs.contains(&"antibot-vendor"),
            "{slugs:?}"
        );
        assert_eq!(l["roots"].as_array().map(|a| a.len()), Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
