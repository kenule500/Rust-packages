SYNCFORGE — THE BENEFITS

The second public package. The honest case for it, by audience — developers,
the Rust ecosystem, and a business like GIA. Not marketing. Actual reasons
someone would choose it, and the costs they accept by doing so.

Technical contract (modules, traits, usage): 21_Packages_To_Build.md,
PACKAGE 2 — syncforge. This document is the why; that document is the what.

---

WHY IT EXISTS

Offline-first sounds like a feature. It is actually a distributed-systems
problem wearing a mobile app's clothes.

The moment a field technician loses signal in a basement, a crawl space, or a
parking garage, the app has to answer four questions correctly:

  1. Did the action happen?             (yes — persist it locally)
  2. Has it already been sent?          (maybe — idempotency)
  3. What if the server saw a newer     (conflict)
     version while we were offline?
  4. What time did it actually happen?  (audit timestamp, not sync time)

Most apps answer these with a queue table, a "synced" boolean, and hope. It
works until it doesn't: a duplicate invoice, a lost photo, a job marked
complete twice, a timestamp that says 3:14 PM when the tech was there at
9:10 AM. Then the support ticket is not "the sync is broken." It is "you
charged my customer twice."

Every offline-first app rebuilds the same primitives: a durable queue, an
idempotency scheme, retry with backoff, batch upload, conflict handling,
connectivity detection, and observability. They rebuild them slightly wrong
each time.

syncforge implements those primitives once, correctly, and — because both the
client and the server can be Rust — shares the same types, validation, and
idempotency logic on both sides.

GIA's field app is the first consumer. It is not the only possible one.

---

WHAT SYNCFORGE IS

A library for building offline-capable Rust applications.

It gives you:

  - A durable local action queue
  - Replay on reconnect
  - Idempotency keys (UUID v7)
  - Conflict resolution strategies
  - Audit-accurate client timestamps
  - Batch upload with per-action results
  - Storage and transport traits (SQLite/HTTP by default)
  - Connectivity detection
  - Tracing metrics

It runs on the client (desktop, mobile via Tauri, WASM) and on the server
(Axum, or any Rust HTTP stack). The same crate. The same action types. The
same validation. The same idempotency rules.

It is not a database, not a backend-as-a-service, not a CRDT, and not a
hosted sync service. It is the layer between your local store and your API
that makes replay safe.

---

BENEFITS FOR DEVELOPERS USING SYNCFORGE

1. ONE SYNC MODEL, BOTH SIDES
   Without syncforge, the client has a queue implementation and the server
   has a different one. Their ideas of "already processed," "conflict," and
   "timestamp" drift. You find out in production.

   With syncforge, the `Action`, `ActionId`, `Batch`, and `BatchResult` types
   are shared. The client and server cannot disagree about the shape of a
   replay because they compile against the same definitions.

2. A DURABLE QUEUE, NOT AN IN-MEMORY VECTOR
   Actions are appended to storage before the UI says "saved." A crash, a
   force-quit, or a dead battery does not lose the write. The queue survives
   and replays on the next launch.

   The default storage is SQLite. The `Storage` trait lets you use Postgres,
   RocksDB, IndexedDB, or an in-memory store for tests.

3. IDEMPOTENCY BY DEFAULT
   Every action gets a UUID v7 key at creation time. The server checks the
   key before executing. If the same action arrives twice — because the
   response was lost, because the user tapped twice, because the batch was
   retried — the second one is deduplicated.

   This is the difference between "at-least-once delivery" and "at-least-once
   delivery without duplicate side effects." For an invoicing or job-status
   system, that difference is the whole game.

4. CONFLICT STRATEGIES INSTEAD OF AD HOC MERGES
   syncforge ships the strategies most apps actually need:

     LastWriteWins      server timestamp authority
     ServerWins         server state is canonical
     ClientWins         client state is canonical
     Custom             your merger, as a trait

   You pick one per entity or per action. You do not re-invent "well, if the
   server version is newer but the client was offline longer..." in every
   handler.

5. AUDIT-ACCURATE TIMESTAMPS
   An action carries the time it happened on the device, not the time it
   finally reached the server. The server trusts the client within a
   configurable tolerance and records both.

   For a field service business, this matters. "Job completed at 4:52 PM" is
   a payroll and billing fact, not a network artifact. For compliance, the
   original timestamp is evidence.

6. STORAGE-AGNOSTIC
   The `Storage` trait means syncforge does not force a database on you.
   SQLite is the default because it is the right default for a mobile/desktop
   app. Postgres is a reasonable server-side choice. In-memory is how you
   test replay deterministically.

7. TRANSPORT-AGNOSTIC
   The `Transport` trait means syncforge does not force HTTP. HTTP is the
   default because it is ubiquitous. gRPC, WebSocket, or a mock transport for
   tests are all implementable without changing the queue or conflict logic.

