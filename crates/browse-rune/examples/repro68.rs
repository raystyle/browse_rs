//! #68 规范四形之首(canon):while 加重赋值加 for in 加 Vec::push。
//! 修复判据:60 秒内回执 ok;冷引擎复跑三遍无挂死。跑法:
//! `cargo run -p browse-rune --example repro68`。
fn main() {
    let script = std::fs::read_to_string("/tmp/repro68.rn").unwrap_or_else(|_| {
        r#"pub async fn main() {
        let value = 0;
        while value < 50 { value = value + 1; }
        let v = Vec::new();
        for x in ["a", "b", "c"] { v.push(x); }
        Ok(format!("canon-ok value={} vec={}", value, v.len()))
    }"#
        .to_string()
    });
    #[tokio::main(flavor = "multi_thread")]
    async fn run(script: String) {
        let t = std::time::Instant::now();
        let out = browse_rune::run(
            cdp::Session::new(),
            &script,
            browse_rune::DEFAULT_CAPS,
            browse_rune::RunOpts::default(),
        )
        .await;
        println!("{t:?} => {out:?}");
    }
    run(script);
}
