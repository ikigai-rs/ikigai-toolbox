//! The README's host, for real: ikigai-fs's `FileEndpoint` over a build directory,
//! bound at `ikigai_toolbox::DIST`, and the toolbox beside it.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use futures::executor::block_on;
use ikigai_core::{Capability, EndpointSpace, Fallback, Iri, Kernel, Request, UriTemplate, Verb};

fn host(dist: PathBuf) -> Kernel {
    let files = EndpointSpace::new().bind(
        UriTemplate::parse("urn:iki:toolbox:dist:{path}").unwrap(),
        ikigai_fs::FileEndpoint::new(dist),
    );
    Kernel::new(Arc::new(Fallback::new(vec![
        Arc::new(ikigai_toolbox::space()),
        Arc::new(files),
    ])))
}

fn source(kernel: &Kernel, iri: &str) -> ikigai_core::Result<ikigai_core::Representation> {
    let request = Request::new(Verb::Source, Iri::parse(iri).unwrap());
    block_on(kernel.issue(request, &Capability::root()))
}

#[test]
fn a_host_serves_the_build_through_ikigai_fs() {
    let dir = std::env::temp_dir().join(format!("ikigai-toolbox-host-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.js"), "export {}\n").unwrap();
    std::fs::write(dir.join("engine.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let kernel = host(dir.clone());

    let js = source(&kernel, "urn:toolbox:app.js").expect("app.js");
    assert_eq!(js.repr_type.media_type, "text/javascript");
    assert_eq!(js.bytes, b"export {}\n");
    let wasm = source(&kernel, "urn:toolbox:engine.wasm").expect("engine.wasm");
    assert_eq!(wasm.repr_type.media_type, "application/wasm");
    assert_eq!(&wasm.bytes[..4], b"\0asm", "bytes, not a string");
    match source(&kernel, "urn:toolbox:style.css") {
        Err(ikigai_core::Error::NotFound(why)) => {
            assert!(why.starts_with("urn:toolbox:style.css: "), "{why}")
        }
        other => panic!("a file the build lacks is NotFound, got {other:?}"),
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The cost of serving the engine uncached: ikigai-fs reads are live by default, so
/// every `urn:toolbox:engine.wasm` is a fresh ~8.3 MB read. Needs `./build.sh` first;
/// run with `cargo test --release --test host -- --ignored --nocapture`.
#[test]
#[ignore = "needs dist/ from ./build.sh"]
fn measure_an_uncached_engine_read() {
    let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist");
    let kernel = host(dist);
    let mut times = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let rep = source(&kernel, "urn:toolbox:engine.wasm").expect("engine.wasm");
        times.push(start.elapsed());
        assert_eq!(rep.repr_type.media_type, "application/wasm");
        assert!(rep.bytes.len() > 1_000_000);
    }
    times.sort();
    println!(
        "uncached urn:toolbox:engine.wasm: min {:?}, median {:?}, max {:?} (10 reads)",
        times[0], times[5], times[9]
    );
}
