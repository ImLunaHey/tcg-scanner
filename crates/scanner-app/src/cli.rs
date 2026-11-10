use crate::options::{ClientCommand, ClientOptions};
use scanner_types::error::Result;
use serde_json::{Value, json};

pub async fn execute(options: ClientOptions) -> Result<Value> {
    let token = scanner_platform::load_credential(&options.token_file)?;
    let client = scanner_client::Client::new(&options.endpoint, &token)?;
    let (method, params) = parameters(options.command)?;
    client.call(&method, params).await
}
fn parameters(command: ClientCommand) -> Result<(String, Value)> {
    let (method, params) = match command {
        ClientCommand::Call { method, params } => {
            return Ok((
                method,
                serde_json::from_str(&params).map_err(crate::configuration::config)?,
            ));
        }
        ClientCommand::Status => ("service.status", json!({})),
        ClientCommand::Devices => ("devices", json!({})),
        ClientCommand::Jobs { limit, offset } => {
            ("jobs.list", json!({"limit":limit,"offset":offset}))
        }
        ClientCommand::Job { id } => ("jobs.get", json!({"id":id})),
        ClientCommand::Cancel { id } => ("jobs.cancel", json!({"id":id})),
        ClientCommand::Profiles => ("profiles.list", json!({})),
        ClientCommand::Scan {
            request_id,
            device_id,
            dpi,
            duplex,
            format,
            prepare_cards,
            card_language,
        } => {
            let mut params = json!({"requestID":request_id,"dpi":dpi,"duplex":duplex,"format":format,"prepareCards":prepare_cards,"cardLanguage":card_language});
            if let Some(id) = device_id {
                params["deviceID"] = json!(id);
            }
            ("jobs.create", params)
        }
    };
    Ok((method.into(), params))
}