8. OFFLINE BY DEFAULT, NOT AS AN AFTERTHOUGHT
   The app does not need a special "offline mode." It writes to the queue,
   the queue persists, and the engine replays when connectivity returns. The
   UI can show "3 changes pending" without knowing how the sync works.

9. TESTABLE REPLAY
   The hardest part of offline sync is testing it. syncforge gives you a mock
   transport and in-memory storage, so you can write deterministic tests:

     - enqueue 100 actions
     - drop the connection
     - replay with a 500 on action 42
     - assert 99 succeeded, 1 retried, 0 duplicated

   That test is miserable to build by hand and trivial with the right seams.

10. OBSERVABILITY THAT MATCHES THE PROBLEM
    Queue depth, replay success/failure, conflict count, and sync duration are
    first-class metrics emitted through `tracing`. You can see a poison
    message before a user complains.

11. PLATFORM-AGNOSTIC
    Desktop, mobile (Tauri), server, WASM, and — with feature flags — embedded
    environments. The queue and conflict logic do not depend on a platform
    API. Connectivity detection does, and it is pluggable.

12. RUST TYPE SAFETY END TO END
    The action payload is a typed enum, not a JSON blob with stringly-typed
    fields. The server decodes the same enum. A field rename is a compile
    error on both sides, not a runtime surprise in a replay.

---

BENEFITS FOR THE RUST ECOSYSTEM

1. FILLS A REAL GAP
   Rust has excellent HTTP clients, databases, and serialization. It does not
   have a mature, storage- and transport-agnostic offline sync engine that
   runs on both client and server. Most Rust apps either hand-roll a queue or
   adopt a platform-specific sync service.

2. STANDARDIZES A HARD PROBLEM
   Idempotency, replay ordering, and conflict resolution are solved
   differently in every project. A shared library gives the ecosystem one
   vocabulary and one set of invariants.

3. SHARED FOUNDATION FOR MOBILE AND EDGE
   Rust is increasingly used for mobile cores (Tauri, UniFFI) and edge
   workloads. Those are exactly the environments where offline-first matters.
   syncforge gives that work a common substrate.

4. CONTRIBUTORS CAN ADD VALUE WITHOUT OWNING THE WHOLE ENGINE
   A contributor can add a storage adapter (RocksDB, IndexedDB), a transport
   (gRPC, WebSocket), or a conflict strategy without understanding the entire
   crate. The traits are the contribution surface.

5. RAISES THE FLOOR
   When the default queue is durable, the default replay is idempotent, and
   the default conflict policy is explicit, every app built on it is less
   likely to double-charge a customer. That is a ecosystem-level improvement,
   not just a library feature.

---

BENEFITS FOR BUSINESSES (LIKE GIA)

1. THE FIELD APP WORKS WHERE THE WORK IS
   Crawl spaces, basements, rural properties, metal roofs, parking garages.
   The technician can capture a job, a photo, a material log, and a signature
   without signal. The sync happens later, without a "try again" dialog.

2. NO LOST JOB UPDATES
   The update is persisted before the UI confirms it. A crash does not lose
   the "completed" status. A lost response does not lose the invoice.

3. NO DUPLICATE INVOICES OR JOBS
   Idempotency is built into the write path, not bolted onto the payment
   handler. The most expensive bug in field service — charging twice — becomes
   structurally unlikely.

4. AUDIT-ACCURATE RECORDS
   The time a job was completed, a photo was taken, or a signature was
   captured is preserved. That matters for payroll, billing disputes, warranty
   claims, and insurance.

5. LOWER SUPPORT BURDEN
   "It didn't save" and "it charged twice" are the two support categories
   offline apps create. A durable queue and idempotent replay remove most of
   both.

6. ONE CODEBASE, LESS DRIFT
   The same action definitions and validation run on the client and server.
   There is no separate "mobile sync API" with its own bugs.

7. A STRATEGIC ASSET
   Like wirekit, syncforge is independent of GIA's core product. If GIA has a
   bad year, the library still has its own trajectory, contributors, and
   reputation. It is an asset, not just infrastructure.

---

THE HARD PROBLEMS, EXPLAINED

These are the failure modes syncforge is designed around. If you are
evaluating it, this is the list to test.

1. LOST WRITES
   The user taps "Complete," the app writes to memory, the process dies.
   Without a durable queue, the write is gone. syncforge persists before
   acknowledging.

2. DUPLICATE EXECUTION
   The server processes the action, the response is lost, the client retries.
   Without idempotency, the action runs twice. syncforge sends a UUID v7 key
   and the server dedupes.

3. CONFLICTS
   The client was offline for two hours; the dispatcher reassigned the job;
   the technician marked it complete. Which wins? syncforge makes the policy
   explicit instead of accidental.

