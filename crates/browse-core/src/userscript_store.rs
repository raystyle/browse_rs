//! 油猴脚本盘上契约管理（REQ #79）：对引擎 `<profile>/Default/Userscripts/`
//! 的同构读写——安装、列表、启停、删除、换源、路径输出。
//!
//! 引擎（clean-chrome `UserscriptStore`）启动时读盘，装改后需引擎重启生效
//! （CLI `--restart` 自动 down/up）；批 4 `Userscript.*` CDP 域落地后升级为
//! 域优先、盘上兜底。id 与引擎 `MakeId` 同构：`SHA256("namespace@name")`
//! 前 6 字节 hex（12 字符）。

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::workspace_roots;

/// 引擎侧 Userscripts 目录（Chrome profile 下的固定相对位）。
pub fn userscripts_root(profile: &Path) -> PathBuf {
    profile.join("Default").join("Userscripts")
}

/// 脚本 id：`SHA256("namespace@name")` 前 6 字节 hex，与引擎 `MakeId` 同构。
pub fn make_id(namespace: &str, name: &str) -> String {
    let mut h = Sha256::new();
    h.update(format!("{namespace}@{name}"));
    h.finalize()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `==UserScript==` 头解析结果（缺省语义同置场 CLI：无 `@name` 用文件基名，
/// 无 `@namespace` 用 `"file"`）。
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptMeta {
    /// 显示名（index 条目与 id 计入）。
    pub name: String,
    /// 命名空间（id 计入）。
    pub namespace: String,
    /// `@match` 列表。
    pub matches: Vec<String>,
    /// `@include` 列表。
    pub includes: Vec<String>,
    /// `@exclude` 列表。
    pub excludes: Vec<String>,
    /// `@connect` 列表（连域闸）。
    pub connects: Vec<String>,
    /// `@grant` 列表（显式授权面，含 unsafeWindow）。
    pub grants: Vec<String>,
    /// `@noframes`。
    pub noframes: bool,
    /// `@run-at`（引擎注入时机，meta 不落盘、由脚本体头外字段自行携带）。
    pub run_at: Option<String>,
}

/// 解析 `==UserScript==` 头；无头块时按全缺省 meta 返回（仍可安装，等于
/// 一段无匹配面的普通脚本）。
pub fn parse_header(src: &str, file_stem: &str) -> ScriptMeta {
    let mut m = ScriptMeta {
        name: file_stem.to_string(),
        namespace: "file".to_string(),
        matches: Vec::new(),
        includes: Vec::new(),
        excludes: Vec::new(),
        connects: Vec::new(),
        grants: Vec::new(),
        noframes: false,
        run_at: None,
    };
    let Some(block) = header_block(src) else {
        return m;
    };
    for line in block.lines() {
        let t = line.trim_start_matches([' ', '\t']).trim();
        let Some(rest) = t.strip_prefix("//") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(kv) = rest.strip_prefix('@') else {
            continue;
        };
        let (k, v) = match kv.split_once(char::is_whitespace) {
            Some((k, v)) => (k, v.trim()),
            None => (kv, ""),
        };
        match k {
            "name" => m.name = v.to_string(),
            "namespace" => m.namespace = v.to_string(),
            "match" => m.matches.push(v.to_string()),
            "include" => m.includes.push(v.to_string()),
            "exclude" => m.excludes.push(v.to_string()),
            "connect" => m.connects.push(v.to_string()),
            "grant" => m.grants.push(v.to_string()),
            "noframes" => m.noframes = true,
            "run-at" => m.run_at = Some(v.to_string()),
            _ => {}
        }
    }
    m
}

fn header_block(src: &str) -> Option<&str> {
    let start = src.find("==UserScript==")?;
    let end = src[start..].find("==/UserScript==")? + start;
    Some(&src[start..end])
}

/// 剥头后的脚本体（头块连同收尾标记剔除；无头块返回原文）。
pub fn script_body(src: &str) -> &str {
    let Some(end) = src.find("==/UserScript==") else {
        return src;
    };
    let tail = &src[end + "==/UserScript==".len()..];
    // 收尾标记后的第一个换行剥掉，避免正文以空行起头
    tail.strip_prefix('\n').unwrap_or(tail)
}

/// 读 index；缺文件视为空册（首装），**存在但形坏必须报错**——半写的坏
/// 盘若被吞成空册，下一次写会把真册盖掉（评审 F2）。
fn read_index(root: &Path) -> Result<Value, String> {
    let p = root.join("index.json");
    let Some(txt) = fs::read_to_string(&p).ok() else {
        return Ok(json!({"scripts": []}));
    };
    let v: Value = serde_json::from_str(&txt)
        .map_err(|e| format!("{} 形坏: {e}(不覆盖;手工修复或删除该文件重来)", p.display()))?;
    if v["scripts"].as_array().is_none() {
        return Err(format!("{} scripts 非数组(不覆盖)", p.display()));
    }
    Ok(v)
}

/// 原子写 index：同目录临时文件写稳后 rename 覆盖——崩溃窗口不留半截
/// 真名文件（评审 F2；`fs::write` 直写的截断窗口会让下次读把坏盘当空册）。
fn write_index(root: &Path, idx: &Value) -> Result<(), String> {
    let p = root.join("index.json");
    let tmp = root.join("index.json.tmp");
    let s = serde_json::to_string_pretty(idx).map_err(|e| format!("index 序列化失败: {e}"))?;
    fs::write(&tmp, &s).map_err(|e| format!("写 {} 失败: {e}", tmp.display()))?;
    fs::rename(&tmp, &p).map_err(|e| format!("落 {} 失败: {e}", p.display()))
}

fn meta_json(m: &ScriptMeta) -> Value {
    json!({
        "match": m.matches, "include": m.includes, "exclude": m.excludes,
        "connect": m.connects, "noframes": m.noframes, "grant": m.grants,
    })
}

/// 安装一个或多个 `.user.js`：写 `scripts/<id>/{meta.json, script.user.js}`，
/// index 条目 upsert（幂等重装覆盖源码与 meta，`enabled` 保留现值）。
///
/// # Errors
///
/// 源文件读失败或盘上写失败时返回带路径的错误串。
pub fn install(profile: &Path, files: &[PathBuf]) -> Result<Value, String> {
    let root = userscripts_root(profile);
    fs::create_dir_all(&root).map_err(|e| format!("建 {} 失败: {e}", root.display()))?;
    let mut idx = read_index(&root)?;
    let scripts = idx["scripts"]
        .as_array_mut()
        .ok_or("index.json scripts 非数组")?;
    let mut out = Vec::new();
    for f in files {
        let src = fs::read_to_string(f).map_err(|e| format!("读 {} 失败: {e}", f.display()))?;
        let stem = f
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let m = parse_header(&src, &stem);
        let id = make_id(&m.namespace, &m.name);
        let dir = root.join("scripts").join(&id);
        fs::create_dir_all(&dir).map_err(|e| format!("建 {} 失败: {e}", dir.display()))?;
        let meta_s =
            serde_json::to_string(&meta_json(&m)).map_err(|e| format!("meta 序列化失败: {e}"))?;
        fs::write(dir.join("meta.json"), &meta_s).map_err(|e| format!("写 meta 失败: {e}"))?;
        fs::write(dir.join("script.user.js"), script_body(&src))
            .map_err(|e| format!("写脚本体失败: {e}"))?;
        let existing = scripts
            .iter()
            .position(|e| e["id"].as_str() == Some(id.as_str()));
        let replaced = existing.is_some();
        let entry = match existing {
            Some(i) => {
                scripts[i]["name"] = json!(m.name);
                scripts[i]["namespace"] = json!(m.namespace);
                scripts[i].clone()
            }
            None => {
                let position = scripts.len();
                let e = json!({"id": id, "name": m.name, "namespace": m.namespace, "enabled": true, "position": position});
                scripts.push(e.clone());
                e
            }
        };
        out.push(json!({"id": id, "name": entry["name"].clone(), "namespace": entry["namespace"].clone(),
            "enabled": entry["enabled"].clone(), "position": entry["position"].clone(), "replaced": replaced}));
    }
    write_index(&root, &idx)?;
    Ok(json!({"installed": out, "root": root}))
}

/// 定位条目（id 前缀或全名匹配）下标；找不到报带候选提示的错误。
fn find_entry(scripts: &[Value], id_or_name: &str) -> Result<usize, String> {
    if let Some(i) = scripts
        .iter()
        .position(|e| e["id"].as_str() == Some(id_or_name))
    {
        return Ok(i);
    }
    if let Some(i) = scripts
        .iter()
        .position(|e| e["name"].as_str() == Some(id_or_name))
    {
        return Ok(i);
    }
    let hit_idx: Vec<usize> = scripts
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(id_or_name))
                || e["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with(id_or_name))
        })
        .map(|(i, _)| i)
        .collect();
    if hit_idx.len() == 1 {
        return Ok(hit_idx[0]);
    }
    let names: Vec<String> = scripts
        .iter()
        .filter_map(|e| {
            e["id"]
                .as_str()
                .map(|id| format!("{id} {}", e["name"].as_str().unwrap_or("")))
        })
        .collect();
    Err(format!(
        "未命中或歧义脚本 {id_or_name:?}(候选 {} 条);在册:\n{}",
        hit_idx.len(),
        names.join("\n")
    ))
}

