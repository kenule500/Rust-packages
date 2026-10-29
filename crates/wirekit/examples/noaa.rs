//! Fetch a NOAA point forecast.
//!
//! ```sh
//! cargo run -p wirekit --example noaa --features noaa
//! ```
//!
//! Set `NOAA_USER_AGENT` to identify your application; NOAA rejects requests
//! without a descriptive agent.

use wirekit::noaa::{ForecastRequest, NoaaClient};
use wirekit::WirekitClient;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), wirekit::WirekitError> {
    let user_agent = std::env::var("NOAA_USER_AGENT").unwrap_or_else(|_| {
        format!(
            "wirekit-example/{} (set NOAA_USER_AGENT to your app and contact)",
            wirekit::VERSION
        )
    });

    let client = WirekitClient::builder(wirekit::noaa::BASE_URL)?.build()?;
    let noaa = NoaaClient::builder(client).user_agent(user_agent).build();

    let request = ForecastRequest::new(40.7128, -74.0060)?;
    let forecast = noaa.forecast(&request).await?;

    println!(
        "{} periods, first temperature {:?}, precipitation {:?}",
        forecast.periods.len(),
        forecast.temperature,
        forecast.precipitation
    );

    if let Some(period) = forecast.periods.first() {
        println!(
            "{}: {}{} — {}",
            period.name,
            period.temperature.unwrap_or_default(),
            period.temperature_unit.as_deref().unwrap_or_default(),
            period.short_forecast.as_deref().unwrap_or("no summary")
        );
    }

    Ok(())
}
