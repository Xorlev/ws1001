use failure::Error;
use futures::{pin_mut, StreamExt};
use std::time::Duration;
use ws1001::WeatherRecordStream;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let stream = WeatherRecordStream::new(Duration::from_secs(10))
        .await?
        .start();
    pin_mut!(stream);
    while let Some(record) = stream.next().await {
        println!("{:#?}", record);
    }

    Ok(())
}
