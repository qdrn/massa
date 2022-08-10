// Copyright (c) 2022 MASSA LABS <info@massa.net>

use crate::repl::Output;
use anyhow::{anyhow, bail, Result};
use console::style;
use massa_models::api::{
    AddressInfo, CompactAddressInfo, DatastoreEntryInput, EventFilter, OperationInput,
};
use massa_models::api::{ReadOnlyBytecodeExecution, ReadOnlyCall};
use massa_models::node::NodeId;
use massa_models::prehash::Map;
use massa_models::timeslots::get_current_latest_block_slot;
use massa_models::{
    Address, Amount, BlockId, EndorsementId, Operation, OperationId, OperationType, Slot,
};
use massa_sdk::Client;
use massa_signature::KeyPair;
use massa_time::MassaTime;
use massa_wallet::{Wallet, WalletError};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fmt::{Debug, Display};
use std::net::IpAddr;
use std::path::PathBuf;
use std::process;
use strum::{EnumMessage, EnumProperty, IntoEnumIterator};
use strum_macros::{Display, EnumIter, EnumMessage, EnumProperty, EnumString};

/// All the client commands
/// the order they are defined is the order they are displayed in so be careful
/// Maybe it would be worth renaming some of them for consistency
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, EnumIter, EnumMessage, EnumString, EnumProperty, Display)]
pub enum Command {
    #[strum(ascii_case_insensitive, message = "exit the client gracefully")]
    exit,

    #[strum(ascii_case_insensitive, message = "display this help")]
    help,

