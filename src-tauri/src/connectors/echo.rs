use super::{BotConnector, DispatchOutcome, Outbound, TokenTx};

pub struct EchoConnector;

impl EchoConnector {
    pub async fn stream(who: &str, text: &str, tokens: &TokenTx) -> String {
        let reply = format!("Echo mode is on, so this stays on this machine. {who} heard: {text}");
        for word in reply.split_inclusive(' ') {
            tokens.push(word);
            tokio::time::sleep(std::time::Duration::from_millis(28)).await;
        }
        reply
    }
}

impl BotConnector for EchoConnector {
    fn kind(&self) -> &'static str {
        "echo"
    }

    async fn dispatch(&self, req: Outbound, tokens: &TokenTx) -> Result<DispatchOutcome, String> {
        let reply = Self::stream(&req.bot_name, &req.text, tokens).await;
        Ok(DispatchOutcome::Reply(reply))
    }
}
