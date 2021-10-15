mod ctx;
mod gas_cost;
mod transaction_builder;

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::time::Instant;

use near_crypto::{KeyType, SecretKey};
use near_primitives::account::{AccessKey, AccessKeyPermission, FunctionCallPermission};
use near_primitives::contract::ContractCode;
use near_primitives::runtime::fees::RuntimeFeesConfig;
use near_primitives::transaction::{
    Action, AddKeyAction, CreateAccountAction, DeleteAccountAction, DeleteKeyAction,
    DeployContractAction, SignedTransaction, StakeAction, TransferAction,
};
use near_primitives::types::AccountId;
use near_primitives::version::PROTOCOL_VERSION;
use near_vm_logic::mocks::mock_external::MockedExternal;
use near_vm_logic::{ExtCosts, VMConfig};
use num_rational::Ratio;
use rand::Rng;

use crate::cost_table::format_gas;
use crate::testbed_runners::{end_count, start_count, Config};
use crate::v2::ctx::Ctx;
use crate::v2::gas_cost::GasCost;
use crate::v2::transaction_builder::TransactionBuilder;
use crate::vm_estimator::create_context;
use crate::{Cost, CostTable};

use self::ctx::TestBed;

static ALL_COSTS: &[(Cost, fn(&mut Ctx) -> GasCost)] = &[
    (Cost::ActionReceiptCreation, action_receipt_creation),
    (Cost::ActionSirReceiptCreation, action_sir_receipt_creation),
    (Cost::ActionTransfer, action_transfer),
    (Cost::ActionCreateAccount, action_create_account),
    (Cost::ActionDeleteAccount, action_delete_account),
    (Cost::ActionAddFullAccessKey, action_add_full_access_key),
    (Cost::ActionAddFunctionAccessKeyBase, action_add_function_access_key_base),
    (Cost::ActionAddFunctionAccessKeyPerByte, action_add_function_access_key_per_byte),
    (Cost::ActionDeleteKey, action_delete_key),
    (Cost::ActionStake, action_stake),
    (Cost::ActionDeployContractBase, action_deploy_contract_base),
    (Cost::ActionDeployContractPerByte, action_deploy_contract_per_byte),
    (Cost::ActionFunctionCallBase, action_function_call_base),
    (Cost::ActionFunctionCallPerByte, action_function_call_per_byte),
    (Cost::ActionFunctionCallBaseV2, action_function_call_base_v2),
    (Cost::ActionFunctionCallPerByteV2, action_function_call_per_byte_v2),
    (Cost::HostFunctionCall, host_function_call),
    (Cost::WasmInstruction, wasm_instruction),
    (Cost::DataReceiptCreationBase, data_receipt_creation_base),
    (Cost::DataReceiptCreationPerByte, data_receipt_creation_per_byte),
    (Cost::ReadMemoryBase, read_memory_base),
    (Cost::ReadMemoryByte, read_memory_byte),
    (Cost::WriteMemoryBase, write_memory_base),
    (Cost::WriteMemoryByte, write_memory_byte),
    (Cost::ReadRegisterBase, read_register_base),
    (Cost::ReadRegisterByte, read_register_byte),
    (Cost::WriteRegisterBase, write_register_base),
    (Cost::WriteRegisterByte, write_register_byte),
    (Cost::LogBase, log_base),
    (Cost::LogByte, log_byte),
    (Cost::Utf8DecodingBase, utf8_decoding_base),
    (Cost::Utf8DecodingByte, utf8_decoding_byte),
    (Cost::Utf16DecodingBase, utf16_decoding_base),
    (Cost::Utf16DecodingByte, utf16_decoding_byte),
    (Cost::Sha256Base, sha256_base),
    (Cost::Sha256Byte, sha256_byte),
    (Cost::Keccak256Base, keccak256_base),
    (Cost::Keccak256Byte, keccak256_byte),
    (Cost::Keccak512Base, keccak512_base),
    (Cost::Keccak512Byte, keccak512_byte),
    (Cost::Ripemd160Base, ripemd160_base),
    (Cost::Ripemd160Block, ripemd160_block),
    (Cost::EcrecoverBase, ecrecover_base),
    (Cost::AltBn128G1MultiexpBase, alt_bn128g1_multiexp_base),
    (Cost::AltBn128G1MultiexpByte, alt_bn128g1_multiexp_byte),
    (Cost::AltBn128G1MultiexpSublinear, alt_bn128g1_multiexp_sublinear),
    (Cost::AltBn128G1SumBase, alt_bn128g1_sum_base),
    (Cost::AltBn128G1SumByte, alt_bn128g1_sum_byte),
    (Cost::AltBn128PairingCheckBase, alt_bn128_pairing_check_base),
    (Cost::AltBn128PairingCheckByte, alt_bn128_pairing_check_byte),
    (Cost::StorageHasKeyBase, storage_has_key_base),
    (Cost::StorageHasKeyByte, storage_has_key_byte),
    (Cost::StorageReadBase, storage_read_base),
    (Cost::StorageReadKeyByte, storage_read_key_byte),
    (Cost::StorageReadValueByte, storage_read_value_byte),
    (Cost::StorageWriteBase, storage_write_base),
    (Cost::StorageWriteKeyByte, storage_write_key_byte),
    (Cost::StorageWriteValueByte, storage_write_value_byte),
    (Cost::StorageWriteEvictedByte, storage_write_evicted_byte),
    (Cost::StorageRemoveBase, storage_remove_base),
    (Cost::StorageRemoveKeyByte, storage_remove_key_byte),
    (Cost::StorageRemoveRetValueByte, storage_remove_ret_value_byte),
];

