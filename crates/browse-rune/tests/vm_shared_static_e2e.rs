//! #113 实弹回归(真 chrome,`BROWSE_E2E=1` 门):同字面量静态串参双调
//! `cdp::raw` 且首调回 Err——修前 VM 终态 `Cannot take, value is
//! M-000000`(deny 重试梯/任意同源字面量复用面),修后应得正常
//! `{"error": ...}` 回执(非法 URL 导航的 CDP 错面,零 deny 策略依赖)。

use std::path::PathBuf;

/// 起隔离 chrome:env `BROWSE_E2E_CHROME` 或托管集 pin 最新版。
fn chrome_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BROWSE_E2E_CHROME") {
        return Some(PathBuf::from(p));
    }
    let managed =
        std::path::Path::new(std::env::var("HOME").ok()?.as_str()).join(".browse-rs/chromium");
    let mut best: Option<(String, PathBuf)> = None;
    for entry in std::fs::read_dir(&managed).ok()?.flatten() {
        if !entry.path().join("chrome").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if best.as_ref().is_none_or(|(n, _)| name > *n) {
            best = Some((name, entry.path()));
        }
    }
    best.map(|(_, d)| d.join("chrome"))
}

#[tokio::test]
async fn shared_static_arg_two_raw_calls_first_err_live() {
    if std::env::var("BROWSE_E2E")
        .map(|v| v != "1")
        .unwrap_or(true)
    {
        eprintln!("跳过(设 BROWSE_E2E=1 跑实弹)");
        return;
    }
    let bin = chrome_bin().expect("chrome 不在(BROWSE_E2E_CHROME 或托管集)");
    let profile = std::env::temp_dir().join(format!("cv113-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&profile);
    let mut child = cdp::spawn::spawn_engine(&bin, &profile, true, &[]).expect("spawn");
    let ws = cdp::spawn::wait_devtools_ready(&profile)
        .await
        .expect("调试口");
    let session = cdp::Session::new();
    session.connect(&ws).await.expect("连");
    eprintln!("[e2e-live] bin={:?} profile={:?}", bin, profile);
    let src = r##"
    pub async fn main() {
        let nj = r#"{"url":"::not a url::"}"#;
        match cdp::raw("Page.navigate", nj).await {
            Ok(_) => { return Ok("ok-first"); }
            Err(e1) => {
                time::sleep(time::Duration::from_millis(100)).await;
                match cdp::raw("Page.navigate", nj).await {
                    Ok(_) => { return Ok("ok-second"); }
                    Err(e2) => { return Ok(format!("ladder:{}|{}", e1.op, e2.op)); }
                }
            }
        }
    }
    "##;
    let out = browse_rune::run(
        session.clone(),
        src,
        browse_rune::DEFAULT_CAPS,
        Default::default(),
    )
    .await;
    let _ = session.close().await;
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);
    // 修前:Err(Rune VM 终态错 Cannot take);修后:ladder 回执真达
    let v = out.expect("#113 修后不得 VM 终态错");
    let text = v.to_string();
    assert!(text.contains("ladder"), "回执应含 ladder 双错流,得: {text}");
}
