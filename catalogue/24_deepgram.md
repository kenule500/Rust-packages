# 24 — Deepgram

**Provider:** Deepgram
**Package:** `wirekit` (module `wirekit::deepgram`)
**Status:** provider design, not a crate
**Covers:** Speech-to-text
**Base URL:** `https://api.deepgram.com/v1`
**Auth:** Bearer token

## What it does

Deepgram turns voice into text: inbound calls, technician voice notes, and
voice-to-quote. It is the STT layer behind Tool 1. GIA uses transcripts for
conversation summaries, call insights, and field notes.

The module wraps batch transcription and real-time streaming with typed
transcript, word, and utterance models.

## GIA use

- Tool 1 (Telephony & Voice) — STT
- Agents 1 (Receptionist), 2 (Missed Call), 6 (Inbox)
- Agent 20 (Image Diagnosis) — voice notes attached to photos
- Voice-to-text for field techs (2.6) and voice-to-invoice (5.1)

## Surface to build

| Type | Purpose |
|---|---|
| `DeepgramClient` | API key and project |
| `TranscriptionRequest` | Audio URL or bytes, model, language |
| `Transcription` | Transcript, confidence, words |
| `Word` | Timing and confidence |
| `Utterance` | Speaker-separated segment |

Methods: transcribe a file, transcribe a URL, open a streaming session,
detect keywords, diarize speakers.

## Wirekit contract

- **Auth:** Bearer token; never log the key.
- **Errors:** map Deepgram's error envelope to `WirekitError::Api`; a 400
  usually means an unsupported audio format.
- **Retry:** 429 and 5xx; streaming reconnects need a resume strategy, not a
  blind retry.
- **Rate limits:** plan-dependent; token bucket; batch where possible.
- **Webhooks:** callback URLs are supported for batch jobs; verify the callback
  or use a signed URL.
- **Idempotency:** batch jobs can be keyed by media ID; do not transcribe the
  same recording twice.

## GIA integration

Inbound calls stream to Deepgram; the transcript feeds Agent 6 and the
conversation summary. Field voice notes are uploaded and transcribed
asynchronously; the job note is attached when ready. Voice-to-invoice uses the
same transcript with a structured extraction step.

## Test plan

- Fixture audio: clean, noisy, multi-speaker, voicemail.
- Formats: WAV, MP3, M4A, OGG/Opus.
- Streaming reconnect and partial transcript.
- Diarization and keyword detection.
- Cost guard: max minutes per workspace.

## Open questions

1. **Data retention.** Does Deepgram retain audio/transcripts? Confirm the
   zero-retention option and the contract terms.
2. **PII.** Call recordings may contain sensitive data; redact before storage
   and set retention.
3. **Language coverage.** Confirm the languages GIA supports at launch; do not
   promise Spanish unless tested.
4. **Cost.** Streaming minutes are a real COGS item; cap per workspace.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §22.
