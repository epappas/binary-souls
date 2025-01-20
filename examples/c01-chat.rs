use ai_agent::oa_client::new_oa_client;
use ai_agent::{chat, gpts};
use async_openai::types::CreateChatCompletionRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// -- Init AI Client
	let oa_client = new_oa_client()?;
	let chat_client = oa_client.chat();
	let model = gpts::MODEL.to_string();

	// -- User question
	let question = "What it takes to be a good person?";

	// -- Build messages
	let messages = vec![chat::user_msg(question)?];

	// -- Exec Chat Request
	let msg_req = CreateChatCompletionRequest { model, messages, ..Default::default() };
	let chat_response = chat_client.create(msg_req).await?;
	let first_choice = chat::first_choice(chat_response)?;

	// -- Display response
	let response = first_choice.message.content.ok_or("No message content?")?;

	println!("\nResponse:\n\n{response}");

	Ok(())
}