    #[strum(
        ascii_case_insensitive,
        props(args = "IpAddr1 IpAddr2 ..."),
        message = "unban given IP address(es)"
    )]
    node_unban_by_ip,

    #[strum(
        ascii_case_insensitive,
        props(args = "Id1 Id2 ..."),
        message = "unban given id(s)"
    )]
    node_unban_by_id,

    #[strum(
        ascii_case_insensitive,
        props(args = "IpAddr1 IpAddr2 ..."),
        message = "ban given IP address(es)"
    )]
    node_ban_by_ip,

    #[strum(
        ascii_case_insensitive,
        props(args = "Id1 Id2 ..."),
        message = "ban given id(s)"
    )]
    node_ban_by_id,

    #[strum(ascii_case_insensitive, message = "stops the node")]
    node_stop,

    #[strum(ascii_case_insensitive, message = "show staking addresses")]
    node_get_staking_addresses,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address1 Address2 ..."),
        message = "remove staking addresses"
    )]
    node_remove_staking_addresses,

    #[strum(
        ascii_case_insensitive,
        props(args = "SecretKey1 SecretKey2 ..."),
        message = "add staking secret keys"
    )]
    node_add_staking_secret_keys,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address discord_id"),
        message = "generate the testnet rewards program node/staker ownership proof"
    )]
    node_testnet_rewards_program_ownership_proof,

    #[strum(
        ascii_case_insensitive,
        props(args = "[IpAddr]"),
        message = "whitelist given IP addresses"
    )]
    node_whitelist,

    #[strum(
        ascii_case_insensitive,
        props(args = "[IpAddr]"),
        message = "remove from whitelist given IP addresses"
    )]
    node_remove_from_whitelist,

    #[strum(
        ascii_case_insensitive,
        message = "show the status of the node (reachable? number of peers connected, consensus, version, config parameter summary...)"
    )]
    get_status,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address1 Address2 ..."),
        message = "get info about a list of addresses (balances, block creation, ...)"
    )]
    get_addresses,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address Key"),
        message = "get a datastore entry (key must be UTF-8)"
    )]
    get_datastore_entry,

    #[strum(
        ascii_case_insensitive,
        props(args = "BlockId"),
        message = "show info about a block (content, finality ...)"
    )]
    get_block,

    #[strum(
        ascii_case_insensitive,
        props(args = "EndorsementId1 EndorsementId2 ..."),
        message = "show info about a list of endorsements (content, finality ...)"
    )]
    get_endorsements,

    #[strum(
        ascii_case_insensitive,
        props(args = "OperationId1 OperationId2 ..."),
        message = "show info about a list of operations(content, finality ...) "
    )]
    get_operations,

    #[strum(
        ascii_case_insensitive,
        props(
            args = "start=Slot end=Slot emitter_address=Address caller_address=Address operation_id=OperationId"
        ),
        message = "show events emitted by smart contracts with various filters"
    )]
    get_filtered_sc_output_event,

    #[strum(
        ascii_case_insensitive,
        message = "show wallet info (keys, addresses, balances ...)"
    )]
    wallet_info,

    #[strum(
        ascii_case_insensitive,
        message = "generate a secret key and add it into the wallet"
    )]
    wallet_generate_secret_key,

    #[strum(
        ascii_case_insensitive,
        props(args = "SecretKey1 SecretKey2 ..."),
        message = "add a list of secret keys to the wallet"
    )]
    wallet_add_secret_keys,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address1 Address2 ..."),
        message = "remove a list of addresses from the wallet"
    )]
    wallet_remove_addresses,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address string"),
        message = "sign provided string with given address (address must be in the wallet)"
    )]
    wallet_sign,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address RollCount Fee"),
        message = "buy rolls with wallet address"
    )]
    buy_rolls,

    #[strum(
        ascii_case_insensitive,
        props(args = "Address RollCount Fee"),
        message = "sell rolls with wallet address"
    )]
    sell_rolls,

    #[strum(
        ascii_case_insensitive,
        props(args = "SenderAddress ReceiverAddress Amount Fee"),
        message = "send coins from a wallet address"
    )]
    send_transaction,

    #[strum(
        ascii_case_insensitive,
        props(args = "SenderAddress PathToBytecode MaxGas GasPrice Coins Fee",),
        message = "create and send an operation containing byte code"
    )]
    send_smart_contract,

    #[strum(
        ascii_case_insensitive,
        props(
            args = "SenderAddress TargetAddress FunctionName Parameter MaxGas GasPrice Coins Fee",
        ),
        message = "create and send an operation to call a function of a smart contract"
    )]
    call_smart_contract,

    #[strum(
        ascii_case_insensitive,
        props(args = "PathToBytecode MaxGas GasPrice Address",),
        message = "execute byte code, address is optional. Nothing is really executed on chain"
    )]
    read_only_smart_contract,

    #[strum(
        ascii_case_insensitive,
        props(args = "TargetAddress TargetFunction Parameter MaxGas GasPrice SenderAddress",),
        message = "call a smart contract function, sender address is optional. Nothing is really executed on chain"
    )]
    read_only_call,

    #[strum(
        ascii_case_insensitive,
        message = "show time remaining to end of current episode"
    )]
    when_episode_ends,

    #[strum(ascii_case_insensitive, message = "tells you when moon")]
    when_moon,
}

/// Display the help of all commands
pub(crate) fn help() {
    println!("HELP of Massa client (list of available commands):");
    Command::iter().map(|c| c.help()).collect()
}

/// bail a shinny RPC error
macro_rules! rpc_error {
    ($e:expr) => {
        bail!("check if your node is running: {}", $e)
    };
}

/// print a yellow warning
macro_rules! client_warning {
    ($e:expr) => {
        println!("{}: {}", style("WARNING").yellow(), $e)
    };
}

/// Used to have a shinny json output
/// TODO re-factor me
#[derive(Debug, Serialize)]
struct ExtendedWalletEntry {
    /// the keypair
    pub keypair: KeyPair,
    /// address and balance information
    pub address_info: CompactAddressInfo,
}

impl Display for ExtendedWalletEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Secret key: {}", self.keypair)?;
        writeln!(f, "Public key: {}", self.keypair.get_public_key())?;
        writeln!(f, "{}", self.address_info)?;
        writeln!(f, "\n=====\n")?;
        Ok(())
    }
}

