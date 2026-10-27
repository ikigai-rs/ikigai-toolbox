//! The body path (ledger #2290): `urn:toolbox:{name}` hands the base's body through as
//! the kernel produced it, so an fs `Replay` keeps its validator, its length and its
//! range reads, and a large file is never held whole. The bytes path is the same answer,
//! collected under the run's byte budget, and past it is `OverBudget`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::block_on;
use futures::StreamExt;
use ikigai_core::{
    Body, ByteStream, Bytes, Capability, ContentId, Endpoint, EndpointSpace, Error, Fallback,
    Invocation, Iri, Kernel, Replay, ReprType, Representation, Request, Result, UriTemplate, Verb,
};

const CHUNK: usize = 64 * 1024;

fn kernel_over(dist: impl Endpoint + 'static) -> Kernel {
    let files = EndpointSpace::new().bind(
        UriTemplate::parse("urn:iki:toolbox:dist:{path}").unwrap(),
        dist,
    );
    Kernel::new(Arc::new(Fallback::new(vec![
        Arc::new(ikigai_toolbox::space()),
        Arc::new(files),
    ])))
}

fn read(name: &str) -> Request {
    Request::new(
        Verb::Source,
        Iri::parse(format!("urn:toolbox:{name}")).unwrap(),
    )
}

fn body(kernel: &Kernel, name: &str) -> Result<Representation<Body>> {
    block_on(kernel.issue_body(read(name), &Capability::root()))
}

fn bytes(kernel: &Kernel, name: &str) -> Result<Representation> {
    block_on(kernel.issue(read(name), &Capability::root()))
}

fn drain(stream: ByteStream) -> Result<Vec<u8>> {
    block_on(async {
        let mut out = Vec::new();
        let mut stream = stream;
        while let Some(chunk) = stream.next().await {
            out.extend_from_slice(&chunk?);
        }
        Ok(out)
    })
}

/// A large file, made lazily: `len` bytes, byte `i` is `i % 251`, in chunks of
/// [`CHUNK`], counting the chunks polled from every reader. Seekable, with a length
/// and a strong validator, as a spill or a stored object answers.
struct Generated {
    len: u64,
    polled: Arc<AtomicUsize>,
    fail_at: Option<u64>,
}

fn byte_at(i: u64) -> u8 {
    (i % 251) as u8
}

impl Generated {
    fn reader(&self, from: u64) -> ByteStream {
        let (len, polled, fail_at) = (self.len, self.polled.clone(), self.fail_at);
        Box::pin(futures::stream::unfold(from, move |at| {
            let polled = polled.clone();
            async move {
                if at >= len {
                    return None;
                }
                polled.fetch_add(1, Ordering::SeqCst);
                if fail_at.is_some_and(|f| at >= f) {
                    return Some((Err(Error::Conflict("changed while read".into())), len));
                }
                let end = (at + CHUNK as u64).min(len);
                let chunk: Vec<u8> = (at..end).map(byte_at).collect();
                Some((Ok(Bytes::from(chunk)), end))
            }
        }))
    }
}

/// The validator the generated body names (any fixed id: the toolbox must hand it on).
fn validator() -> ContentId {
    ContentId::of(b"the generated engine")
}

impl Replay for Generated {
    fn open(&self) -> ByteStream {
        self.reader(0)
    }
    fn open_at(&self, offset: u64) -> Option<ByteStream> {
        Some(self.reader(offset))
    }
    fn len_hint(&self) -> Option<u64> {
        Some(self.len)
    }
    fn validator(&self) -> Option<ContentId> {
        Some(validator())
    }
}

/// The build directory as a body producer: every file is a [`Generated`] body.
struct GeneratedDist {
    len: u64,
    polled: Arc<AtomicUsize>,
    fail_at: Option<u64>,
}

impl GeneratedDist {
    fn new(len: u64) -> (GeneratedDist, Arc<AtomicUsize>) {
        let polled = Arc::new(AtomicUsize::new(0));
        let dist = GeneratedDist {
            len,
            polled: polled.clone(),
            fail_at: None,
        };
        (dist, polled)
    }
}

#[async_trait]
impl Endpoint for GeneratedDist {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        let body = self.invoke_body(inv).await?;
        body.collect_within(inv.memory_budget().expect("the kernel mints one"))
            .await
    }

    async fn invoke_body(&self, _: &Invocation<'_>) -> Result<Representation<Body>> {
        let replay = Generated {
            len: self.len,
            polled: self.polled.clone(),
            fail_at: self.fail_at,
        };
        Ok(Representation::from_body(
            ReprType::new("application/octet-stream"),
            Body::replay(replay),
        )
        .cacheable())
    }
}

fn the_replay(rep: &Representation<Body>) -> &Arc<dyn Replay> {
    match &rep.bytes {
        Body::Replay(replay) => replay,
        Body::Bytes(b) => panic!("a Replay, not re-wrapped bytes ({} bytes)", b.len()),
        Body::Once(_) => panic!("a Replay, not a one-shot stream"),
    }
}

