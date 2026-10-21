//! Sign, verify, tamper-check, revoke, and consume links offline.
//!
//! The example uses the in-memory store and a hand-rolled executor because the
//! store contract is async and the crate ships no runtime.

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use signedlink::{
    generate, Claims, Code, LinkError, LinkStore, MemoryLinkStore, SignedToken, SignedUrl, Signer,
    SigningKey, TenantId, Utm,
};
use url::Url;

fn block_on<F: Future>(future: F) -> F::Output {
    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let now = 1_800_000_000_i64;
    let key = SigningKey::new(b"example-signing-key-with-32-bytes")?;
    let signer = Signer::new(key, Duration::from_secs(3_600), Duration::from_secs(30))?;
    let store = MemoryLinkStore::new();

    let utm = Utm::from_pairs([
        ("utm_source", "newsletter"),
        ("utm_medium", "email"),
        ("utm_campaign", "launch"),
    ])?;
    let claims = Claims::new(
        Url::parse("https://example.com/landing")?,
        TenantId::new("acme")?,
        Duration::from_secs(86_400),
    )?
    .with_campaign("launch")?
    .with_utm(utm)?;

    let token: SignedToken = signer.sign(&claims, now)?;
    let code: Code = block_on(generate(&store))?;
    block_on(store.put(&code, token.claims()))?;
    let public = SignedUrl::new(&Url::parse("https://links.example.com/r")?, &code, &token)?;

    println!("token: {token}");
    println!("code: {code}");
    println!("url: {public}");
    println!("url length: {}", public.expose_url().len());
    println!("canonical: {}", token.claims().canonical());

    let verified = signer.verify(token.expose_token(), now + 10)?;
    println!(
        "verified: tenant={} campaign={:?} utm_source={:?} single_use={}",
        verified.tenant, verified.campaign, verified.utm.source, verified.single_use
    );
    let _ = block_on(signer.verify_with_store(token.expose_token(), &code, now + 10, &store))?;

    let mut tampered: Vec<char> = token.expose_token().chars().collect();
    tampered[0] = if tampered[0] == 'A' { 'B' } else { 'A' };
    let tampered: String = tampered.into_iter().collect();
    match signer.verify(&tampered, now + 10) {
        Err(LinkError::Tampered) => println!("tamper rejected"),
        other => println!("unexpected tamper result: {other:?}"),
    }

    let revoked_code = block_on(generate(&store))?;
    block_on(store.put(&revoked_code, token.claims()))?;
    block_on(store.revoke(&revoked_code))?;
    match block_on(signer.verify_with_store(token.expose_token(), &revoked_code, now + 10, &store))
    {
        Err(LinkError::Revoked) => println!("revocation rejected"),
        other => println!("unexpected revocation result: {other:?}"),
    }

    let once_claims = token.claims().clone().with_single_use(true);
    let once_token = signer.sign(&once_claims, now)?;
    let once_code = block_on(generate(&store))?;
    block_on(store.put(&once_code, once_token.claims()))?;
    let first =
        block_on(signer.verify_with_store(once_token.expose_token(), &once_code, now + 10, &store));
    let second =
        block_on(signer.verify_with_store(once_token.expose_token(), &once_code, now + 10, &store));
    println!(
        "single use: first_ok={} second={:?}",
        first.is_ok(),
        second.err().map(|error| error.code())
    );
    Ok(())
}