4. CLOCK SKEW
   The device clock is wrong, or the timezone changed. The action timestamp is
   still the best record of when it happened. syncforge carries it and lets
   the server apply a tolerance.

5. PARTIAL BATCH FAILURE
   A batch of 50 actions succeeds for 49 and fails for 1. Without per-action
   results, the client either retries all 50 (duplicates) or drops the one
   (lost write). syncforge returns a `BatchResult` with per-action status.

6. POISON MESSAGES
   One action always fails validation. Without a dead-letter path, it blocks
   the queue forever. syncforge marks it failed after N attempts and lets the
   app surface it, instead of silently stopping sync.

7. QUEUE GROWTH
   A device offline for a week accumulates thousands of actions. Without
   batching and compression, replay is slow and fragile. syncforge batches
   and lets you cap/observe queue depth.

8. SCHEMA DRIFT
   The client and server disagree about the action payload. Without shared
   types, this is a runtime error in production. syncforge shares the types
   and fails at compile time.

---

BENEFITS COMPARED TO ALTERNATIVES

VERSUS BUILDING YOUR OWN QUEUE:

  Building your own:
    - Every app duplicates the same primitives
    - Idempotency is usually added after the first duplicate
    - Conflict handling is ad hoc
    - Tests are written late, if at all

  syncforge:
    - The primitives exist on day one
    - The failure modes are named and tested
    - The conflict policy is explicit
    - The seams are designed for deterministic tests

VERSUS A HOSTED SYNC SERVICE (Firebase, Supabase Realtime, PowerSync,
ElectricSQL, Realm):

  Hosted/DB-specific:
    - Fast to start
    - You inherit their data model, pricing, and availability
    - Server-side business logic is constrained or external
    - Migration is expensive once you depend on it

  syncforge:
    - You own the server and the data
    - You define the action schema and conflict policy
    - Works with your existing API and database
    - No per-seat or per-sync pricing
    - More wiring up front

  The honest trade-off: a hosted service gets you to a demo faster. syncforge
  gets you to a system you control. If you are a small app with no backend,
  a hosted service may be the right call. If you already have a Rust API and
  a domain model, syncforge fits.

VERSUS A MESSAGE QUEUE (Kafka, RabbitMQ, NATS):

  A message queue is server-side infrastructure. It does not solve the
  device-side problem: the phone was offline, the action happened, and it
  needs to be persisted locally and replayed later, idempotently.

  syncforge is the device-to-server layer. It can hand off to a message queue
  on the server if you need one, but it does not replace it.

VERSUS A CRDT LIBRARY (Automerge, Yjs, cr-sqlite):

  CRDTs solve concurrent editing of shared documents. They are excellent for
  text, canvases, and collaborative state. They are not a complete offline
  action queue, and they do not give you idempotency, audit timestamps, or
  server-side business validation.

  syncforge can carry CRDT payloads as actions. It is complementary, not
  competitive.

VERSUS A DATABASE-SPECIFIC SYNC TOOL:

  Tools that sync a specific database (SQLite-to-Postgres, etc.) are useful
  when the data model is simple and the conflict policy is "last write wins
  everywhere." They are a poor fit when the server must validate business
  rules, emit events, or enforce permissions per action.

  syncforge is action-oriented, not table-oriented. That is more work and more
  control.

---

ARCHITECTURE IN BRIEF

The full module spec is in 21_Packages_To_Build.md. The shape is:

  queue.rs        append, list pending, mark sent/failed
  replay.rs       group, order, send, handle partial failure
  conflict.rs     LastWriteWins / ServerWins / ClientWins / Custom
  batch.rs        batch size, compression, timeout
  idempotency.rs  UUID v7 keys, server-side dedupe
  timestamp.rs    client time, server tolerance, audit record
  transport.rs    HTTP default; gRPC/WebSocket/mock
  storage.rs      SQLite default; Postgres/RocksDB/memory
  reconnect.rs    polling, event-based, manual
  metrics.rs      queue depth, replay rate, conflicts, duration

The flow:

  enqueue
    -> persist to local storage
    -> UI acknowledges
    -> on reconnect: batch pending actions in order
    -> send batch
    -> server validates, dedupes, executes, returns per-action result
    -> client marks sent / retries / marks failed
    -> conflict policy applies where the server reports a conflict

The core types:

  Action {
    id: ActionId,              // UUID v7
    entity: EntityType,
    operation: Operation,
    payload: serde_json::Value, // or a typed enum
    client_timestamp: DateTime<Utc>,
    idempotency_key: Uuid,
    attempts: u32,
  }

  BatchResult {
    results: Vec<ActionResult>, // one per action
  }

  ActionResult {
    action_id: ActionId,
    status: Applied | Duplicate | Conflict | Rejected | Retry,
    server_timestamp: Option<DateTime<Utc>>,
    conflict: Option<ConflictInfo>,
  }

