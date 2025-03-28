from swarms import Agent
from swarms.prompts.finance_agent_sys_prompt import (
    FINANCIAL_AGENT_SYS_PROMPT,
)
from swarms_tools import (
    yahoo_finance_api,
)


def main():
    # Initialize the agent
    agent = Agent(
        agent_name="Financial-Analysis-Agent",
        agent_description="Personal finance advisor agent",
        system_prompt=FINANCIAL_AGENT_SYS_PROMPT,
        max_loops=1,
        model_name="gpt-4o",
        dynamic_temperature_enabled=True,
        user_name="swarms_corp",
        retry_attempts=3,
        context_length=8192,
        return_step_meta=False,
        output_type="str",  # "json", "dict", "csv" OR "string" "yaml" and
        auto_generate_prompt=False,  # Auto generate prompt for the agent based on name, description, and system prompt, task
        max_tokens=4000,  # max output tokens
        saved_state_path="agent_00.json",
        interactive=False,
        tools=[yahoo_finance_api],
    )

    # Run the agent with a specific query and print the result
    result = agent.run("Analyze the latest metrics for nvidia")
    print("\n=== Financial Analysis Result ===\n")
    print(result)


if __name__ == "__main__":
    main()