pub fn run(config: Config) -> CostTable {
    let mut ctx = Ctx::new(&config);
    let mut res = CostTable::default();

    for (cost, f) in ALL_COSTS.iter().copied() {
        let skip = match &ctx.config.metrics_to_measure {
            None => false,
            Some(costs) => !costs.contains(&format!("{:?}", cost)),
        };
        if skip {
            continue;
        }

        let start = Instant::now();
        let value = f(&mut ctx);
        let gas = value.to_gas();
        res.add(cost, gas);
        eprintln!(
            "{:<40} {:>25} gas  (computed in {:.2?})",
            cost.to_string(),
            format_gas(gas),
            start.elapsed()
        );
    }
    eprintln!();

    res
}

fn action_receipt_creation(ctx: &mut Ctx) -> GasCost {
    if let Some(cached) = ctx.cached.action_receipt_creation.clone() {
        return cached;
    }

    let test_bed = ctx.test_bed();

    let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
        let (sender, receiver) = tb.random_account_pair();

        tb.transaction_from_actions(sender, receiver, vec![])
    };
    let cost = transaction_cost(test_bed, &mut make_transaction);

    ctx.cached.action_receipt_creation = Some(cost.clone());
    cost
}

fn action_sir_receipt_creation(ctx: &mut Ctx) -> GasCost {
    if let Some(cached) = ctx.cached.action_sir_receipt_creation.clone() {
        return cached;
    }

    let test_bed = ctx.test_bed();

    let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
        let sender = tb.random_account();
        let receiver = sender.clone();

        tb.transaction_from_actions(sender, receiver, vec![])
    };
    let cost = transaction_cost(test_bed, &mut make_transaction);

    ctx.cached.action_sir_receipt_creation = Some(cost.clone());
    cost
}

fn action_transfer(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let (sender, receiver) = tb.random_account_pair();

            let actions = vec![Action::Transfer(TransferAction { deposit: 1 })];
            tb.transaction_from_actions(sender, receiver, actions)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_receipt_creation(ctx);

    total_cost - base_cost
}

fn action_create_account(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_account();
            let new_account =
                AccountId::try_from(format!("{}_{}", sender, tb.rng().gen::<u64>())).unwrap();

            let actions = vec![
                Action::CreateAccount(CreateAccountAction {}),
                Action::Transfer(TransferAction { deposit: 10u128.pow(26) }),
            ];
            tb.transaction_from_actions(sender, new_account, actions)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_receipt_creation(ctx);

    total_cost - base_cost
}