/// 列表：对读 index.json,并按 workspace `page-skills/<slug>/probe.user.js`
/// 头算 id 对册（命中标 `pair`，双脚本对在册的装机态一眼可见）。
///
/// # Errors
///
/// index.json 存在但形坏时返回错误（缺文件视为空册）。
pub fn list(profile: &Path) -> Result<Value, String> {
    let root = userscripts_root(profile);
    let mut idx = read_index(&root)?;
    let mut pairs: Vec<(String, String)> = Vec::new(); // (id, slug)
    for wroot in workspace_roots() {
        let pk = wroot.join("page-skills");
        let Ok(rd) = fs::read_dir(&pk) else { continue };
        for e in rd.flatten() {
            let slug = e.file_name().to_string_lossy().into_owned();
            let probe = e.path().join("probe.user.js");
            let Ok(src) = fs::read_to_string(&probe) else {
                continue;
            };
            let m = parse_header(&src, "probe");
            pairs.push((make_id(&m.namespace, &m.name), slug));
        }
    }
    if let Some(arr) = idx["scripts"].as_array_mut() {
        for e in arr.iter_mut() {
            if let Some(id) = e["id"].as_str()
                && let Some((_, slug)) = pairs.iter().find(|(pid, _)| pid == id)
            {
                e["pair"] = json!(slug);
            }
        }
    }
    Ok(json!({"root": root, "scripts": idx["scripts"].clone()}))
}