---

USAGE (SHAPE, NOT FINAL API)

CLIENT:

  let engine = SyncEngine::builder()
      .storage(SqliteStorage::open("local.db").await?)
      .transport(HttpTransport::new("https://api.example.com/sync"))
      .idempotency(IdempotencyScheme::UuidV7)
      .reconnect(ReconnectStrategy::Polling(Duration::from_secs(5)))
      .build()?;

  engine.enqueue(Action::UpdateStatus {
      entity_id: "job_123",
      status: "COMPLETED",
      timestamp: Utc::now(),
  }).await?;

  // The queue persists and replays on reconnect. No further code required.

SERVER:

  async fn sync_handler(
      State(handler): State<BatchHandler>,
      Json(batch): Json<Batch>,
  ) -> Result<Json<BatchResult>, StatusCode> {
      handler.process(batch).await
  }

  The handler:
    - validates the batch
    - checks idempotency keys
    - executes each action through your domain service
    - returns per-action results

The API will change before 1.0. Treat this as the shape, not a contract.

---

WHAT SYNCFORGE IS NOT

  - Not a database. It stores a queue; your domain data lives in your store.
  - Not a backend. You run the server and the handler.
  - Not a CRDT. It can carry CRDT payloads, but it does not merge documents.
  - Not auth or permissions. Your server still authorizes every action.
  - Not a schema migration tool. Action versions are your responsibility.
  - Not exactly-once magic. It provides at-least-once delivery plus
    idempotency keys; your handler must be idempotent for side effects that
    cannot be deduped by the key alone.

---

THE HONEST TRADE-OFFS

  Benefit:  You own the sync layer
  Cost:     You own the server, the handler, and the conflict policy

  Benefit:  Storage and transport agnostic
  Cost:     More wiring than a batteries-included backend

  Benefit:  Shared types on client and server
  Cost:     Both sides must be Rust (or you re-implement the types)

  Benefit:  Explicit conflict strategies
  Cost:     You must decide which one each entity needs

  Benefit:  Durable queue
  Cost:     You must manage queue growth and dead letters

  Benefit:  Open source
  Cost:     Public maintenance, PR review, and support expectations

  The trade-offs are worth it for a product with a real field app and a Rust
  backend. They are not worth it for a weekend prototype with no offline
  requirement.

---

BENEFITS FOR THE CREATOR (STRATEGIC)

1. REPUTATION
   "The Rust offline-first sync engine" is a meaningful position. It is a
   harder problem than another HTTP wrapper, and it attracts a different kind
   of contributor.

2. FOUNDATION FOR PRODUCTS
   syncforge could power:
     - A sync observability service (queue depth, conflict rates per customer)
     - A conflict-resolution UI toolkit
     - A hosted relay for teams that do not want to run the server
     - A mobile/edge data layer

3. ACQUISITION INTEREST
   Companies building mobile, edge, and local-first products have acquired
   infrastructure libraries. A mature sync engine is a plausible target.

4. TALENT MAGNET
   Engineers who care about distributed systems and local-first software want
   to work on this. It is a stronger hiring signal than a CRUD app.

5. INDEPENDENT OF GIA
   If GIA has a bad year, syncforge does not. It is a separate asset with its
   own users and trajectory.

---

BENEFITS THAT COMPOUND OVER TIME

  YEAR 1:
    - GIA's field app uses it in production
    - A few external users try it
    - Queue, replay, idempotency, SQLite, HTTP

  YEAR 2:
    - Community storage/transport adapters
    - Conflict strategies beyond the defaults
    - WASM and gRPC support
    - First companies adopt it as a dependency

  YEAR 3:
    - The default choice for Rust offline-first apps
    - Referenced in local-first and edge discussions
    - Potential acquisition or hosted-product interest

The compounding comes from the same loop as wirekit: each adapter and each
user makes the library more useful, and each new use case attracts more
adapters.

---

SUMMARY

The single biggest benefit:

  Offline sync is a distributed-systems problem, and most apps solve it by
  accident. syncforge makes the hard parts explicit, shared, and testable.

Everything else follows:

  - Durable queue -> no lost writes
  - Idempotency -> no duplicates
  - Conflict strategies -> no accidental overwrites
  - Client timestamps -> audit accuracy
  - Storage/transport traits -> no lock-in
  - Shared types -> no client/server drift

For GIA, syncforge is how the field app works in a basement without losing a
job, a photo, or an invoice. For the Rust ecosystem, it is a missing piece of
local-first infrastructure. For the developer using it, it is the difference
between a queue table and a sync engine.

If wirekit is how Rust talks to the world, syncforge is how Rust remembers
what happened while it was disconnected.