fn action_delete_account(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            let receiver = sender.clone();
            let beneficiary_id = tb.random_unused_account();

            let actions = vec![Action::DeleteAccount(DeleteAccountAction { beneficiary_id })];
            tb.transaction_from_actions(sender, receiver, actions)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_sir_receipt_creation(ctx);

    total_cost - base_cost
}

fn action_add_full_access_key(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();

            add_key_transaction(tb, sender, AccessKeyPermission::FullAccess)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_sir_receipt_creation(ctx);

    total_cost - base_cost
}

fn action_add_function_access_key_base(ctx: &mut Ctx) -> GasCost {
    if let Some(cost) = ctx.cached.action_add_function_access_key_base.clone() {
        return cost;
    }

    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            let receiver_id = tb.account(0).to_string();

            let permission = AccessKeyPermission::FunctionCall(FunctionCallPermission {
                allowance: Some(100),
                receiver_id,
                method_names: vec!["method1".to_string()],
            });
            add_key_transaction(tb, sender, permission)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_sir_receipt_creation(ctx);

    let cost = total_cost - base_cost;
    ctx.cached.action_add_function_access_key_base = Some(cost.clone());
    cost
}

fn action_add_function_access_key_per_byte(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let many_methods: Vec<_> = (0..1000).map(|i| format!("a123456{:03}", i)).collect();
        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            let receiver_id = tb.account(0).to_string();

            let permission = AccessKeyPermission::FunctionCall(FunctionCallPermission {
                allowance: Some(100),
                receiver_id,
                method_names: many_methods.clone(),
            });
            add_key_transaction(tb, sender, permission)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_add_function_access_key_base(ctx);

    // 1k methods for 10 bytes each
    let bytes_per_transaction = 10 * 1000;

    (total_cost - base_cost) / bytes_per_transaction
}

fn add_key_transaction(
    tb: &mut TransactionBuilder,
    sender: AccountId,
    permission: AccessKeyPermission,
) -> SignedTransaction {
    let receiver = sender.clone();

    let public_key = "ed25519:DcA2MzgpJbrUATQLLceocVckhhAqrkingax4oJ9kZ847".parse().unwrap();
    let access_key = AccessKey { nonce: 0, permission };

    tb.transaction_from_actions(
        sender,
        receiver,
        vec![Action::AddKey(AddKeyAction { public_key, access_key })],
    )
}

fn action_delete_key(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            let receiver = sender.clone();

            let actions = vec![Action::DeleteKey(DeleteKeyAction {
                public_key: SecretKey::from_seed(KeyType::ED25519, sender.as_ref()).public_key(),
            })];
            tb.transaction_from_actions(sender, receiver, actions)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_sir_receipt_creation(ctx);

    total_cost - base_cost
}

fn action_stake(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            let receiver = sender.clone();

            let actions = vec![Action::Stake(StakeAction {
                stake: 1,
                public_key: "22skMptHjFWNyuEWY22ftn2AbLPSYpmYwGJRGwpNHbTV".parse().unwrap(),
            })];
            tb.transaction_from_actions(sender, receiver, actions)
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = action_sir_receipt_creation(ctx);

    total_cost - base_cost
}

fn action_deploy_contract_base(ctx: &mut Ctx) -> GasCost {
    if let Some(cost) = ctx.cached.action_deploy_contract_base.clone() {
        return cost;
    }

    let total_cost = {
        let code = ctx.read_resource("test-contract/res/smallest_contract.wasm");
        deploy_contract_cost(ctx, code)
    };

    let base_cost = action_sir_receipt_creation(ctx);

    let cost = total_cost - base_cost;
    ctx.cached.action_deploy_contract_base = Some(cost.clone());
    cost
}
fn action_deploy_contract_per_byte(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let code = ctx.read_resource(if cfg!(feature = "nightly_protocol_features") {
            "test-contract/res/nightly_large_contract.wasm"
        } else {
            "test-contract/res/stable_large_contract.wasm"
        });
        deploy_contract_cost(ctx, code)
    };

    let base_cost = action_deploy_contract_base(ctx);

    let bytes_per_transaction = 1024 * 1024;

    (total_cost - base_cost) / bytes_per_transaction
}
fn deploy_contract_cost(ctx: &mut Ctx, code: Vec<u8>) -> GasCost {
    let test_bed = ctx.test_bed();

    let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
        let sender = tb.random_unused_account();
        let receiver = sender.clone();

        let actions = vec![Action::DeployContract(DeployContractAction { code: code.clone() })];
        tb.transaction_from_actions(sender, receiver, actions)
    };
    transaction_cost(test_bed, &mut make_transaction)
}

