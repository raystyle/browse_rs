//! #69 集成测试：rune 面在非 TTY 且 stdin 开而不关的调用壳里不无限
//! 阻塞（3 秒静默界放行，后续按原错误面快出）；EOF 管道道行为不变。
//! 用坏 @名走「读腿之后、daemon 之前」的 exit 2 面，避开引擎依赖。

use std::io::Write;
use std::process::{Command, Stdio};

/// 持开不关的 stdin 下，rune 面应在有限时间内出用法错（缺脚本文件，
/// exit 2），而不是挂在参数道读腿上（#69 修复前无限阻塞）。
#[test]
fn rune_open_stdin_does_not_hang() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_browse"))
        .args(["rune", "@definitely-missing-69", "--root", "/tmp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起 browse 子进程");
    // 父进程持有写端不写不关：agent 壳常开管道形
    let sink = child.stdin.take().expect("stdin 管道");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        match child.try_wait().expect("探子进程态") {
            Some(s) => break s,
            None => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "#69 回归：持开 stdin 下 rune 面挂过 20 秒（应 3 秒静默界放行）"
                );
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    };
    assert_eq!(
        status.code(),
        Some(2),
        "缺脚本文件应 exit 2（读腿放行后原错误面）"
    );
    drop(sink);
    let _ = child.wait();
}

/// EOF 对照：管道写完即关，同一命令同面快出（路径零回归）。
#[test]
fn rune_closed_stdin_pipe_unaffected() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_browse"))
        .args(["rune", "@definitely-missing-69", "--root", "/tmp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起 browse 子进程");
    let mut sink = child.stdin.take().expect("stdin 管道");
    writeln!(sink, "pipe-param-data").expect("写管道参数");
    drop(sink);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        match child.try_wait().expect("探子进程态") {
            Some(s) => break s,
            None => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "EOF 管道道不应有额外时延（读腿期内完成）"
                );
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    };
    assert_eq!(status.code(), Some(2), "缺脚本文件应 exit 2（同面）");
}
