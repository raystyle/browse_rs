//! #113 复现与回归:match Err 臂绑定跨 await、Any 字段取、同字面量双参调。

use std::sync::{Arc, Mutex};

/// 对照 A:同形无 sleep——分界 await 恢复 vs Any 字段取本身。
#[test]
fn err_arm_binding_no_sleep() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async { err_arm_no_sleep_inner(seen.clone()).await });
    let got = seen.lock().unwrap().concat();
    assert_eq!(got, "denied twice: denied by policy / denied by policy");
}

async fn err_arm_no_sleep_inner(seen: Arc<Mutex<Vec<String>>>) {
    #[derive(Debug, Clone, rune::Any)]
    #[rune(item = ::cdp)]
    struct FailErr3 {
        #[rune(get)]
        op: String,
        #[rune(get)]
        message: String,
    }
    let mut context = rune::Context::with_default_modules().unwrap();
    let mut m = rune::Module::new();
    m.function("failing", || async {
        Err::<i64, FailErr3>(FailErr3 {
            op: String::from("raw"),
            message: String::from("denied by policy"),
        })
    })
    .build()
    .unwrap();
    let sink = seen.clone();
    m.function("record", move |s: String| {
        sink.lock().unwrap().push(s);
    })
    .build()
    .unwrap();
    m.ty::<FailErr3>().unwrap();
    context.install(m).unwrap();
    let src = r#"
    pub async fn main() {
        match failing().await {
            Ok(_) => record(String::from("ok")),
            Err(e1) => {
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
                format!(
                    "{src}
pub async fn __w() {{ main().await }}"
                ),
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
    let rt = std::sync::Arc::new(context.runtime().unwrap());
    let vm = rune::Vm::new(rt, std::sync::Arc::new(unit));
    let out = vm
        .send_execute(["__w"], ())
        .expect("派发")
        .async_complete()
        .await;
    out.into_result().expect("对照A(无sleep)应存活");
}

/// 对照 B:Any 绑定跨 await 后直接传宿主函数(镜像 cdp::__describe 消费形)。
#[test]
fn err_arm_pass_to_host() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async { err_arm_pass_to_host_inner(seen.clone()).await });
    let got = seen.lock().unwrap().concat();
    assert_eq!(got, "described: raw");
}

async fn err_arm_pass_to_host_inner(seen: Arc<Mutex<Vec<String>>>) {
    #[derive(Debug, Clone, rune::Any)]
    #[rune(item = ::cdp)]
    struct FailErr2 {
        #[rune(get)]
        op: String,
        #[rune(get)]
        message: String,
    }
    let mut context = rune::Context::with_default_modules().unwrap();
    context
        .install(rune_modules::time::module(true).unwrap())
        .unwrap();
    let mut m = rune::Module::new();
    m.function("failing", || async {
        Err::<i64, FailErr2>(FailErr2 {
            op: String::from("raw"),
            message: String::from("m"),
        })
    })
    .build()
    .unwrap();
    let sink = seen.clone();
    m.function("describe", move |e: FailErr2| {
        sink.lock().unwrap().push(format!("described: {}", e.op));
    })
    .build()
    .unwrap();
    m.ty::<FailErr2>().unwrap();
    context.install(m).unwrap();
    let src = r#"
    pub async fn main() {
        match failing().await {
            Ok(_) => (),
            Err(e1) => {
                time::sleep(time::Duration::from_millis(10)).await;
                describe(e1)
            }
        }
    }
    "#;
    let mut sources = rune::Sources::new();
    sources
        .insert(
            rune::Source::new(
                "script",
                format!(
                    "{src}
pub async fn __w() {{ main().await }}"
                ),
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
    let rt = std::sync::Arc::new(context.runtime().unwrap());
    let vm = rune::Vm::new(rt, std::sync::Arc::new(unit));
    let out = vm
        .send_execute(["__w"], ())
        .expect("派发")
        .async_complete()
        .await;
    if let Err(e) = out.into_result() {
        panic!(
            "KIND={}",
            e.to_string().chars().take(160).collect::<String>()
        );
    }
}

/// 对照 C:Ok 载荷为 rune Value(镜像真 cdp::raw 的 Result<RuneValue, CdpError> 签名)。
#[test]
fn err_value_payload_second_call() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async { err_value_payload_second_call_inner(seen.clone()).await });
    assert_eq!(seen.lock().unwrap().concat(), "second-ok");
}

async fn err_value_payload_second_call_inner(seen: Arc<Mutex<Vec<String>>>) {
    #[derive(Debug, Clone, rune::Any)]
    #[rune(item = ::cdp)]
    struct FailErr4 {
        #[rune(get)]
        op: String,
        #[rune(get)]
        message: String,
    }
    let mut context = rune::Context::with_default_modules().unwrap();
    let mut m = rune::Module::with_crate("cdp").unwrap();
    m.function("failing", || async {
        Err::<rune::runtime::Value, FailErr4>(FailErr4 {
            op: String::from("raw"),
            message: String::from("m"),
        })
    })
    .build()
    .unwrap();
    let sink = seen.clone();
    m.function("record", move |s: String| {
        sink.lock().unwrap().push(s);
    })
    .build()
    .unwrap();
    m.ty::<FailErr4>().unwrap();
    context.install(m).unwrap();
    let src = r#"
    pub async fn main() {
        match cdp::failing().await {
            Ok(_) => cdp::record(String::from("ok")),
            Err(_) => {
                match cdp::failing().await {
                    Ok(_) => cdp::record(String::from("ok2")),
                    Err(_) => cdp::record(String::from("second-ok")),
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
                format!(
                    "{src}
pub async fn __w() {{ main().await }}"
                ),
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
    let rt2 = Arc::new(context.runtime().unwrap());
    let vm = rune::Vm::new(rt2, Arc::new(unit));
    let out = vm
        .send_execute(["__w"], ())
        .expect("派发")
        .async_complete()
        .await;
    out.into_result().expect("对照C应存活");
}

/// #113 真回归锚:同字面量静态串参双调宿主函数且首调回 Err——
/// 0.14.2 原版共享 access 格被首次取参毒化,第二调 Cannot take(修前实弹)。
#[test]
fn shared_static_arg_two_calls_first_err() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async { shared_static_arg_inner(seen.clone()).await });
    assert_eq!(seen.lock().unwrap().concat(), "second-err:raw");
}

async fn shared_static_arg_inner(seen: Arc<Mutex<Vec<String>>>) {
    #[derive(Debug, Clone, rune::Any)]
    #[rune(item = ::cdp)]
    struct FailErr5 {
        #[rune(get)]
        op: String,
    }
    let mut context = rune::Context::with_default_modules().unwrap();
    let mut m = rune::Module::with_crate("cdp").unwrap();
    m.function("failing", |_: String| async {
        Err::<i64, FailErr5>(FailErr5 {
            op: String::from("raw"),
        })
    })
    .build()
    .unwrap();
    let sink = seen.clone();
    m.function("record", move |s: String| {
        sink.lock().unwrap().push(s);
    })
    .build()
    .unwrap();
    m.ty::<FailErr5>().unwrap();
    context.install(m).unwrap();
    let src = r#"
    pub async fn main() {
        match cdp::failing("same-literal").await {
            Ok(_) => cdp::record(String::from("ok")),
            Err(_) => {
                match cdp::failing("same-literal").await {
                    Ok(_) => cdp::record(String::from("ok2")),
                    Err(e2) => cdp::record(format!("second-err:{}", e2.op)),
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
        .build()
        .expect("编译应过");
    let rt2 = Arc::new(context.runtime().unwrap());
    let vm = rune::Vm::new(rt2, Arc::new(unit));
    let out = vm
        .send_execute(["__w"], ())
        .expect("派发")
        .async_complete()
        .await;
    out.into_result()
        .expect("#113 修后:同字面量双参调不得 Cannot take");
}

/// 主锚:Err(Any) 绑定跨 time::sleep + 二次调用 + 双字段取(#113 原始形)。
#[test]
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
    m.ty::<FailErr>().unwrap();
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
    // exec.rs 同款:执行走 try_clone 副本(#113 真机差异面)
    use rune::alloc::clone::TryClone;
    let vm = vm.try_clone().unwrap();
    let exec = vm.send_execute(["__w"], ()).expect("派发");
    // exec.rs 同款双限额包裹(#113 真机差异面:budget 管指令,limit 管分配)
    let budgeted = rune::runtime::budget::with(10_000_000, exec.async_complete());
    let limited = rune::alloc::limit::with(16_777_216, budgeted);
    let out = limited.await;
    out.into_result()
        .expect("Err 臂绑定跨 await 点应存活(#113)");
    let got = seen.lock().unwrap().concat();
    assert_eq!(got, "denied twice: denied by policy / denied by policy");
}