/// Aggregation of the local, with some useful information as the balance, etc
/// to be printed by the client.
#[derive(Debug, Serialize)]
pub struct ExtendedWallet(Map<Address, ExtendedWalletEntry>);

impl ExtendedWallet {
    /// Reorganize everything into an extended wallet
    fn new(wallet: &Wallet, addresses_info: &[AddressInfo]) -> Result<Self> {
        Ok(ExtendedWallet(
            addresses_info
                .iter()
                .map(|x| {
                    let keypair = wallet
                        .keys
                        .get(&x.address)
                        .ok_or_else(|| anyhow!("missing key"))?;
                    Ok((
                        x.address,
                        ExtendedWalletEntry {
                            keypair: keypair.clone(),
                            address_info: x.compact(),
                        },
                    ))
                })
                .collect::<Result<_>>()?,
        ))
    }
}

impl Display for ExtendedWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            client_warning!("your wallet does not contain any key, use 'wallet_generate_secret_key' to generate a new key and add it to your wallet");
        }

        for entry in self.0.values() {
            writeln!(f, "{}", entry)?;
        }
        Ok(())
    }
}

impl Command {
    /// Display the help of the command
    /// with fancy colors and so on
    pub(crate) fn help(&self) {
        println!(
            "- {} {}: {}{}",
            style(self.to_string()).green(),
            if self.get_str("args").is_some() {
                style(self.get_str("args").unwrap_or("")).yellow()
            } else {
                style("no args").color256(8).italic() // grey
            },
            if self.get_str("todo").is_some() {
                style(self.get_str("todo").unwrap_or("[not yet implemented] ")).red()
            } else {
                style("")
            },
            self.get_message().unwrap()
        )
    }

