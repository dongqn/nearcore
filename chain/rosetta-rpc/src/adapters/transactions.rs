use std::collections::HashMap;
use std::string::ToString;

use actix::Addr;

use near_primitives::hash::CryptoHash;
use near_primitives::views::SignedTransactionView;

/// A mapping from NEAR transaction or receipt hash to list of receipts.
/// and a mapping from transaction hashes to transactions.
/// The latter map is needed to determine the amount of deposit in a single transaction when
/// converting blocks to Rosetta transactions.
pub(crate) struct ExecutionToReceipts {
    map: HashMap<CryptoHash, Vec<CryptoHash>>,
    transactions: HashMap<CryptoHash, SignedTransactionView>,
}

impl ExecutionToReceipts {
    /// Fetches execution outcomes for given block and constructs a mapping from
    /// transaction or receipt causing the execution to list of created
    /// receipts’ hashes.
    pub(crate) async fn for_block(
        view_client_addr: Addr<near_client::ViewClientActor>,
        block_hash: CryptoHash,
    ) -> crate::errors::Result<Self> {
        let block = view_client_addr
            .send(near_client::GetBlock(near_primitives::types::BlockId::Hash(block_hash).into()))
            .await?
            .map_err(|e| crate::errors::ErrorKind::InternalError(e.to_string()))?;
        let mut transactions = HashMap::new();
        for (shard_id, contained) in block.header.chunk_mask.iter().enumerate() {
            if *contained {
                let chunk = view_client_addr
                    .send(near_client::GetChunk::ChunkHash(near_primitives::sharding::ChunkHash(
                        block.chunks[shard_id].chunk_hash,
                    )))
                    .await?
                    .map_err(|e| crate::errors::ErrorKind::InternalInvariantError(e.to_string()))?;
                transactions.extend(chunk.transactions.into_iter().map(|t| (t.hash, t)));
            }
        }
        let map = view_client_addr
            .send(near_client::GetExecutionOutcomesForBlock { block_hash })
            .await?
            .map_err(crate::errors::ErrorKind::InternalInvariantError)?
            .into_values()
            .flat_map(|outcomes| outcomes)
            .filter(|exec| !exec.outcome.receipt_ids.is_empty())
            .map(|exec| (exec.id, exec.outcome.receipt_ids))
            .collect();
        Ok(Self { map, transactions })
    }