/// 启停一个脚本（改 index 条目 `enabled`）。
///
/// # Errors
///
/// id/名未命中或 index 写失败时返回错误串。
pub fn set_enabled(profile: &Path, id_or_name: &str, enabled: bool) -> Result<Value, String> {
    let root = userscripts_root(profile);
    let mut idx = read_index(&root)?;
    let scripts = idx["scripts"]
        .as_array_mut()
        .ok_or("index.json scripts 非数组")?;
    let i = find_entry(scripts, id_or_name)?;
    scripts[i]["enabled"] = json!(enabled);
    let e = scripts[i].clone();
    write_index(&root, &idx)?;
    Ok(e)
}

/// 删除一个脚本：index 条目与 `scripts/<id>/` 目录同删。
///
/// # Errors
///
/// id/名未命中、目录删失败或 index 写失败时返回错误串。
pub fn remove(profile: &Path, id_or_name: &str) -> Result<Value, String> {
    let root = userscripts_root(profile);
    let mut idx = read_index(&root)?;
    let scripts = idx["scripts"]
        .as_array_mut()
        .ok_or("index.json scripts 非数组")?;
    let i = find_entry(scripts, id_or_name)?;
    // 先删目录后写 index（评审 F3）：目录删失败时条目仍在册，可重试；
    // 反序会留下「条目已无、目录残留」的幻影态
    let dir = root
        .join("scripts")
        .join(scripts[i]["id"].as_str().unwrap_or_default());
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e2| format!("删 {} 失败: {e2}", dir.display()))?;
    }
    let e = scripts.remove(i);
    // position 重排（引擎按序装载）
    for (n, s) in scripts.iter_mut().enumerate() {
        s["position"] = json!(n);
    }
    write_index(&root, &idx)?;
    Ok(e)
}