    /// run a given command
    ///
    /// # parameters
    /// - client: the RPC client
    /// - wallet: an access to the wallet
    /// - parameters: the parsed parameters
    /// - json: true if --json was passed as an option
    ///     it means that we don't want to print anything we just want the json output
    pub(crate) async fn run(
        &self,
        client: &Client,
        wallet: &mut Wallet,
        parameters: &[String],
        json: bool,
    ) -> Result<Box<dyn Output>> {
        match self {
            Command::exit => process::exit(0),

            Command::help => {
                if !json {
                    if !parameters.is_empty() {
                        if let Ok(c) = parameters[0].parse::<Command>() {
                            c.help();
                        } else {
                            println!(
                                "Command not found!\ntype \"help\" to get the list of commands"
                            );
                            help();
                        }
                    } else {
                        help();
                    }
                }
                Ok(Box::new(()))
            }

            Command::node_unban_by_ip => {
                let ips = parse_vec::<IpAddr>(parameters)?;
                match client.private.node_unban_by_ip(ips).await {
                    Ok(()) => {
                        if !json {
                            println!("Request of unbanning successfully sent!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                };
                Ok(Box::new(()))
            }

            Command::node_unban_by_id => {
                let ids = parse_vec::<NodeId>(parameters)?;
                match client.private.node_unban_by_id(ids).await {
                    Ok(()) => {
                        if !json {
                            println!("Request of unbanning successfully sent!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                };
                Ok(Box::new(()))
            }

            Command::node_ban_by_ip => {
                let ips = parse_vec::<IpAddr>(parameters)?;
                match client.private.node_ban_by_ip(ips).await {
                    Ok(()) => {
                        if !json {
                            println!("Request of banning successfully sent!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                }
                Ok(Box::new(()))
            }

            Command::node_ban_by_id => {
                let ids = parse_vec::<NodeId>(parameters)?;
                match client.private.node_ban_by_id(ids).await {
                    Ok(()) => {
                        if !json {
                            println!("Request of banning successfully sent!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                }
                Ok(Box::new(()))
            }

            Command::node_stop => {
                match client.private.stop_node().await {
                    Ok(()) => {
                        if !json {
                            println!("Request of stopping the Node successfully sent")
                        }
                    }
                    Err(e) => rpc_error!(e),
                };
                Ok(Box::new(()))
            }

            Command::node_get_staking_addresses => {
                match client.private.get_staking_addresses().await {
                    Ok(staking_addresses) => Ok(Box::new(staking_addresses)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::node_remove_staking_addresses => {
                let addresses = parse_vec::<Address>(parameters)?;
                match client.private.remove_staking_addresses(addresses).await {
                    Ok(()) => {
                        if !json {
                            println!("Addresses successfully removed!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                }
                Ok(Box::new(()))
            }

            Command::node_add_staking_secret_keys => {
                let keypairs = parse_vec::<KeyPair>(parameters)?;
                match client.private.add_staking_secret_keys(keypairs).await {
                    Ok(()) => {
                        if !json {
                            println!("Keys successfully added!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                };
                Ok(Box::new(()))
            }

            Command::node_testnet_rewards_program_ownership_proof => {
                if parameters.len() != 2 {
                    bail!("wrong number of parameters");
                }
                // parse
                let addr = parameters[0].parse::<Address>()?;
                let msg = parameters[1].as_bytes().to_vec();
                // get address signature
                if let Some(addr_sig) = wallet.sign_message(addr, msg.clone()) {
                    // get node signature
                    match client.private.node_sign_message(msg).await {
                        // print concatenation
                        Ok(node_sig) => {
                            if !json {
                                println!("Enter the following in discord:");
                            }
                            Ok(Box::new(format!(
                                "{}/{}/{}/{}",
                                node_sig.public_key,
                                node_sig.signature,
                                addr_sig.public_key,
                                addr_sig.signature
                            )))
                        }
                        Err(e) => rpc_error!(e),
                    }
                } else {
                    bail!("address not found")
                }
            }

            Command::get_status => match client.public.get_status().await {
                Ok(node_status) => Ok(Box::new(node_status)),
                Err(e) => rpc_error!(e),
            },

            Command::get_addresses => {
                let addresses = parse_vec::<Address>(parameters)?;
                match client.public.get_addresses(addresses).await {
                    Ok(addresses_info) => Ok(Box::new(addresses_info)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::get_datastore_entry => {
                if parameters.len() != 2 {
                    bail!("invalid number of parameters");
                }
                let address = parameters[0].parse::<Address>()?;
                let key = parameters[1].as_bytes().to_vec();
                match client
                    .public
                    .get_datastore_entries(vec![DatastoreEntryInput { address, key }])
                    .await
                {
                    Ok(result) => Ok(Box::new(result)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::get_block => {
                if parameters.len() != 1 {
                    bail!("wrong param numbers")
                }
                let block_id = parameters[0].parse::<BlockId>()?;
                match client.public.get_block(block_id).await {
                    Ok(block_info) => Ok(Box::new(block_info)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::get_endorsements => {
                let endorsements = parse_vec::<EndorsementId>(parameters)?;
                match client.public.get_endorsements(endorsements).await {
                    Ok(endorsements_info) => Ok(Box::new(endorsements_info)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::get_operations => {
                let operations = parse_vec::<OperationId>(parameters)?;
                match client.public.get_operations(operations).await {
                    Ok(operations_info) => Ok(Box::new(operations_info)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::get_filtered_sc_output_event => {
                let p_list: [&str; 5] = [
                    "start",
                    "end",
                    "emitter_address",
                    "caller_address",
                    "operation_id",
                ];
                let mut p: HashMap<&str, &str> = HashMap::new();
                for v in parameters {
                    let s: Vec<&str> = v.split('=').collect();
                    if s.len() == 2 && p_list.contains(&s[0]) {
                        p.insert(s[0], s[1]);
                    } else {
                        bail!("invalid parameter");
                    }
                }
                let filter = EventFilter {
                    start: parse_value(&p, p_list[0]),
                    end: parse_value(&p, p_list[1]),
                    emitter_address: parse_value(&p, p_list[2]),
                    original_caller_address: parse_value(&p, p_list[3]),
                    original_operation_id: parse_value(&p, p_list[4]),
                };
                match client.public.get_filtered_sc_output_event(filter).await {
                    Ok(events) => Ok(Box::new(events)),
                    Err(e) => rpc_error!(e),
                }
            }

            Command::wallet_info => {
                if !json {
                    client_warning!("do not share your key");
                }
                match client
                    .public
                    .get_addresses(wallet.get_full_wallet().keys().copied().collect())
                    .await
                {
                    Ok(addresses_info) => {
                        Ok(Box::new(ExtendedWallet::new(wallet, &addresses_info)?))
                    }
                    Err(_) => Ok(Box::new(wallet.clone())), // FIXME
                }
            }

            Command::wallet_generate_secret_key => {
                let key = KeyPair::generate();
                let ad = wallet.add_keypair(key.clone())?;
                if json {
                    Ok(Box::new(ad.to_string()))
                } else {
                    println!("Generated {} address and added it to the wallet", ad);
                    println!(
                        "Type `node_add_staking_secret_keys {}` to start staking with this key.\n",
                        key
                    );
                    Ok(Box::new(()))
                }
            }

            Command::wallet_add_secret_keys => {
                let addresses = parse_vec::<KeyPair>(parameters)?
                    .into_iter()
                    .map(|key| Ok((wallet.add_keypair(key.clone())?, key)))
                    .collect::<Result<HashMap<Address, KeyPair>>>()?;
                if json {
                    return Ok(Box::new(addresses.into_keys().collect::<Vec<Address>>()));
                } else {
                    for (address, key) in addresses.iter() {
                        println!("Derived and added address {} to the wallet.", address);
                        println!(
                            "Type `node_add_staking_secret_keys {}` to start staking with this key.\n",
                            key
                        );
                    }
                }
                Ok(Box::new(()))
            }

            Command::wallet_remove_addresses => {
                let mut res = "".to_string();
                for key in parse_vec::<Address>(parameters)?.into_iter() {
                    match wallet.remove_address(key) {
                        Ok(_) => {
                            let _ = writeln!(res, "Removed address {} from the wallet", key);
                        }
                        Err(WalletError::MissingKeyError(_)) => {
                            let _ = writeln!(res, "Address {} wasn't in the wallet", key);
                        }
                        Err(_) => {
                            let _ =
                                writeln!(res, "Failed to remove address {} from the wallet", key);
                        }
                    }
                }
                if !json {
                    println!("{}", res);
                }
                Ok(Box::new(()))
            }

            Command::buy_rolls => {
                if parameters.len() != 3 {
                    bail!("wrong number of parameters");
                }
                let addr = parameters[0].parse::<Address>()?;
                let roll_count = parameters[1].parse::<u64>()?;
                let fee = parameters[2].parse::<Amount>()?;

                if !json {
                    let roll_price = match client.public.get_status().await {
                        Err(e) => bail!("RpcError: {}", e),
                        Ok(status) => status.config.roll_price,
                    };
                    match roll_price
                        .checked_mul_u64(roll_count)
                        .and_then(|x| x.checked_add(fee))
                    {
                        Some(total) => {
                            if let Ok(addresses_info) =
                                client.public.get_addresses(vec![addr]).await
                            {
                                match addresses_info.get(0) {
                                    Some(info) => {
                                        if info.ledger_info.candidate_ledger_info.balance < total {
                                            client_warning!("this operation may be rejected due to insufficient balance");
                                        }
                                    }
                                    None => {
                                        client_warning!(format!("address {} not found", addr))
                                    }
                                }
                            }
                        }
                        None => {
                            client_warning!("the total amount hit the limit overflow, operation will certainly be rejected");
                        }
                    }
                    if let Ok(staked_keys) = client.private.get_staking_addresses().await {
                        if !staked_keys.contains(&addr) {
                            client_warning!("You are buying rolls with an address not registered for staking. Don't forget to run 'node_add_staking_secret_keys <your_secret_key'");
                        }
                    }
                }
                send_operation(
                    client,
                    wallet,
                    OperationType::RollBuy { roll_count },
                    fee,
                    addr,
                    json,
                )
                .await
            }

            Command::sell_rolls => {
                if parameters.len() != 3 {
                    bail!("wrong number of parameters");
                }
                let addr = parameters[0].parse::<Address>()?;
                let roll_count = parameters[1].parse::<u64>()?;
                let fee = parameters[2].parse::<Amount>()?;

                if !json {
                    if let Ok(addresses_info) = client.public.get_addresses(vec![addr]).await {
                        match addresses_info.get(0) {
                            Some(info) => {
                                if info.ledger_info.candidate_ledger_info.balance < fee
                                    || roll_count > info.rolls.candidate_rolls
                                {
                                    client_warning!("this operation may be rejected due to insufficient balance or roll count");
                                }
                            }
                            None => client_warning!(format!("address {} not found", addr)),
                        }
                    }
                }

                send_operation(
                    client,
                    wallet,
                    OperationType::RollSell { roll_count },
                    fee,
                    addr,
                    json,
                )
                .await
            }

            Command::send_transaction => {
                if parameters.len() != 4 {
                    bail!("wrong number of parameters");
                }
                let addr = parameters[0].parse::<Address>()?;
                let recipient_address = parameters[1].parse::<Address>()?;
                let amount = parameters[2].parse::<Amount>()?;
                let fee = parameters[3].parse::<Amount>()?;

                if !json {
                    match amount.checked_add(fee) {
                        Some(total) => {
                            if let Ok(addresses_info) =
                                client.public.get_addresses(vec![addr]).await
                            {
                                match addresses_info.get(0) {
                                    Some(info) => {
                                        if info.ledger_info.candidate_ledger_info.balance < total {
                                            client_warning!("this operation may be rejected due to insufficient balance");
                                        }
                                    }
                                    None => {
                                        client_warning!(format!("address {} not found", addr))
                                    }
                                }
                            }
                        }
                        None => {
                            client_warning!("the total amount hit the limit overflow, operation will certainly be rejected");
                        }
                    }
                }

                send_operation(
                    client,
                    wallet,
                    OperationType::Transaction {
                        recipient_address,
                        amount,
                    },
                    fee,
                    addr,
                    json,
                )
                .await
            }
            Command::when_episode_ends => {
                let end = match client.public.get_status().await {
                    Ok(node_status) => node_status.config.end_timestamp,
                    Err(e) => bail!("RpcError: {}", e),
                };
                let mut res = "".to_string();
                if let Some(e) = end {
                    let (days, hours, mins, secs) =
                        e.saturating_sub(MassaTime::now()?).days_hours_mins_secs()?; // compensation milliseconds is zero

                    let _ = write!(res, "{} days, {} hours, {} minutes, {} seconds remaining until the end of the current episode", days, hours, mins, secs);
                } else {
                    let _ = write!(res, "There is no end !");
                }
                if !json {
                    println!("{}", res);
                }
                Ok(Box::new(()))
            }
            Command::when_moon => {
                let res = "At night 🌔.";
                if !json {
                    println!("{}", res);
                }
                Ok(Box::new(()))
            }
            Command::send_smart_contract => {
                if parameters.len() != 6 {
                    bail!("wrong number of parameters");
                }
                let addr = parameters[0].parse::<Address>()?;
                let path = parameters[1].parse::<PathBuf>()?;
                let max_gas = parameters[2].parse::<u64>()?;
                let gas_price = parameters[3].parse::<Amount>()?;
                let coins = parameters[4].parse::<Amount>()?;
                let fee = parameters[5].parse::<Amount>()?;

                if !json {
                    match gas_price
                        .checked_mul_u64(max_gas)
                        .and_then(|x| x.checked_add(coins))
                        .and_then(|x| x.checked_add(fee))
                    {
                        Some(total) => {
                            if let Ok(addresses_info) =
                                client.public.get_addresses(vec![addr]).await
                            {
                                match addresses_info.get(0) {
                                    Some(info) => {
                                        if info.ledger_info.candidate_ledger_info.balance < total {
                                            client_warning!("this operation may be rejected due to insufficient balance");
                                        }
                                    }
                                    None => {
                                        client_warning!(format!("address {} not found", addr));
                                    }
                                }
                            }
                        }
                        None => {
                            client_warning!("the total amount hit the limit overflow, operation will certainly be rejected");
                        }
                    }
                };
                let data = get_file_as_byte_vec(&path).await?;
                if !json {
                    let max_block_size = match client.public.get_status().await {
                        Ok(node_status) => node_status.config.max_block_size,
                        Err(e) => bail!("RpcError: {}", e),
                    };
                    if data.len() > max_block_size as usize / 2 {
                        client_warning!("bytecode size exceeded half of the maximum size of a block, operation will certainly be rejected");
                    }
                }

                send_operation(
                    client,
                    wallet,
                    OperationType::ExecuteSC {
                        data,
                        max_gas,
                        coins,
                        gas_price,
                    },
                    fee,
                    addr,
                    json,
                )
                .await
            }
            Command::call_smart_contract => {
                if parameters.len() != 8 {
                    bail!("wrong number of parameters");
                }
                let addr = parameters[0].parse::<Address>()?;
                let target_addr = parameters[1].parse::<Address>()?;
                let target_func = parameters[2].clone();
                let param = parameters[3].clone();
                let max_gas = parameters[4].parse::<u64>()?;
                let gas_price = parameters[5].parse::<Amount>()?;
                let coins = parameters[6].parse::<Amount>()?;
                let fee = parameters[7].parse::<Amount>()?;
                if !json {
                    match gas_price
                        .checked_mul_u64(max_gas)
                        .and_then(|x| x.checked_add(fee))
                    {
                        Some(total) => {
                            if let Ok(addresses_info) =
                                client.public.get_addresses(vec![target_addr]).await
                            {
                                match addresses_info.get(0) {
                                    Some(info) => {
                                        if info.ledger_info.candidate_ledger_info.balance < total
                                            || info.candidate_balance_info.unwrap_or_default()
                                                < coins
                                        {
                                            client_warning!("this operation may be rejected due to insufficient balance");
                                        }
                                    }
                                    None => {
                                        client_warning!(format!(
                                            "address {} not found",
                                            target_addr
                                        ));
                                    }
                                }
                            }
                        }
                        None => {
                            client_warning!("the total amount hit the limit overflow, operation will certainly be rejected");
                        }
                    }
                };
                send_operation(
                    client,
                    wallet,
                    OperationType::CallSC {
                        target_addr,
                        target_func,
                        param,
                        max_gas,
                        sequential_coins: Amount::zero(),
                        parallel_coins: coins,
                        gas_price,
                    },
                    fee,
                    addr,
                    json,
                )
                .await
            }
            Command::wallet_sign => {
                if parameters.len() != 2 {
                    bail!("wrong number of parameters");
                }
                let addr = parameters[0].parse::<Address>()?;
                let msg = parameters[1].clone();
                if let Some(signed) = wallet.sign_message(addr, msg.into_bytes()) {
                    Ok(Box::new(signed))
                } else {
                    bail!("Missing public key")
                }
            }
            Command::read_only_smart_contract => {
                if parameters.len() != 3 && parameters.len() != 4 {
                    bail!("wrong number of parameters");
                }

                let path = parameters[0].parse::<PathBuf>()?;
                let max_gas = parameters[1].parse::<u64>()?;
                let simulated_gas_price = parameters[2].parse::<Amount>()?;
                let address = if let Some(adr) = parameters.get(3) {
                    Some(adr.parse::<Address>()?)
                } else {
                    None
                };
                let bytecode = get_file_as_byte_vec(&path).await?;
                match client
                    .public
                    .execute_read_only_bytecode(ReadOnlyBytecodeExecution {
                        max_gas,
                        simulated_gas_price,
                        bytecode,
                        address,
                    })
                    .await
                {
                    Ok(res) => Ok(Box::new(res)),
                    Err(e) => rpc_error!(e),
                }
            }
            Command::read_only_call => {
                if parameters.len() != 5 && parameters.len() != 6 {
                    bail!("wrong number of parameters");
                }

                let target_address = parameters[0].parse::<Address>()?;
                let target_function = parameters[1].parse::<String>()?;
                let parameter = parameters[2].parse::<String>()?;
                let max_gas = parameters[3].parse::<u64>()?;
                let simulated_gas_price = parameters[4].parse::<Amount>()?;
                let caller_address = if let Some(addr) = parameters.get(5) {
                    Some(addr.parse::<Address>()?)
                } else {
                    None
                };
                match client
                    .public
                    .execute_read_only_call(ReadOnlyCall {
                        caller_address,
                        target_address,
                        target_function,
                        parameter,
                        max_gas,
                        simulated_gas_price,
                    })
                    .await
                {
                    Ok(res) => Ok(Box::new(res)),
                    Err(e) => rpc_error!(e),
                }
            }
            Command::node_whitelist => {
                let ips = parse_vec::<IpAddr>(parameters)?;
                match client.private.node_whitelist(ips).await {
                    Ok(()) => {
                        if !json {
                            println!("Request of whitelisting successfully sent!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                }
                Ok(Box::new(()))
            }
            Command::node_remove_from_whitelist => {
                let ips = parse_vec::<IpAddr>(parameters)?;
                match client.private.node_remove_from_whitelist(ips).await {
                    Ok(()) => {
                        if !json {
                            println!("Request of removing from whitelist successfully sent!")
                        }
                    }
                    Err(e) => rpc_error!(e),
                }
                Ok(Box::new(()))
            }
        }
    }
}

/// helper to wrap and send an operation with proper validity period
async fn send_operation(
    client: &Client,
    wallet: &Wallet,
    op: OperationType,
    fee: Amount,
    addr: Address,
    json: bool,
) -> Result<Box<dyn Output>> {
    let cfg = match client.public.get_status().await {
        Ok(node_status) => node_status,
        Err(e) => rpc_error!(e),
    }
    .config;

    let slot = get_current_latest_block_slot(cfg.thread_count, cfg.t0, cfg.genesis_timestamp, 0)? // clock compensation is zero
        .unwrap_or_else(|| Slot::new(0, 0));
    let mut expire_period = slot.period + cfg.operation_validity_periods;
    if slot.thread >= addr.get_thread(cfg.thread_count) {
        expire_period += 1;
    };

    let op = wallet.create_operation(
        Operation {
            fee,
            expire_period,
            op,
        },
        addr,
    )?;

    match client
        .public
        .send_operations(vec![OperationInput {
            creator_public_key: op.creator_public_key,
            serialized_content: op.serialized_data,
            signature: op.signature,
        }])
        .await
    {
        Ok(operation_ids) => {
            if !json {
                println!("Sent operation IDs:");
            }
            Ok(Box::new(operation_ids))
        }
        Err(e) => rpc_error!(e),
    }
}

/// TODO: ugly utilities functions
/// takes a slice of string and makes it into a `Vec<T>`
pub fn parse_vec<T: std::str::FromStr>(args: &[String]) -> anyhow::Result<Vec<T>, T::Err> {
    args.iter().map(|x| x.parse::<T>()).collect()
}

/// reads a file
async fn get_file_as_byte_vec(filename: &std::path::Path) -> Result<Vec<u8>> {
    Ok(tokio::fs::read(filename).await?)
}

// chains get_key_value with its parsing and displays a warning on parsing error
pub fn parse_value<T: std::str::FromStr>(p: &HashMap<&str, &str>, key: &str) -> Option<T> {
    p.get_key_value(key).and_then(|x| {
        x.1.parse::<T>()
            .map_err(|_| {
                client_warning!(format!(
                    "'{}' parameter was ignored because of wrong corresponding value",
                    key
                ))
            })
            .ok()
    })
}