fn action_function_call_base(ctx: &mut Ctx) -> GasCost {
    let total_cost = noop_host_function_call_cost(ctx);
    let base_cost = action_sir_receipt_creation(ctx);

    total_cost - base_cost
}
fn action_function_call_per_byte(ctx: &mut Ctx) -> GasCost {
    let total_cost = {
        let test_bed = ctx.test_bed_with_contracts();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            tb.transaction_from_function_call(sender, "noop", vec![0; 1024 * 1024])
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    let base_cost = noop_host_function_call_cost(ctx);

    let bytes_per_transaction = 1024 * 1024;

    (total_cost - base_cost) / bytes_per_transaction
}

fn action_function_call_base_v2(ctx: &mut Ctx) -> GasCost {
    let (base, _per_byte) = action_function_call_base_per_byte_v2(ctx);
    base
}
fn action_function_call_per_byte_v2(ctx: &mut Ctx) -> GasCost {
    let (_base, per_byte) = action_function_call_base_per_byte_v2(ctx);
    per_byte
}
fn action_function_call_base_per_byte_v2(ctx: &mut Ctx) -> (GasCost, GasCost) {
    if let Some(base_byte_cost) = ctx.cached.action_function_call_base_per_byte_v2.clone() {
        return base_byte_cost;
    }

    let (base, byte) =
        crate::function_call::test_function_call(ctx.config.metric, ctx.config.vm_kind);
    let convert_ratio = |r: Ratio<i128>| -> Ratio<u64> {
        Ratio::new((*r.numer()).try_into().unwrap(), (*r.denom()).try_into().unwrap())
    };
    let base_byte_cost = (
        GasCost { value: convert_ratio(base), metric: ctx.config.metric },
        GasCost { value: convert_ratio(byte), metric: ctx.config.metric },
    );

    ctx.cached.action_function_call_base_per_byte_v2 = Some(base_byte_cost.clone());
    base_byte_cost
}

fn data_receipt_creation_base(ctx: &mut Ctx) -> GasCost {
    // NB: there isn't `ExtCosts` for data receipt creation, so we ignore (`_`) the counts.
    let (total_cost, _) = fn_cost_count(ctx, "data_receipt_10b_1000", ExtCosts::base);
    let (base_cost, _) = fn_cost_count(ctx, "data_receipt_base_10b_1000", ExtCosts::base);
    (total_cost - base_cost) / 1000
}

fn data_receipt_creation_per_byte(ctx: &mut Ctx) -> GasCost {
    // NB: there isn't `ExtCosts` for data receipt creation, so we ignore (`_`) the counts.
    let (total_cost, _) = fn_cost_count(ctx, "data_receipt_100kib_1000", ExtCosts::base);
    let (base_cost, _) = fn_cost_count(ctx, "data_receipt_10b_1000", ExtCosts::base);

    let bytes_per_transaction = 1000 * 100 * 1024;

    (total_cost - base_cost) / bytes_per_transaction
}

fn host_function_call(ctx: &mut Ctx) -> GasCost {
    let (total_cost, count) = fn_cost_count(ctx, "base_1M", ExtCosts::base);
    assert_eq!(count, 1_000_000);

    let base_cost = noop_host_function_call_cost(ctx);

    (total_cost - base_cost) / count
}

fn wasm_instruction(ctx: &mut Ctx) -> GasCost {
    let vm_kind = ctx.config.vm_kind;

    let code = ctx.read_resource(if cfg!(feature = "nightly_protocol_features") {
        "test-contract/res/nightly_large_contract.wasm"
    } else {
        "test-contract/res/stable_large_contract.wasm"
    });

    let n_iters = 10;

    let code = ContractCode::new(code.to_vec(), None);
    let mut fake_external = MockedExternal::new();
    let config = VMConfig::default();
    let fees = RuntimeFeesConfig::test();
    let promise_results = vec![];

    let mut run = || {
        let context = create_context(vec![]);
        let (outcome, err) = near_vm_runner::run_vm(
            &code,
            "cpu_ram_soak_test",
            &mut fake_external,
            context,
            &config,
            &fees,
            &promise_results,
            vm_kind,
            PROTOCOL_VERSION,
            None,
        );
        match (outcome, err) {
            (Some(it), Some(_)) => it,
            _ => panic!(),
        }
    };

    let warmup_outcome = run();

    let start = start_count(ctx.config.metric);
    for _ in 0..n_iters {
        run();
    }
    let total = end_count(ctx.config.metric, &start);
    let total = Ratio::from_integer(total);

    let instructions_per_iter = {
        let op_cost = config.regular_op_cost as u64;
        warmup_outcome.burnt_gas / op_cost
    };

    let per_instruction = total / (instructions_per_iter * n_iters);
    GasCost { value: per_instruction, metric: ctx.config.metric }
}

fn read_memory_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "read_memory_10b_10k", ExtCosts::read_memory_base, 10_000)
}
fn read_memory_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "read_memory_1Mib_10k", ExtCosts::read_memory_byte, 1024 * 1024 * 10_000)
}

