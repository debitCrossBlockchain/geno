use crate::{
    errors::JsonRpcError,
    request::JsonRpcRequest,
    service::JsonRpcService,
    view::{
        account_view::{AccountInfoView, AccountView},
        ledger_view::LedgerView,
        transaction_view::{SubmitTx, TransactionResultView, TxHash},
    },
};
use anyhow::{ensure, format_err, Error, Result};
use core::future::Future;
use executor::LAST_COMMITTED_BLOCK_INFO_REF;
use futures::channel::oneshot;
use ledger_store::LedgerStorage;
use msp::signing::{self, check_address, check_private_key, check_tx_hash};
use serde_json::{Map, Value};
use state::READING_TRIE_REF;
use std::{
    cmp::min,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    pin::Pin,
    sync::Arc,
};
use tx_pool::types::TxPoolStatusCode;
use types::SignedTransaction;
use utils::{
    general::hash_crypto_byte,
    parse::ProtocolParser,
    verify_sign::{sign, verify_sign},
    TransactionSign,
};

type RpcHandler =
    Box<fn(JsonRpcService, JsonRpcRequest) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>>;

pub(crate) type RpcRegistry = HashMap<String, RpcHandler>;

async fn create_account(
    service: JsonRpcService,
    request: JsonRpcRequest,
) -> Result<Option<AccountView>> {
    let sign_type: String = request.parse_param(0, "sign_type")?;
    if !sign_type.eq("eddsa_ed25519") && !sign_type.eq(("secp256k1")) && !sign_type.eq(("sm2")) {
        return Err(Error::new(JsonRpcError::invalid_param(
            0,
            "sign_type",
            "eddsa_ed25519 or secp256k1 or sm2",
        )));
    }
    let priv_key = signing::create_secret_key(sign_type.as_str()).unwrap();
    let public_key = priv_key.get_pubkey();
    let private_key = priv_key.as_hex();
    let address = priv_key.get_address();
    return Ok(Some(AccountView {
        address,
        private_key,
        public_key,
        sign_type,
    }));
}

async fn get_accountbase(
    service: JsonRpcService,
    request: JsonRpcRequest,
) -> Result<Option<AccountInfoView>> {
    let account_address: String = request.parse_param(0, "address")?;
    if !check_address(&account_address) {
        return Err(Error::new(JsonRpcError::invalid_address(
            account_address.as_str(),
        )));
    }

    let state_hash = { LAST_COMMITTED_BLOCK_INFO_REF.read().get_state_hash() };
    let change = { READING_TRIE_REF.read().is_change(&state_hash) };
    let result = if change {
        READING_TRIE_REF
            .write()
            .get_mut(&state_hash, &account_address)
    } else {
        READING_TRIE_REF.read().get(&account_address)
    };

    match result {
        Ok(value) => match value {
            Some(account) => {
                return Ok(Some(AccountInfoView::new(account.account())));
            }
            None => {
                return Err(Error::new(JsonRpcError::data_not_found(format!(
                    "Account you are looking for does not exist"
                ))));
            }
        },
        Err(e) => {
            return Err(Error::new(JsonRpcError::internal_error(format!(
                "Trie error",
            ))))
        }
    }
}

async fn get_block_by_sequence(
    service: JsonRpcService,
    request: JsonRpcRequest,
) -> Result<Option<LedgerView>> {
    let _seq: u64 = request.parse_param(0, "sequence")?;
    let _last_seq = {
        LAST_COMMITTED_BLOCK_INFO_REF
            .read()
            .get_header()
            .get_height()
    };
    let ledger_seq = if _last_seq >= _seq { _seq } else { _last_seq };

    match LedgerStorage::load_ledger_header_by_seq(ledger_seq) {
        Ok(value) => match value {
            Some(header) => {
                return Ok(Some(LedgerView::new(header)));
            }
            None => {
                return Err(Error::new(JsonRpcError::data_not_found(
                    "Unable to get the block content of this height!".to_string(),
                )));
            }
        },
        Err(e) => {
            return Err(Error::new(JsonRpcError::internal_error(format!(
                "Db error",
            ))));
        }
    }
}

async fn get_block_by_hash(
    service: JsonRpcService,
    request: JsonRpcRequest,
) -> Result<Option<LedgerView>> {
    let hash: String = request.parse_param(0, "hash")?;

    match LedgerStorage::load_ledger_header_by_hash(&hash) {
        Ok(value) => match value {
            Some(header) => {
                return Ok(Some(LedgerView::new(header)));
            }
            None => {
                return Err(Error::new(JsonRpcError::data_not_found(
                    "Unable to get the block content of this height!".to_string(),
                )));
            }
        },
        Err(e) => {
            return Err(Error::new(JsonRpcError::internal_error(format!(
                "db error",
            ))));
        }
    }
}

async fn get_lastblock(
    service: JsonRpcService,
    request: JsonRpcRequest,
) -> Result<Option<LedgerView>> {
    let header = LAST_COMMITTED_BLOCK_INFO_REF.read().get_header().clone();
    return Ok(Some(LedgerView::new(header)));
}