/// 同 id 换源：以新文件内容重写 meta 与脚本体（name/namespace 由新头决定，
/// id 随之；若 id 漂移视为新装+旧删的原子组合）。
///
/// # Errors
///
/// 源读失败或定位失败时返回错误串。
pub fn update(profile: &Path, id_or_name: &str, file: &Path) -> Result<Value, String> {
    // 先读稳新源再动旧（评审 F3）：源读失败/解析不动在册脚本一步
    let src = fs::read_to_string(file).map_err(|e| format!("读 {} 失败: {e}", file.display()))?;
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let m = parse_header(&src, &stem);
    let new_id = make_id(&m.namespace, &m.name);
    let root = userscripts_root(profile);
    let old = {
        let idx = read_index(&root)?;
        let scripts = idx["scripts"]
            .as_array()
            .ok_or("index.json scripts 非数组")?;
        scripts[find_entry(scripts, id_or_name)?].clone()
    };
    let old_id = old["id"].as_str().unwrap_or_default().to_string();
    let installed = install(profile, &[file.to_path_buf()])?;
    let removed = if new_id != old_id {
        Some(remove(profile, &old_id)?)
    } else {
        None
    };
    Ok(
        json!({"removed": removed, "installed": installed["installed"], "id_moved": new_id != old_id, "file": file.display().to_string()}),
    )
}

