use std::error::Error;

mod application;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = application::Application::new()?;

    if let Err(e) = app.run().await {
        tracing::error!("Error running application: {}", e);
        return Err(e);
    }

    Ok(())
}