async fn get_transaction(
    service: JsonRpcService,
    request: JsonRpcRequest,
) -> Result<TransactionResultView> {
    let hash_str: String = request.parse_param(0, "hash")?;

    if !check_tx_hash(hash_str.as_str()) {
        return Err(Error::new(JsonRpcError::invalid_parameter(
            "tx hash",
            "hash format error",
        )));
    }

    match LedgerStorage::load_tx(&hash_str) {
        Ok(value) => match value {
            Some(tx_store) => return Ok(TransactionResultView::from(&tx_store)),
            None => {
                return Err(Error::new(JsonRpcError::data_not_found(
                    "Unable to get the transaction!".to_string(),
                )));
            }
        },
        Err(e) => {
            return Err(Error::new(JsonRpcError::internal_error(format!(
                "db error",
            ))));
        }
    }
}

async fn send_transaction(service: JsonRpcService, request: JsonRpcRequest) -> Result<TxHash> {
    if request.params.len() != 1 {
        return Err(Error::new(JsonRpcError::invalid_params_size(
            "Currently only one transaction is supported for one upload!".to_string(),
        )));
    }

    let value = request.get_param(0);

    let submit_tx: SubmitTx = match serde_json::from_value(value) {
        Ok(t) => t,
        Err(err) => {
            return Err(Error::new(JsonRpcError::invalid_parameter(
                "sendTransaction",
                err.to_string().as_str(),
            )));
        }
    };

    if let Some(e) = submit_tx.transaction.check_parms() {
        return Err(e);
    }

    let transaction_sign = match submit_tx.signature {
        None => match submit_tx.private_key {
            None => {
                return Err(Error::new(JsonRpcError::invalid_parameter(
                    "sendTransaction",
                    "transaction miss signature or private key",
                )));
            }
            Some(private_key_str) => {
                if !check_private_key(private_key_str.priv_field.as_str()) {
                    return Err(Error::new(JsonRpcError::invalid_parameter(
                        "sendTransaction",
                        "invalid private key",
                    )));
                }

                let proto_tx = submit_tx.transaction.to_protocol()?;
                let tx_hash = hash_crypto_byte(&ProtocolParser::serialize::<
                    protos::ledger::Transaction,
                >(&proto_tx));

                let signature = match sign(
                    private_key_str.priv_field.as_str(),
                    &tx_hash,
                    &private_key_str.encryption_type,
                ) {
                    Ok(value) => value,
                    Err(e) => {
                        return Err(Error::new(JsonRpcError::invalid_parameter(
                            "sendTransaction",
                            "sign error",
                        )));
                    }
                };

                let mut transaction_sign = TransactionSign::default();
                transaction_sign.set_transaction(proto_tx);
                transaction_sign.set_signatures(protobuf::RepeatedField::from(vec![signature]));
                transaction_sign
                    .set_source_type(protos::ledger::TransactionSign_SourceType::JSONRPC);
                transaction_sign
            }
        },
        Some(sig_raw) => {
            let signature = match sig_raw.to_protocol() {
                Ok(sign) => sign,
                Err(e) => return Err(e),
            };

            let proto_tx = submit_tx.transaction.to_protocol()?;
            let tx_hash = hash_crypto_byte(
                &ProtocolParser::serialize::<protos::ledger::Transaction>(&proto_tx),
            );
            match verify_sign(&signature, &tx_hash) {
                Ok(value) => {
                    if !value {
                        return Err(Error::new(JsonRpcError::invalid_parameter(
                            "sendTransaction",
                            "verify sign error",
                        )));
                    }
                }
                Err(e) => {
                    return Err(Error::new(JsonRpcError::invalid_parameter(
                        "sendTransaction",
                        "verify sign error",
                    )));
                }
            }
            let mut transaction_sign = TransactionSign::default();
            transaction_sign.set_transaction(proto_tx);
            transaction_sign.set_signatures(protobuf::RepeatedField::from(vec![signature]));
            transaction_sign.set_source_type(protos::ledger::TransactionSign_SourceType::JSONRPC);
            transaction_sign
        }
    };
    let sign_transaction = SignedTransaction::try_from(transaction_sign)?;
    let tx_hash = sign_transaction.hash_hex();
    let (request_sender, callback) = oneshot::channel();
    service
        .jsonrpc_to_txpool_sender
        .unbounded_send((sign_transaction, request_sender))?;

    let (mempool_status, vm_status_opt) = callback.await??;
    if let Some(vm_status) = vm_status_opt {
        Err(Error::new(JsonRpcError::validation_status(vm_status)))
    } else if mempool_status.code == TxPoolStatusCode::Accepted {
        Ok(TxHash::new(tx_hash))
    } else {
        Err(Error::new(JsonRpcError::mempool_error(mempool_status)?))
    }
}

#[allow(unused_comparisons)]
pub(crate) fn build_registry() -> RpcRegistry {
    let mut registry = RpcRegistry::new();
    register_rpc_method!(registry, "createAccount", create_account, 1, 0);
    register_rpc_method!(registry, "sendTransaction", send_transaction, 1, 0);
    register_rpc_method!(registry, "getAccountBase", get_accountbase, 1, 0);
    register_rpc_method!(registry, "getBlockBySequence", get_block_by_sequence, 1, 0);
    register_rpc_method!(registry, "getBlockByHash", get_block_by_hash, 1, 0);
    register_rpc_method!(registry, "getLastBlock", get_lastblock, 0, 0);
    register_rpc_method!(registry, "getTransaction", get_transaction, 1, 0);
    registry
}
