# Test Report: meta-graph-provider-module (v1)

- **Feature:** meta-graph-provider-module
- **Version:** v1
- **Date:** 2026-11-01
- **Branch:** feature/meta-stripe-platform-core
- **Suite:** `cargo test -p wirekit --features meta --test meta-graph-provider-module`

## Summary

- **Total ACs:** 13
- **Tested ACs:** 13
- **Coverage:** 100%
- **Tests Passed:** 13/13

## Results

| AC | Test | Result |
|---|---|---|
| AC-001 | `ac_001_messenger_send_posts_the_page_scoped_message` | pass |
| AC-002 | `ac_002_a_media_send_carries_an_image_attachment` | pass |
| AC-003 | `ac_003_instagram_posts_to_the_instagram_shape` | pass |
| AC-004 | `ac_004_whatsapp_template_send_orders_its_parameters` | pass |
| AC-005 | `ac_005_whatsapp_text_send_uses_the_text_type` | pass |
| AC-006 | `ac_006_a_correct_signature_parses_the_event` | pass |
| AC-007 | `ac_007_a_wrong_signature_fails_closed` | pass |
| AC-008 | `ac_008_the_handshake_echoes_the_challenge` | pass |
| AC-009 | `ac_009_the_event_carries_its_dedupe_key_and_surface` | pass |
| AC-010 | `ac_010_a_graph_error_body_becomes_an_api_error_with_its_code` | pass |
| AC-011 | `ac_011_the_usage_header_is_surfaced` | pass |
| AC-012 | `ac_012_credentials_never_render` | pass |
| AC-013 | `ac_013_a_cleartext_base_url_is_rejected` | pass |

Run: `13 passed; 0 failed; 0 ignored`, exit code 0. Verified against a local
Graph mock, so no Meta app review was required (RISK-001 stays `[to be confirmed]`).

## Defects found

- **Fixed (AC-002):** Messenger and Instagram were sending
  `message.attachment.payload.link` — WhatsApp's key — instead of `payload.url`.
  `attachment_body` now emits `url`/`attachment_id`; `attachment_payload` keeps
  `link` for the WhatsApp media object.
- **Test defect (AC-013):** the transport rejects a cleartext base URL at
  `build()`, not at `builder()`; the test now asserts that stage.

## Withdrawn criteria

None.