fn write_memory_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "write_memory_10b_10k", ExtCosts::write_memory_base, 10_000)
}
fn write_memory_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "write_memory_1Mib_10k", ExtCosts::write_memory_byte, 1024 * 1024 * 10_000)
}

fn read_register_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "read_register_10b_10k", ExtCosts::read_register_base, 10_000)
}
fn read_register_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "read_register_1Mib_10k", ExtCosts::read_register_byte, 1024 * 1024 * 10_000)
}

fn write_register_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "write_register_10b_10k", ExtCosts::write_register_base, 10_000)
}
fn write_register_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "write_register_1Mib_10k", ExtCosts::write_register_byte, 1024 * 1024 * 10_000)
}

fn log_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "utf16_log_10b_10k", ExtCosts::log_base, 10_000)
}
fn log_byte(ctx: &mut Ctx) -> GasCost {
    // NOTE: We are paying per *output* byte here, hence 3/2 multiplier.
    fn_cost(ctx, "utf16_log_10kib_10k", ExtCosts::log_byte, (10 * 1024 * 3 / 2) * 10_000)
}

fn utf8_decoding_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "utf8_log_10b_10k", ExtCosts::utf8_decoding_base, 10_000)
}
fn utf8_decoding_byte(ctx: &mut Ctx) -> GasCost {
    let no_nul =
        fn_cost(ctx, "utf8_log_10kib_10k", ExtCosts::utf8_decoding_byte, 10 * 1024 * 10_000);
    let nul = fn_cost(
        ctx,
        "nul_utf8_log_10kib_10k",
        ExtCosts::utf8_decoding_byte,
        (10 * 1024 - 1) * 10_000,
    );
    nul.max(no_nul)
}

fn utf16_decoding_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "utf16_log_10b_10k", ExtCosts::utf16_decoding_base, 10_000)
}
fn utf16_decoding_byte(ctx: &mut Ctx) -> GasCost {
    let no_nul =
        fn_cost(ctx, "utf16_log_10kib_10k", ExtCosts::utf16_decoding_byte, 10 * 1024 * 10_000);
    let nul = fn_cost(
        ctx,
        "nul_utf16_log_10kib_10k",
        ExtCosts::utf16_decoding_byte,
        (10 * 1024 - 2) * 10_000,
    );
    nul.max(no_nul)
}

fn sha256_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "sha256_10b_10k", ExtCosts::sha256_base, 10_000)
}
fn sha256_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "sha256_10kib_10k", ExtCosts::sha256_byte, 10 * 1024 * 10_000)
}

