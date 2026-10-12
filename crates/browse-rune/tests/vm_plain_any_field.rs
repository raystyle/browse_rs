//! #113 定界对照 F:Any 直返值字段取(非 Result 载荷)。

use rune::Module;
use std::sync::{Arc, Mutex};

/// 对照 F:非 Result 载荷的 Any 直返值字段取(#113 定界)。
#[tokio::test]
async fn plain_any_field_get() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    #[derive(Debug, Clone, rune::Any)]
    #[rune(item = ::cdp)]
    struct FailErr {
        #[rune(get)]
        op: String,
        #[rune(get)]
        message: String,
    }
    let mut context = rune::Context::with_default_modules().unwrap();
    let mut m = Module::with_crate("cdp").unwrap();
    m.ty::<FailErr>().unwrap();
    m.function("mk", || FailErr {
        op: String::from("raw"),
        message: String::from("m0"),
    })
    .build()
    .unwrap();
    let sink = seen.clone();
    m.function("record", move |s: String| {
        sink.lock().unwrap().push(s);
    })
    .build()
    .unwrap();
    context.install(m).unwrap();
    let src = r#"
    pub async fn main() {
        let v = cdp::mk();
        cdp::record(format!("plain: {} / {}", v.op, v.message));
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
    let rt = Arc::new(context.runtime().unwrap());
    let vm = rune::Vm::new(rt, Arc::new(unit));
    let out = vm
        .send_execute(["__w"], ())
        .expect("派发")
        .async_complete()
        .await;
    if let Err(e) = out.into_result() {
        let s = e.to_string();
        panic!("F 败: {}", &s[..s.len().min(140)]);
    }
    assert_eq!(seen.lock().unwrap().concat(), "plain: raw / m0");
}
