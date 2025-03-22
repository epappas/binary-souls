from google.cloud import bigquery
import os


def run_query(query, verbose=1):
    os.environ["GOOGLE_APPLICATION_CREDENTIALS"] = {CREDENTIALS}
    client = bigquery.Client()
    try:
        # Run the query
        # print('running query')
        query_job = client.query(query)
        # print('query finished')
        data = query_job.to_dataframe()
        bytes_processed = query_job.total_bytes_processed
        gigabytes_processed = bytes_processed / 1_073_741_824
        if verbose > 0:
            print(f"Equivalent in gigabytes: {gigabytes_processed:.2f} GB")
        total_GB_process.append(gigabytes_processed)
        return data
    except Exception as e:
        print(f"An error occurred: {e}")  # Print the error message
        raise  # Re-raise the exception after printing the error


def chain_meta(index, in_usd=False):
    chains = ["ethereum", "cardano", "polygon"]
    schema = ["ethereum", "prod_cardano", "polygon"]
    network_table = ["erc20_network", "gold_transfer_network", "erc20_network"]
    ledger_table = ["erc20_ledger", "gold_transfer_ledger", "erc20_ledger"]
    query = """ SELECT asset_id, chain_id, name, symbol from crypto_data.token_whitelist where is_model = false """
    asset_ids = run_query(query, verbose=0)
    if in_usd:
        quote = ["USD", "USD", "USD"]
    else:
        quote = ["ETH", "ADA", "MATIC"]

    return {
        "chain": chains[index],
        "schema": schema[index],
        "network": network_table[index],
        "ledger": ledger_table[index],
        "quote": quote[index],
        "asset_ids": sorted(
            [
                int(x)
                for x in asset_ids[
                    asset_ids["chain_id"] == chains[index]
                ].asset_id.values
            ]
        ),
    }


def fetch_all_meta(num_of_chains=3, in_usd=False):
    all_meta = {}
    for i in range(0, num_of_chains):
        data = chain_meta(i, in_usd)
        all_meta[data["chain"]] = data
    return all_meta


def all_time_dominace(meta_data):
    query = """ WITH data AS (SELECT count(*) as tx_count, asset_id, partition_date
              FROM {schema}.{network}
              group by asset_id, partition_date)
SELECT tx_count/ sum(tx_count) over (partition by partition_date) as param, partition_date, asset_id from data """.format(
        schema=meta_data["schema"], network=meta_data["network"]
    )
    return run_query(query, verbose=0)


from ml_runtime import MLRuntime, ModelConfig

config = ModelConfig(
    max_memory=1024 * 1024 * 1024,  # 1GB
    max_concurrent_requests=10,
    inference_timeout_ms=1000,
)


def main():
    meta_datas = fetch_all_meta()
    # for chain, meta_data in meta_datas.items():
    #     data = all_time_dominace(meta_data)
    #     data["chain"] = chain
    #     data["meta"] = [{"param_name": "Transaction Dominance"}] * len(data)
    #     data.to_csv(f'{chain}_dominance.csv', index = False)

    with MLRuntime(config) as runtime:
        subscore = runtime.register_subscore("dominance")

        runtime.store_data("custom_key", b"custom data", encrypt=True)

        for chain, meta_data in meta_datas.items():
            data = all_time_dominace(meta_data)
            data["chain"] = chain
            data["meta"] = [{"param_name": "Transaction Dominance"}] * len(data)

            runtime.update_score(
                subscore,
                partition_time=data["partition_date"],
                identifier=data["asset_id"],
                value=data["tx_count"],
                meta=data["meta"],
            )




if __name__ == "__main__":
    # is very quick and easy, runs for the entire history each time beacuase it was easiest

    # Initialization
    total_GB_process = []
    main()