fn keccak256_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "keccak256_10b_10k", ExtCosts::keccak256_base, 10_000)
}
fn keccak256_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "keccak256_10kib_10k", ExtCosts::keccak256_byte, 10 * 1024 * 10_000)
}

fn keccak512_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "keccak512_10b_10k", ExtCosts::keccak512_base, 10_000)
}
fn keccak512_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "keccak512_10kib_10k", ExtCosts::keccak512_byte, 10 * 1024 * 10_000)
}

fn ripemd160_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "ripemd160_10b_10k", ExtCosts::ripemd160_base, 10_000)
}
fn ripemd160_block(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "ripemd160_10kib_10k", ExtCosts::ripemd160_block, (10 * 1024 / 64 + 1) * 10_000)
}

fn ecrecover_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "ecrecover_10k", ExtCosts::ecrecover_base, 10_000)
}

fn alt_bn128g1_multiexp_base(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(ctx, "alt_bn128_g1_multiexp_1_1k", ExtCosts::alt_bn128_g1_multiexp_base, 1000);
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}
fn alt_bn128g1_multiexp_byte(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(
        ctx,
        "alt_bn128_g1_multiexp_10_1k",
        ExtCosts::alt_bn128_g1_multiexp_byte,
        964 * 1000,
    );
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}
fn alt_bn128g1_multiexp_sublinear(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(
        ctx,
        "alt_bn128_g1_multiexp_10_1k",
        ExtCosts::alt_bn128_g1_multiexp_sublinear,
        743342 * 1000,
    );
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}

fn alt_bn128g1_sum_base(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(ctx, "alt_bn128_g1_sum_1_1k", ExtCosts::alt_bn128_g1_sum_base, 1000);
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}
fn alt_bn128g1_sum_byte(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(ctx, "alt_bn128_g1_sum_10_1k", ExtCosts::alt_bn128_g1_sum_byte, 654 * 1000);
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}

fn alt_bn128_pairing_check_base(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(
        ctx,
        "alt_bn128_pairing_check_1_1k",
        ExtCosts::alt_bn128_pairing_check_base,
        1000,
    );
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}
fn alt_bn128_pairing_check_byte(ctx: &mut Ctx) -> GasCost {
    #[cfg(feature = "protocol_feature_alt_bn128")]
    return fn_cost(
        ctx,
        "alt_bn128_pairing_check_10_1k",
        ExtCosts::alt_bn128_pairing_check_byte,
        1924 * 1000,
    );
    #[cfg(not(feature = "protocol_feature_alt_bn128"))]
    return GasCost { value: 0.into(), metric: ctx.config.metric };
}

fn storage_has_key_base(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10b_key_10b_value_1k",
        "storage_has_key_10b_key_10b_value_1k",
        ExtCosts::storage_has_key_base,
        1000,
    )
}
fn storage_has_key_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10kib_key_10b_value_1k",
        "storage_has_key_10kib_key_10b_value_1k",
        ExtCosts::storage_has_key_byte,
        10 * 1024 * 1000,
    )
}

fn storage_read_base(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10b_key_10b_value_1k",
        "storage_read_10b_key_10b_value_1k",
        ExtCosts::storage_read_base,
        1000,
    )
}
fn storage_read_key_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10kib_key_10b_value_1k",
        "storage_read_10kib_key_10b_value_1k",
        ExtCosts::storage_read_key_byte,
        10 * 1024 * 1000,
    )
}
fn storage_read_value_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10b_key_10kib_value_1k",
        "storage_read_10b_key_10kib_value_1k",
        ExtCosts::storage_read_value_byte,
        10 * 1024 * 1000,
    )
}

fn storage_write_base(ctx: &mut Ctx) -> GasCost {
    fn_cost(ctx, "storage_write_10b_key_10b_value_1k", ExtCosts::storage_write_base, 1000)
}
fn storage_write_key_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(
        ctx,
        "storage_write_10kib_key_10b_value_1k",
        ExtCosts::storage_write_key_byte,
        10 * 1024 * 1000,
    )
}
fn storage_write_value_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost(
        ctx,
        "storage_write_10b_key_10kib_value_1k",
        ExtCosts::storage_write_value_byte,
        10 * 1024 * 1000,
    )
}
fn storage_write_evicted_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10b_key_10kib_value_1k",
        "storage_write_10b_key_10kib_value_1k",
        ExtCosts::storage_write_evicted_byte,
        10 * 1024 * 1000,
    )
}

