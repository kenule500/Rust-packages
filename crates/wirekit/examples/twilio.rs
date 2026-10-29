//! Send an SMS through Twilio.
//!
//! ```sh
//! cargo run -p wirekit --example twilio --features twilio
//! ```
//!
//! Requires `TWILIO_ACCOUNT_SID`, `TWILIO_AUTH_TOKEN`, `TWILIO_TO`, and
//! `TWILIO_FROM`. Nothing is sent when they are missing.

use wirekit::twilio::{SendSmsRequest, TwilioClient};
use wirekit::WirekitClient;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), wirekit::WirekitError> {
    let (Ok(account_sid), Ok(auth_token), Ok(to), Ok(from)) = (
        std::env::var("TWILIO_ACCOUNT_SID"),
        std::env::var("TWILIO_AUTH_TOKEN"),
        std::env::var("TWILIO_TO"),
        std::env::var("TWILIO_FROM"),
    ) else {
        eprintln!(
            "set TWILIO_ACCOUNT_SID, TWILIO_AUTH_TOKEN, TWILIO_TO, and TWILIO_FROM to send a message"
        );
        return Ok(());
    };

    let client = WirekitClient::builder(wirekit::twilio::BASE_URL)?.build()?;
    let twilio = TwilioClient::builder(client)
        .account_sid(account_sid)
        .auth_token(auth_token)
        .build()?;

    let request = SendSmsRequest::new(to, from, "wirekit example: hello from a shared client");
    let message = twilio.send_sms(&request).await?;

    println!("{} {}", message.sid, message.status);
    Ok(())
}