#[test]
fn an_fs_file_is_passed_through_as_its_seekable_replay() {
    let dir = std::env::temp_dir().join(format!("ikigai-toolbox-bodies-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let engine: Vec<u8> = (0..300_000u64).map(byte_at).collect();
    std::fs::write(dir.join("engine.wasm"), &engine).unwrap();
    let kernel = kernel_over(ikigai_fs::FileEndpoint::new(PathBuf::from(&dir)));

    let rep = body(&kernel, "engine.wasm").expect("engine.wasm as a body");
    assert_eq!(rep.repr_type.media_type, "application/wasm");
    let replay = the_replay(&rep);
    assert_eq!(
        replay.len_hint(),
        Some(engine.len() as u64),
        "the length survives"
    );
    let tail = replay.open_at(100_000).expect("seekable: a range read");
    assert_eq!(drain(tail).unwrap(), &engine[100_000..]);
    assert_eq!(drain(replay.open()).unwrap(), engine, "read again, whole");

    // The body answer, collected, is the bytes answer.
    let whole = bytes(&kernel, "engine.wasm").expect("engine.wasm as bytes");
    assert_eq!(whole.repr_type, rep.repr_type);
    assert_eq!(whole.bytes, engine);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_validator_length_and_ranges_survive() {
    let (dist, polled) = GeneratedDist::new(1_000_000);
    let kernel = kernel_over(dist);
    let rep = body(&kernel, "engine.wasm").unwrap();
    assert_eq!(rep.repr_type.media_type, "application/wasm");
    let replay = the_replay(&rep);
    assert_eq!(replay.validator(), Some(validator()));
    assert_eq!(replay.len_hint(), Some(1_000_000));
    let from = 999_000u64;
    let tail = drain(replay.open_at(from).unwrap()).unwrap();
    assert_eq!(tail, (from..1_000_000).map(byte_at).collect::<Vec<_>>());
    assert_eq!(
        polled.load(Ordering::SeqCst),
        1,
        "a range read polls only its chunks"
    );
}

#[test]
fn nothing_is_read_until_asked_and_an_early_stop_stops_reading() {
    let (dist, polled) = GeneratedDist::new(10 * CHUNK as u64);
    let kernel = kernel_over(dist);
    let rep = body(&kernel, "engine.wasm").unwrap();
    assert_eq!(
        polled.load(Ordering::SeqCst),
        0,
        "no chunk read on the way through"
    );
    let mut stream = rep.bytes.into_stream();
    let first = block_on(stream.next()).unwrap().unwrap();
    assert_eq!(first.len(), CHUNK);
    drop(stream);
    assert_eq!(
        polled.load(Ordering::SeqCst),
        1,
        "one chunk asked, one chunk read"
    );
}

#[test]
fn a_file_larger_than_the_byte_budget_streams_in_constant_memory() {
    // Twice the default byte budget (64 MiB): collected anywhere on the way, it would
    // be OverBudget; streamed, no chunk is larger than the producer's.
    let len = 2 * ikigai_core::DEFAULT_STREAM_MEMORY;
    let (dist, polled) = GeneratedDist::new(len);
    let kernel = kernel_over(dist);
    let rep = body(&kernel, "engine.wasm").unwrap();
    let (total, largest) = block_on(async {
        let mut stream = rep.bytes.into_stream();
        let (mut total, mut largest) = (0u64, 0usize);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            assert_eq!(chunk[0], byte_at(total), "in order");
            total += chunk.len() as u64;
            largest = largest.max(chunk.len());
        }
        (total, largest)
    });
    assert_eq!(total, len);
    assert_eq!(largest, CHUNK, "never held whole");
    assert_eq!(polled.load(Ordering::SeqCst) as u64, len / CHUNK as u64);
}

#[test]
fn past_the_byte_budget_the_bytes_answer_is_over_budget() {
    let (dist, _) = GeneratedDist::new(1_000_000);
    let kernel = kernel_over(dist).with_stream_memory(100_000, 100_000);
    match bytes(&kernel, "engine.wasm") {
        Err(Error::OverBudget { .. }) => {}
        other => panic!("OverBudget, never a shorter answer: got {other:?}"),
    }
    // The body path refuses the same way when it is collected.
    let rep = body(&kernel, "engine.wasm").unwrap();
    match block_on(rep.bytes.collect(100_000)) {
        Err(Error::OverBudget { .. }) => {}
        other => panic!("OverBudget, never a shorter answer: got {other:?}"),
    }
}

#[test]
fn a_stream_that_fails_part_way_ends_with_its_error() {
    let polled = Arc::new(AtomicUsize::new(0));
    let dist = GeneratedDist {
        len: 4 * CHUNK as u64,
        polled,
        fail_at: Some(2 * CHUNK as u64),
    };
    let kernel = kernel_over(dist);
    let rep = body(&kernel, "engine.wasm").unwrap();
    match drain(rep.bytes.into_stream()) {
        Err(Error::Conflict(_)) => {}
        other => panic!("the error item, never a clean end: got {other:?}"),
    }
    assert!(matches!(
        bytes(&kernel, "engine.wasm"),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn the_body_path_keeps_the_bytes_paths_refusals() {
    let (dist, polled) = GeneratedDist::new(10);
    let kernel = kernel_over(dist);
    match body(&kernel, "../secret") {
        Err(Error::NotFound(why)) => assert!(why.starts_with("urn:toolbox:../secret"), "{why}"),
        other => panic!("NotFound, got {:?}", other.map(|r| r.repr_type)),
    }
    assert_eq!(polled.load(Ordering::SeqCst), 0);

    // The base's NotFound is NotFound for the name asked, on both paths.
    let empty = std::env::temp_dir().join(format!("ikigai-toolbox-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let kernel = kernel_over(ikigai_fs::FileEndpoint::new(empty.clone()));
    match body(&kernel, "style.css") {
        Err(Error::NotFound(why)) => assert!(why.starts_with("urn:toolbox:style.css: "), "{why}"),
        other => panic!("NotFound, got {:?}", other.map(|r| r.repr_type)),
    }
    std::fs::remove_dir_all(&empty).unwrap();
}