fn storage_remove_base(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10b_key_10b_value_1k",
        "storage_remove_10b_key_10b_value_1k",
        ExtCosts::storage_remove_base,
        1000,
    )
}
fn storage_remove_key_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10kib_key_10b_value_1k",
        "storage_remove_10kib_key_10b_value_1k",
        ExtCosts::storage_remove_key_byte,
        10 * 1024 * 1000,
    )
}
fn storage_remove_ret_value_byte(ctx: &mut Ctx) -> GasCost {
    fn_cost_with_setup(
        ctx,
        "storage_write_10b_key_10kib_value_1k",
        "storage_remove_10b_key_10kib_value_1k",
        ExtCosts::storage_remove_ret_value_byte,
        10 * 1024 * 1000,
    )
}

// Helpers

fn transaction_cost(
    test_bed: TestBed,
    make_transaction: &mut dyn FnMut(&mut TransactionBuilder) -> SignedTransaction,
) -> GasCost {
    let block_size = 100;
    let (gas_cost, _ext_costs) = transaction_cost_ext(test_bed, block_size, make_transaction);
    gas_cost
}

fn transaction_cost_ext(
    mut test_bed: TestBed,
    block_size: usize,
    make_transaction: &mut dyn FnMut(&mut TransactionBuilder) -> SignedTransaction,
) -> (GasCost, HashMap<ExtCosts, u64>) {
    let blocks = {
        let n_blocks = test_bed.config.warmup_iters_per_block + test_bed.config.iter_per_block;
        let mut blocks = Vec::with_capacity(n_blocks);
        for _ in 0..n_blocks {
            let mut block = Vec::with_capacity(block_size);
            for _ in 0..block_size {
                let tx = make_transaction(test_bed.transaction_builder());
                block.push(tx)
            }
            blocks.push(block)
        }
        blocks
    };

    let measurements = test_bed.measure_blocks(blocks);
    let measurements =
        measurements.into_iter().skip(test_bed.config.warmup_iters_per_block).collect::<Vec<_>>();

    let mut total_ext_costs: HashMap<ExtCosts, u64> = HashMap::new();
    let mut total = GasCost { value: 0.into(), metric: test_bed.config.metric };
    let mut n = 0;
    for (gas_cost, ext_cost) in measurements {
        total += gas_cost;
        n += block_size as u64;
        for (c, v) in ext_cost {
            *total_ext_costs.entry(c).or_default() += v;
        }
    }

    for v in total_ext_costs.values_mut() {
        *v /= n;
    }

    let gas_cost = total / n;
    (gas_cost, total_ext_costs)
}

fn noop_host_function_call_cost(ctx: &mut Ctx) -> GasCost {
    if let Some(cost) = ctx.cached.noop_host_function_call_cost.clone() {
        return cost;
    }

    let cost = {
        let test_bed = ctx.test_bed_with_contracts();

        let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
            let sender = tb.random_unused_account();
            tb.transaction_from_function_call(sender, "noop", Vec::new())
        };
        transaction_cost(test_bed, &mut make_transaction)
    };

    ctx.cached.noop_host_function_call_cost = Some(cost.clone());
    cost
}

fn fn_cost(ctx: &mut Ctx, method: &str, ext_cost: ExtCosts, count: u64) -> GasCost {
    let (total_cost, measured_count) = fn_cost_count(ctx, method, ext_cost);
    assert_eq!(measured_count, count);

    let base_cost = noop_host_function_call_cost(ctx);

    (total_cost - base_cost) / count
}