/// 输出脚本体文件路径（编辑入口：改后 `update` 同 id 回写或直接编辑该文件）。
///
/// # Errors
///
/// id/名未命中时返回错误串。
pub fn script_path(profile: &Path, id_or_name: &str) -> Result<PathBuf, String> {
    let root = userscripts_root(profile);
    let idx = read_index(&root)?;
    let scripts = idx["scripts"]
        .as_array()
        .ok_or("index.json scripts 非数组")?;
    let e = &scripts[find_entry(scripts, id_or_name)?];
    let dir = root
        .join("scripts")
        .join(e["id"].as_str().unwrap_or_default());
    if dir.join("script.user.js").exists() {
        Ok(dir.join("script.user.js"))
    } else {
        Err(format!("{} 下无 script.user.js（盘上缺件）", dir.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_同构前缀() {
        let id = make_id("browse-repro", "gmxhr-cors-repro");
        assert_eq!(id.len(), 12);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        // 实装机对账过的实值（gmbprofile 在册探针）
        assert_eq!(make_id("file", "gm-bridge probe"), "95bc31b2ef12");
    }

    #[test]
    fn 头解析全键与缺省() {
        let src = "// ==UserScript==\n// @name  t1 \n// @namespace ns\n// @match http://a/*\n// @connect 127.0.0.1\n// @grant GM_xmlhttpRequest\n// @grant unsafeWindow\n// @noframes\n// @run-at document-start\n// ==/UserScript==\nbody();\n";
        let m = parse_header(src, "stem");
        assert_eq!((m.name.as_str(), m.namespace.as_str()), ("t1", "ns"));
        assert_eq!(m.matches, ["http://a/*"]);
        assert_eq!(m.connects, ["127.0.0.1"]);
        assert_eq!(m.grants, ["GM_xmlhttpRequest", "unsafeWindow"]);
        assert!(m.noframes);
        assert_eq!(m.run_at.as_deref(), Some("document-start"));
        assert_eq!(script_body(src), "body();\n");

        let d = parse_header("no header", "stem");
        assert_eq!((d.name.as_str(), d.namespace.as_str()), ("stem", "file"));
        assert_eq!(script_body("no header"), "no header");
    }

    #[test]
    fn 装列启停删全链() {
        let tmp = std::env::temp_dir().join(format!("us-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let js = tmp.join("a.user.js");
        fs::create_dir_all(&tmp).unwrap();
        fs::write(&js, "// ==UserScript==\n// @name a\n// @match http://x/*\n// @grant GM_setValue\n// ==/UserScript==\nrun();").unwrap();

        let r = install(&tmp, std::slice::from_ref(&js)).unwrap();
        let id = r["installed"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(r["installed"][0]["replaced"], false);
        // meta 落盘带 grant
        let meta = fs::read_to_string(
            tmp.join("Default/Userscripts/scripts")
                .join(&id)
                .join("meta.json"),
        )
        .unwrap();
        assert!(meta.contains("GM_setValue"));
        assert_eq!(
            fs::read_to_string(
                tmp.join("Default/Userscripts/scripts")
                    .join(&id)
                    .join("script.user.js")
            )
            .unwrap(),
            "run();"
        );

        // 幂等重装:enabled 保留
        let off = set_enabled(&tmp, &id, false).unwrap();
        assert_eq!(off["enabled"], false);
        let r2 = install(&tmp, &[js]).unwrap();
        assert_eq!(r2["installed"][0]["replaced"], true);
        let l = list(&tmp).unwrap();
        assert_eq!(l["scripts"][0]["enabled"], false); // 重装不动 enabled

        // 全名与前缀定位
        let byname = set_enabled(&tmp, "a", true).unwrap();
        assert_eq!(byname["id"].as_str().unwrap(), id);
        // 前缀定位与删除
        let p = script_path(&tmp, &id[..6]).unwrap();
        assert!(p.ends_with("script.user.js"));
        let rm = remove(&tmp, &id[..6]).unwrap();
        assert_eq!(rm["id"].as_str().unwrap(), id);
        let l2 = list(&tmp).unwrap();
        assert_eq!(l2["scripts"].as_array().unwrap().len(), 0);
        assert!(!tmp.join("Default/Userscripts/scripts").join(&id).exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn update_源失败不动旧() {
        let tmp = std::env::temp_dir().join(format!("us-upd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let js = tmp.join("k.user.js");
        fs::write(
            &js,
            "// ==UserScript==\n// @name k\n// ==/UserScript==\nkeep();",
        )
        .unwrap();
        install(&tmp, &[js]).unwrap();
        let l = list(&tmp).unwrap();
        let id = l["scripts"][0]["id"].as_str().unwrap().to_string();
        // 源不存在:一步报错,旧脚本在册在盘
        let bad = update(&tmp, &id, &tmp.join("nope.user.js"));
        assert!(bad.is_err());
        let l2 = list(&tmp).unwrap();
        assert_eq!(l2["scripts"].as_array().unwrap().len(), 1);
        assert!(script_path(&tmp, &id).is_ok());
        // 正常换源:同 id 不漂移,enabled/position 保留
        let js2 = tmp.join("k2.user.js");
        fs::write(
            &js2,
            "// ==UserScript==\n// @name k\n// ==/UserScript==\nkeep2();",
        )
        .unwrap();
        set_enabled(&tmp, &id, false).unwrap();
        let r = update(&tmp, &id, &js2).unwrap();
        assert_eq!(r["id_moved"], false);
        assert!(r["removed"].is_null());
        let l3 = list(&tmp).unwrap();
        assert_eq!(l3["scripts"][0]["enabled"], false); // 保留,不重置
        assert_eq!(l3["scripts"][0]["position"], 0); // 同位,不漂移(评审 F3 附带)
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn 坏index报错不吞空册() {
        let tmp = std::env::temp_dir().join(format!("us-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let root = tmp.join("Default/Userscripts");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.json"), "{half-written").unwrap();
        assert!(list(&tmp).is_err(), "坏盘必须报错,不得当空册");
        let js = tmp.join("x.user.js");
        fs::write(
            &js,
            "// ==UserScript==\n// @name x\n// ==/UserScript==\nx();",
        )
        .unwrap();
        assert!(
            install(&tmp, &[js]).is_err(),
            "坏盘上写操作必须拒绝(防覆盖真册)"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn 同名不同id前缀不误中() {
        let tmp = std::env::temp_dir().join(format!("us-same-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let a = tmp.join("na.user.js");
        let b = tmp.join("nb.user.js");
        fs::write(
            &a,
            "// ==UserScript==\n// @name same\n// @namespace na\n// ==/UserScript==\naaa();",
        )
        .unwrap();
        fs::write(
            &b,
            "// ==UserScript==\n// @name same\n// @namespace nb\n// ==/UserScript==\nbbb();",
        )
        .unwrap();
        install(&tmp, &[a, b]).unwrap();
        // e 只可能是 nb 的 id 前缀(na 的 id 若不以 e 开头不命中),不得因同名打到另一条
        let idx = list(&tmp).unwrap(); // 端到端吃 profile,别手调内部 root 形参
        let arr = idx["scripts"].as_array().unwrap();
        let (id_a, _id_b) = (
            arr[0]["id"].as_str().unwrap(),
            arr[1]["id"].as_str().unwrap(),
        );
        // 构造只命中一条的前缀:取 id_b 的前 4 位
        let pre = &_id_b[..4];
        if !id_a.starts_with(pre) {
            let p = script_path(&tmp, pre).unwrap();
            let body = fs::read_to_string(&p).unwrap();
            assert!(
                body.contains("bbb"),
                "前缀唯一命中必须是 id_b 那条,got {body}"
            );
        }
        let _ = fs::remove_dir_all(&tmp);
    }
}
