//! Post a message to a Slack channel.
//!
//! ```sh
//! cargo run -p wirekit --example slack --features slack
//! ```
//!
//! Requires `SLACK_BOT_TOKEN` and `SLACK_CHANNEL`.

use wirekit::slack::{SlackClient, SlackMessage};
use wirekit::WirekitClient;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), wirekit::WirekitError> {
    let (Ok(token), Ok(channel)) = (
        std::env::var("SLACK_BOT_TOKEN"),
        std::env::var("SLACK_CHANNEL"),
    ) else {
        eprintln!("set SLACK_BOT_TOKEN and SLACK_CHANNEL to post a message");
        return Ok(());
    };

    let client = WirekitClient::builder(wirekit::slack::BASE_URL)?.build()?;
    let slack = SlackClient::builder(client).bot_token(token).build()?;

    let message = SlackMessage::text(channel, "wirekit example: hello from a shared client");
    let sent = slack.post_message(&message).await?;

    println!("posted to {:?} at {:?}", sent.channel, sent.ts);
    Ok(())
}