fn fn_cost_count(ctx: &mut Ctx, method: &str, ext_cost: ExtCosts) -> (GasCost, u64) {
    let block_size = 2;
    let mut make_transaction = |tb: &mut TransactionBuilder| -> SignedTransaction {
        let sender = tb.random_unused_account();
        tb.transaction_from_function_call(sender, method, Vec::new())
    };
    let test_bed = ctx.test_bed_with_contracts();
    let (gas_cost, ext_costs) = transaction_cost_ext(test_bed, block_size, &mut make_transaction);
    let ext_cost = ext_costs[&ext_cost];
    (gas_cost, ext_cost)
}

fn fn_cost_with_setup(
    ctx: &mut Ctx,
    setup: &str,
    method: &str,
    ext_cost: ExtCosts,
    count: u64,
) -> GasCost {
    let (total_cost, measured_count) = {
        let block_size = 2usize;
        let n_blocks = ctx.config.warmup_iters_per_block + ctx.config.iter_per_block;

        let mut test_bed = ctx.test_bed_with_contracts();

        let blocks = {
            let mut blocks = Vec::with_capacity(2 * n_blocks);
            for _ in 0..n_blocks {
                let tb = test_bed.transaction_builder();
                let mut setup_block = Vec::new();
                let mut block = Vec::new();
                for _ in 0..block_size {
                    let sender = tb.random_unused_account();
                    let setup_tx =
                        tb.transaction_from_function_call(sender.clone(), setup, Vec::new());
                    let tx = tb.transaction_from_function_call(sender, method, Vec::new());

                    setup_block.push(setup_tx);
                    block.push(tx);
                }
                blocks.push(setup_block);
                blocks.push(block);
            }
            blocks
        };

        let measurements = test_bed.measure_blocks(blocks);
        // Filter out setup blocks.
        let measurements: Vec<_> = measurements
            .into_iter()
            .skip(ctx.config.warmup_iters_per_block * 2)
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, m)| m)
            .collect();

        let mut total_ext_costs: HashMap<ExtCosts, u64> = HashMap::new();
        let mut total = GasCost { value: 0.into(), metric: ctx.config.metric };
        let mut n = 0;
        for (gas_cost, ext_cost) in measurements.into_iter().skip(ctx.config.warmup_iters_per_block)
        {
            total += gas_cost;
            n += block_size as u64;
            for (c, v) in ext_cost {
                *total_ext_costs.entry(c).or_default() += v;
            }
        }

        for v in total_ext_costs.values_mut() {
            *v /= n;
        }

        let gas_cost = total / n;
        (gas_cost, total_ext_costs[&ext_cost])
    };
    assert_eq!(measured_count, count);

    let base_cost = noop_host_function_call_cost(ctx);

    (total_cost - base_cost) / count
}

#[test]
fn smoke() {
    use genesis_populate::GenesisBuilder;
    use near_store::create_store;
    use nearcore::{get_store_path, load_config};
    use std::sync::Arc;

    use crate::testbed_runners::GasMetric;

    let temp_dir = tempfile::tempdir().unwrap();

    let state_dump_path = temp_dir.path().to_path_buf();
    nearcore::init_configs(
        &state_dump_path,
        None,
        Some("test.near".parse().unwrap()),
        Some("alice.near"),
        1,
        true,
        None,
        false,
        None,
        false,
        None,
        None,
        None,
    );

    let near_config = load_config(&state_dump_path);
    let store = create_store(&get_store_path(&state_dump_path));
    GenesisBuilder::from_config_and_store(&state_dump_path, Arc::new(near_config.genesis), store)
        .add_additional_accounts(5_000)
        .add_additional_accounts_contract(near_test_contracts::tiny_contract().to_vec())
        .print_progress()
        .build()
        .unwrap()
        .dump_state()
        .unwrap();

    let metrics = ["StorageRemoveBase", "StorageRemoveKeyByte", "StorageRemoveRetValueByte"];
    let config = Config {
        warmup_iters_per_block: 1,
        iter_per_block: 2,
        active_accounts: 5_000,
        block_sizes: vec![100],
        state_dump_path,
        metric: GasMetric::Time,
        vm_kind: near_vm_runner::VMKind::Wasmer0,
        metrics_to_measure: Some(metrics.iter().map(|it| it.to_string()).collect::<Vec<_>>())
            .filter(|it| !it.is_empty()),
    };
    let _table = run(config);
}
