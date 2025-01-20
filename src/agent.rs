use ai_agent::conv;
use ai_agent::model::ModelManager;
use ai_agent::oa_client::new_oa_client;
use ai_agent::tools::new_ai_tools;
use rpc_router::resources_builder;
use tokio::task::JoinSet;

pub async fn respond_llm(message: String) -> Result<String, Box<dyn std::error::Error>> {
	let mut output: Vec<String> = vec![];
	let oa_client = new_oa_client()?;

	let mm = ModelManager::default();
	let ai_tools = new_ai_tools(Some(resources_builder![mm]))?;
	let message = message.clone();

	// -- User questions
	let formatted_question = format!(
		r#"
== Question from user: {message}
		"#
	);
	let questions: [String; 1] = [formatted_question];

	// -- Execute questions concurrently
	let mut join_set: JoinSet<(String, Result<String, ai_agent::Error>)> = JoinSet::new();

	for question in questions {
		let oa_client = oa_client.clone();
		let ai_tools = ai_tools.clone();
		join_set.spawn(async move {
			// Execute user question.
			let result = conv::send_user_msg(oa_client, ai_tools, &question).await;

			(question.to_string(), result)
		});
	}

	while let Some(join_result) = join_set.join_next().await {
		let (question, send_result) = join_result?;
		let response = send_result?;

		output.push(format!(
			r#"
== Task:
{question}

== AI Response:
{response}
		"#
		));
	}

	Ok(output.join("\n"))
}
