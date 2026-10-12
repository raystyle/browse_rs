//! #113 复现:match Err 臂内持绑定跨 await(time::sleep)后 take 崩。
//! 纯 VM 道:自定义 async 宿主 fn 回 Err,经 record 宿主回调收面,不涉 cdp 会话。

use std::sync::{Arc, Mutex};

#[test]
#[ignore = "#113 复现锚:rune 0.14.2 Any 载荷 match 绑定跨 await 恢复后取崩(UnsupportedObjectSlotIndexGet/mech 形 Cannot take);修后去 ignore 转正"]
fn err_arm_binding_survives_sleep() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async { err_arm_binding_survives_sleep_inner(seen.clone()).await });
}

async fn err_arm_binding_survives_sleep_inner(seen: Arc<Mutex<Vec<String>>>) {
    let mut context = rune::Context::with_default_modules().unwrap();
    context
        .install(rune_modules::time::module(true).unwrap())
        .unwrap();
    let sink = seen.clone();
    #[derive(Debug, Clone, rune::Any)]
    #[rune(item = ::cdp)]
    struct FailErr {
        #[rune(get)]
        op: String,
        #[rune(get)]
        message: String,
    }
    let mut m = rune::Module::new();
    m.function("failing", || async {
        Err::<i64, FailErr>(FailErr {
            op: String::from("raw"),
            message: String::from("denied by policy"),
        })
    })
    .build()
    .unwrap();
    m.function("record", move |s: String| {
        sink.lock().unwrap().push(s);
    })
    .build()
    .unwrap();
    context.install(m).unwrap();

    let src = r#"
    pub async fn main() {
        match failing().await {
            Ok(_) => record(String::from("ok")),
            Err(e1) => {
                time::sleep(time::Duration::from_millis(10)).await;
                match failing().await {
                    Ok(_) => record(String::from("ok2")),
                    Err(e2) => record(format!("denied twice: {} / {}", e1.message, e2.message)),
                }
            }
        }
    }
    "#;
    let mut sources = rune::Sources::new();
    sources
        .insert(
            rune::Source::new(
                "script",
                format!("{src}\npub async fn __w() {{ main().await }}"),
            )
            .unwrap(),
        )
        .unwrap();
    let mut diagnostics = rune::Diagnostics::new();
    let unit = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build();
    let unit = match unit {
        Ok(u) => u,
        Err(_) => {
            let mut buf = rune::termcolor::Buffer::no_color();
            let _ = diagnostics.emit(&mut buf, &sources);
            panic!("编译应过: {}", String::from_utf8_lossy(buf.as_slice()));
        }
    };
    let rt = Arc::new(context.runtime().unwrap());
    let vm = rune::Vm::new(rt, Arc::new(unit));
    let out = vm
        .send_execute(["__w"], ())
        .expect("派发")
        .async_complete()
        .await;
    out.into_result()
        .expect("Err 臂绑定跨 await 点应存活(#113)");
    let got = seen.lock().unwrap().concat();
    assert_eq!(got, "denied twice: denied by policy / denied by policy");
}
