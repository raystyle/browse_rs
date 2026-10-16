//! geelib 新标签挂起修回归(真 chrome,`BROWSE_E2E=1` 门):auto-attach 开启下
//! `window.open` 新标签 5 秒内 commit——修前 opener renderer 因上游
//! `ShouldThrottlePopups`(只看 autoAttach_ 位)在嵌套 RunLoop 等一个
//! 不会来的 resume,新标签永空;修后事件泵对每个新附着 session 幂等补发
//! `Runtime.runIfWaitingForDebugger`(kimi 定谳链,puppeteer 标准姿势)。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::json;

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
async fn auto_attach_window_open_commits() {
    if std::env::var("BROWSE_E2E")
        .map(|v| v != "1")
        .unwrap_or(true)
    {
        eprintln!("跳过(设 BROWSE_E2E=1 跑实弹)");
        return;
    }
    let bin = chrome_bin().expect("chrome 不在(BROWSE_E2E_CHROME 或托管集)");
    let profile = std::env::temp_dir().join(format!("cdp-newtab-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&profile);
    // --disable-popup-blocking:headless 默认拦弹窗,新 target 根本不建
    // (wrok 排查报告同款实证旗)
    let mut child = cdp::spawn::spawn_engine(
        &bin,
        &profile,
        true,
        &["--disable-popup-blocking".to_string()],
    )
    .expect("spawn");
    let ws = cdp::spawn::wait_devtools_ready(&profile)
        .await
        .expect("调试口");
    let session = cdp::Session::new();
    session.connect(&ws).await.expect("连");

    // 复刻 browse daemon 面:浏览器级 auto-attach(修前挂起条件全量在位)
    session.enable_auto_attach().await.expect("auto-attach 开");

    // 钉活动 page(浏览器级连接上 Runtime.* 须路由到具体 target)
    let pages = session
        .call("Target.getTargets", json!({}))
        .await
        .expect("pages");
    let page_id = pages
        .get("targetInfos")
        .and_then(|v| v.as_array())
        .and_then(|a| {
            a.iter()
                .find(|t| t.get("type").and_then(|x| x.as_str()) == Some("page"))
                .and_then(|t| t.get("targetId"))
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .expect("初始页 target");
    let attached = session
        .call(
            "Target.attachToTarget",
            json!({"targetId": page_id, "flatten": true}),
        )
        .await
        .expect("attach");
    let page_sid = attached
        .get("sessionId")
        .and_then(|v| v.as_str())
        .expect("sessionId")
        .to_string();

    // 真 URL 才走 BeginNavigation/挂起路径(about:blank 无导航,负对照
    // 实证其绕过);内置单响应 http 服务供新标签 commit 可观测
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let port = listener.local_addr().unwrap().port();
    let _srv = std::thread::spawn(move || {
        use std::io::{Read, Write};
        for _ in 0..8 {
            let Ok((mut sock, _)) = listener.accept() else {
                break;
            };
            let mut buf = [0u8; 512];
            let _ = sock.read(&mut buf);
            let body = b"<html><body>page</body></html>";
            let _ = sock.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = sock.write_all(body);
        }
    });
    let open_url = format!("http://127.0.0.1:{port}/newtab");

    // opener 真页化(对齐 wrok 现场:about:blank opener 无 DevTools agent
    // 面,负对照实证其绕过挂起链;真 http opener 才进 ShouldThrottlePopups 等待)
    session
        .call_on(
            "Page.navigate",
            json!({"url": format!("http://127.0.0.1:{port}/opener")}),
            &page_sid,
        )
        .await
        .expect("opener 导航");
    tokio::time::sleep(Duration::from_millis(600)).await;

    let t0 = Instant::now();
    let opened = session
        .call_on(
            "Runtime.evaluate",
            json!({
                "expression": format!("window.open('{open_url}','_blank') ? 'opened' : 'blocked'"),
                "returnByValue": true
            }),
            &page_sid,
        )
        .await
        .expect("evaluate window.open");
    let opened = opened
        .pointer("/result/value")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert_eq!(opened, "opened", "window.open 被拒或未返回(挂起形)");

    // 5 秒窗内新标签 url 必须 commit 出 about:blank(修前永空)
    let deadline = t0 + Duration::from_secs(5);
    let mut committed = None;
    while Instant::now() < deadline {
        let targets = session
            .call("Target.getTargets", json!({}))
            .await
            .expect("getTargets");
        let blank_pages = targets
            .get("targetInfos")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                    .filter(|t| {
                        t.get("url").and_then(|v| v.as_str()).is_some_and(|u| {
                            u.starts_with("about:blank") && u.len() > "about:blank".len() + 1
                        })
                    })
                    .count()
            })
            .unwrap_or(0);
        // window.open('about:blank') 后新标签 url=about:blank;判定以
        // 「有 url 已知的空页 target」为准(修前 title/url 双空)
        let landed = targets
            .get("targetInfos")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                    .any(|t| {
                        t.get("url")
                            .and_then(|v| v.as_str())
                            .is_some_and(|u| u.contains("/newtab"))
                    })
            })
            .unwrap_or(false);
        let _ = blank_pages;
        if landed {
            // 新标签 url commit 出导航目标 = 挂起解(修前 title/url 永空)
            committed = Some(2);
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let _ = session.close().await;
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);

    assert!(
        committed.is_some(),
        "auto-attach 下 window.open 新标签 5 秒未 commit(挂起回归)"
    );
    eprintln!(
        "新标签 {:.1}s 内 commit(geelib 挂起修回归绿)",
        t0.elapsed().as_secs_f32()
    );
}
