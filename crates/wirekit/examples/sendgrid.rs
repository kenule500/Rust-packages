//! Send a transactional email through SendGrid in sandbox mode.
//!
//! ```sh
//! cargo run -p wirekit --example sendgrid --features sendgrid
//! ```
//!
//! Requires `SENDGRID_API_KEY`, `SENDGRID_FROM`, and `SENDGRID_TO`. Sandbox
//! mode validates the request without delivering mail.

use wirekit::sendgrid::{EmailAddress, SendEmailRequest, SendGridClient, SendGridConfig};
use wirekit::WirekitClient;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), wirekit::WirekitError> {
    let (Ok(api_key), Ok(from), Ok(to)) = (
        std::env::var("SENDGRID_API_KEY"),
        std::env::var("SENDGRID_FROM"),
        std::env::var("SENDGRID_TO"),
    ) else {
        eprintln!("set SENDGRID_API_KEY, SENDGRID_FROM, and SENDGRID_TO to send mail");
        return Ok(());
    };

    let client = WirekitClient::builder(wirekit::sendgrid::BASE_URL)?.build()?;
    let config = SendGridConfig::new(api_key, EmailAddress::new(from)).sandbox(true);
    let sendgrid = SendGridClient::new(client, config)?;

    let request = SendEmailRequest::new(EmailAddress::new(to))
        .subject("wirekit example")
        .text("This message was validated in sandbox mode.")
        .html("<p>This message was validated in <strong>sandbox</strong> mode.</p>");

    let sent = sendgrid.send_email(&request).await?;
    println!("HTTP {} message id {:?}", sent.status, sent.message_id);
    Ok(())
}