    /// Creates an empty mapping.  This is useful for tests.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self { map: Default::default(), transactions: Default::default() }
    }

    /// Returns list of related transactions for given NEAR transaction or
    /// receipt.
    fn get_related(&self, exec_hash: CryptoHash) -> Vec<crate::models::RelatedTransaction> {
        self.map
            .get(&exec_hash)
            .map(|hashes| {
                hashes
                    .iter()
                    .map(crate::models::TransactionIdentifier::receipt)
                    .map(crate::models::RelatedTransaction::forward)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Constructs a Rosetta transaction hash for a change with a given cause.
///
/// If the change happened due to a transaction or a receipt, returns hash of
/// that transaction as well.  If the change was due to another reason, the
/// second returned value will be None.
///
/// Transactions are significant because we need to generate list of receipts as
/// related transactions so that Rosetta API clients can match transactions and
/// receipts together.
///
/// Returns error if unexpected cause was encountered.
fn convert_cause_to_transaction_id(
    block_hash: &CryptoHash,
    cause: &near_primitives::views::StateChangeCauseView,
) -> crate::errors::Result<(crate::models::TransactionIdentifier, Option<CryptoHash>)> {
    use crate::models::TransactionIdentifier;
    use near_primitives::views::StateChangeCauseView;

    match cause {
        StateChangeCauseView::TransactionProcessing { tx_hash } => {
            Ok((TransactionIdentifier::transaction(&tx_hash), Some(*tx_hash)))
        }
        StateChangeCauseView::ActionReceiptProcessingStarted { receipt_hash }
        | StateChangeCauseView::ActionReceiptGasReward { receipt_hash }
        | StateChangeCauseView::ReceiptProcessing { receipt_hash }
        | StateChangeCauseView::PostponedReceipt { receipt_hash } => {
            Ok((TransactionIdentifier::receipt(&receipt_hash), Some(*receipt_hash)))
        }
        StateChangeCauseView::InitialState => {
            Ok((TransactionIdentifier::block_event("block", block_hash), None))
        }
        StateChangeCauseView::ValidatorAccountsUpdate => {
            Ok((TransactionIdentifier::block_event("block-validators-update", block_hash), None))
        }
        StateChangeCauseView::UpdatedDelayedReceipts => {
            Ok((TransactionIdentifier::block_event("block-delayed-receipts", block_hash), None))
        }
        StateChangeCauseView::NotWritableToDisk => {
            Err(crate::errors::ErrorKind::InternalInvariantError(
                "State Change 'NotWritableToDisk' should never be observed".to_string(),
            ))
        }
        StateChangeCauseView::Migration => {
            Ok((TransactionIdentifier::block_event("migration", block_hash), None))
        }
        StateChangeCauseView::Resharding => Err(crate::errors::ErrorKind::InternalInvariantError(
            "State Change 'Resharding' should never be observed".to_string(),
        )),
    }
}

type RosettaTransactionsMap = std::collections::HashMap<String, crate::models::Transaction>;

pub(crate) struct RosettaTransactions<'a> {
    exec_to_rx: ExecutionToReceipts,
    block_hash: &'a CryptoHash,
    map: RosettaTransactionsMap,
}

impl<'a> RosettaTransactions<'a> {
    fn new(exec_to_rx: ExecutionToReceipts, block_hash: &'a CryptoHash) -> Self {
        Self { exec_to_rx, block_hash, map: Default::default() }
    }

    /// Returns a Rosetta transaction object for given state change cause.
    ///
    /// `transaction_identifier`, `related_transactions` and `metadata` of the
    /// object will be populated but initially the `operations` will be an empty
    /// vector.  It’s caller’s responsibility to fill it out as required.
    fn get_for_cause(
        &mut self,
        cause: &near_primitives::views::StateChangeCauseView,
    ) -> crate::errors::Result<&mut crate::models::Transaction> {
        let (id, exec_hash) = convert_cause_to_transaction_id(&self.block_hash, cause)?;
        let tx = self.map.entry(id.hash).or_insert_with_key(|hash| {
            let related_transactions = exec_hash
                .map(|exec_hash| self.exec_to_rx.get_related(exec_hash))
                .unwrap_or_default();
            crate::models::Transaction {
                transaction_identifier: crate::models::TransactionIdentifier { hash: hash.clone() },
                operations: Vec::new(),
                related_transactions,
                metadata: crate::models::TransactionMetadata {
                    type_: crate::models::TransactionType::Transaction,
                },
            }
        });
        Ok(tx)
    }
}

/// Returns Rosetta transactions which map to given account changes.
pub(crate) fn convert_block_changes_to_transactions(
    runtime_config: &near_primitives::runtime::config::RuntimeConfig,
    block_hash: &CryptoHash,
    accounts_changes: near_primitives::views::StateChangesView,
    mut accounts_previous_state: std::collections::HashMap<
        near_primitives::types::AccountId,
        near_primitives::views::AccountView,
    >,
    exec_to_rx: ExecutionToReceipts,
) -> crate::errors::Result<RosettaTransactionsMap> {
    let mut transactions = RosettaTransactions::new(exec_to_rx, block_hash);
    for account_change in accounts_changes {
        let transactions_in_block = &transactions.exec_to_rx.transactions;
        match account_change.value {
            near_primitives::views::StateChangeValueView::AccountUpdate { account_id, account } => {
                // Calculate the total amount of deposit from transfer actions.
                // This is needed to separate transfers into a separate operation
                // to pass the rosetta cli check
                let deposit = match &account_change.cause {
                    near_primitives::views::StateChangeCauseView::TransactionProcessing {
                        tx_hash,
                    } => transactions_in_block.get(tx_hash).and_then(|t| {
                        let total_sum = t
                            .actions
                            .iter()
                            .map(|action| match action {
                                near_primitives::views::ActionView::Transfer { deposit } => {
                                    *deposit
                                }
                                _ => 0,
                            })
                            .sum::<u128>();
                        if total_sum == 0 {
                            None
                        } else {
                            Some(total_sum)
                        }
                    }),
                    _ => None,
                };
                let previous_account_state = accounts_previous_state.get(&account_id);
                convert_account_update_to_operations(
                    runtime_config,
                    &mut transactions.get_for_cause(&account_change.cause)?.operations,
                    &account_id,
                    previous_account_state,
                    &account,
                    deposit,
                );
                accounts_previous_state.insert(account_id, account);
            }
            near_primitives::views::StateChangeValueView::AccountDeletion { account_id } => {
                let previous_account_state = accounts_previous_state.remove(&account_id);
                convert_account_delete_to_operations(
                    runtime_config,
                    &mut transactions.get_for_cause(&account_change.cause)?.operations,
                    &account_id,
                    previous_account_state,
                );
            }
            unexpected_value => {
                return Err(crate::errors::ErrorKind::InternalInvariantError(format!(
                    "queried AccountChanges, but received {:?}.",
                    unexpected_value
                )))
            }
        }
    }

    Ok(transactions.map)
}

fn convert_account_update_to_operations(
    runtime_config: &near_primitives::runtime::config::RuntimeConfig,
    operations: &mut Vec<crate::models::Operation>,
    account_id: &near_primitives::types::AccountId,
    previous_account_state: Option<&near_primitives::views::AccountView>,
    account: &near_primitives::views::AccountView,
    deposit: Option<near_primitives::types::Balance>,
) {
    let previous_account_balances = previous_account_state
        .map(|account| crate::utils::RosettaAccountBalances::from_account(account, runtime_config))
        .unwrap_or_else(crate::utils::RosettaAccountBalances::zero);

    let new_account_balances =
        crate::utils::RosettaAccountBalances::from_account(account, runtime_config);

    if previous_account_balances.liquid != new_account_balances.liquid {
        // Transfers would only lead to change in liquid balance, so it is sufficient to
        // have the check here only. If deposit is not `None` then we separate it into its own
        // operation to make Rosetta cli check happy.
        if let Some(deposit) = deposit {
            operations.push(crate::models::Operation {
                operation_identifier: crate::models::OperationIdentifier::new(operations),
                related_operations: None,
                account: crate::models::AccountIdentifier {
                    address: account_id.clone().into(),
                    sub_account: None,
                    metadata: None,
                },
                amount: Some(-crate::models::Amount::from_yoctonear(deposit)),
                type_: crate::models::OperationType::Transfer,
                status: Some(crate::models::OperationStatusKind::Success),
                metadata: None,
            });
            operations.push(crate::models::Operation {
                operation_identifier: crate::models::OperationIdentifier::new(operations),
                related_operations: None,
                account: crate::models::AccountIdentifier {
                    address: account_id.clone().into(),
                    sub_account: None,
                    metadata: None,
                },
                amount: Some(crate::models::Amount::from_yoctonear_diff(
                    crate::utils::SignedDiff::cmp(
                        // this operation is guaranteed to not underflow. Otherwise the transaction is invalid
                        previous_account_balances.liquid - deposit,
                        new_account_balances.liquid,
                    ),
                )),
                type_: crate::models::OperationType::Transfer,
                status: Some(crate::models::OperationStatusKind::Success),
                metadata: None,
            });
        } else {
            operations.push(crate::models::Operation {
                operation_identifier: crate::models::OperationIdentifier::new(operations),
                related_operations: None,
                account: crate::models::AccountIdentifier {
                    address: account_id.clone().into(),
                    sub_account: None,
                    metadata: None,
                },
                amount: Some(crate::models::Amount::from_yoctonear_diff(
                    crate::utils::SignedDiff::cmp(
                        previous_account_balances.liquid,
                        new_account_balances.liquid,
                    ),
                )),
                type_: crate::models::OperationType::Transfer,
                status: Some(crate::models::OperationStatusKind::Success),
                metadata: None,
            });
        }
    }

    if previous_account_balances.liquid_for_storage != new_account_balances.liquid_for_storage {
        operations.push(crate::models::Operation {
            operation_identifier: crate::models::OperationIdentifier::new(operations),
            related_operations: None,
            account: crate::models::AccountIdentifier {
                address: account_id.clone().into(),
                sub_account: Some(crate::models::SubAccount::LiquidBalanceForStorage.into()),
                metadata: None,
            },
            amount: Some(crate::models::Amount::from_yoctonear_diff(
                crate::utils::SignedDiff::cmp(
                    previous_account_balances.liquid_for_storage,
                    new_account_balances.liquid_for_storage,
                ),
            )),
            type_: crate::models::OperationType::Transfer,
            status: Some(crate::models::OperationStatusKind::Success),
            metadata: None,
        });
    }

    if previous_account_balances.locked != new_account_balances.locked {
        operations.push(crate::models::Operation {
            operation_identifier: crate::models::OperationIdentifier::new(operations),
            related_operations: None,
            account: crate::models::AccountIdentifier {
                address: account_id.clone().into(),
                sub_account: Some(crate::models::SubAccount::Locked.into()),
                metadata: None,
            },
            amount: Some(crate::models::Amount::from_yoctonear_diff(
                crate::utils::SignedDiff::cmp(
                    previous_account_balances.locked,
                    new_account_balances.locked,
                ),
            )),
            type_: crate::models::OperationType::Transfer,
            status: Some(crate::models::OperationStatusKind::Success),
            metadata: None,
        });
    }
}

fn convert_account_delete_to_operations(
    runtime_config: &near_primitives::runtime::config::RuntimeConfig,
    operations: &mut Vec<crate::models::Operation>,
    account_id: &near_primitives::types::AccountId,
    previous_account_state: Option<near_primitives::views::AccountView>,
) {
    let previous_account_balances = if let Some(previous_account_state) = previous_account_state {
        crate::utils::RosettaAccountBalances::from_account(previous_account_state, runtime_config)
    } else {
        return;
    };
    let new_account_balances = crate::utils::RosettaAccountBalances::zero();

    if previous_account_balances.liquid != new_account_balances.liquid {
        operations.push(crate::models::Operation {
            operation_identifier: crate::models::OperationIdentifier::new(operations),
            related_operations: None,
            account: crate::models::AccountIdentifier {
                address: account_id.clone().into(),
                sub_account: None,
                metadata: None,
            },
            amount: Some(crate::models::Amount::from_yoctonear_diff(
                crate::utils::SignedDiff::cmp(
                    previous_account_balances.liquid,
                    new_account_balances.liquid,
                ),
            )),
            type_: crate::models::OperationType::Transfer,
            status: Some(crate::models::OperationStatusKind::Success),
            metadata: None,
        });
    }

    if previous_account_balances.liquid_for_storage != new_account_balances.liquid_for_storage {
        operations.push(crate::models::Operation {
            operation_identifier: crate::models::OperationIdentifier::new(operations),
            related_operations: None,
            account: crate::models::AccountIdentifier {
                address: account_id.clone().into(),
                sub_account: Some(crate::models::SubAccount::LiquidBalanceForStorage.into()),
                metadata: None,
            },
            amount: Some(crate::models::Amount::from_yoctonear_diff(
                crate::utils::SignedDiff::cmp(
                    previous_account_balances.liquid_for_storage,
                    new_account_balances.liquid_for_storage,
                ),
            )),
            type_: crate::models::OperationType::Transfer,
            status: Some(crate::models::OperationStatusKind::Success),
            metadata: None,
        });
    }

    if previous_account_balances.locked != new_account_balances.locked {
        operations.push(crate::models::Operation {
            operation_identifier: crate::models::OperationIdentifier::new(operations),
            related_operations: None,
            account: crate::models::AccountIdentifier {
                address: account_id.clone().into(),
                sub_account: Some(crate::models::SubAccount::Locked.into()),
                metadata: None,
            },
            amount: Some(crate::models::Amount::from_yoctonear_diff(
                crate::utils::SignedDiff::cmp(
                    previous_account_balances.locked,
                    new_account_balances.locked,
                ),
            )),
            type_: crate::models::OperationType::Transfer,
            status: Some(crate::models::OperationStatusKind::Success),
            metadata: None,
        });
    }
}
