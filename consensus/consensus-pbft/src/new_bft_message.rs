use crate::{
    bft_log::BftInstanceMap,
    bft_state::{BftInstancePhase, BftState},
    instance::{bft_instance_index::BftInstanceIndex, bft_vc_instance::BftVcInstance},
};
use fxhash::FxHashSet;
use itertools::Itertools;
use protobuf::RepeatedField;
use protos::{
    consensus::{
        Bft, BftBaseInfo, BftCommit, BftMessageType, BftNewView, BftPrePrepare, BftPrepare,
        BftPreparedSet, BftProof, BftSign, BftViewChange, BftViewChangeValue,
    },
    ledger::Ledger,
};
use tracing::{error, info};
use utils::{general::hash_crypto_byte, parse::ProtocolParser};

pub struct NewBftMessage {}

impl NewBftMessage {
    fn sign_bft_message(state: &BftState, bft: Bft) -> BftSign {
        let sig = state.sign_data(ProtocolParser::serialize::<Bft>(&bft).as_slice());
        let mut bft_sign = BftSign::new();
        bft_sign.set_signature(sig);
        bft_sign.set_bft(bft);
        bft_sign.set_chain_id(state.chain_id.clone());
        bft_sign.set_chain_hub(state.chain_hub.clone());

        return bft_sign;
    }

    pub fn new_base_info(view_number: i64, sequence: u64, replica_id: i64) -> BftBaseInfo {
        let mut bft_base_info = BftBaseInfo::new();
        bft_base_info.set_view_number(view_number);
        bft_base_info.set_sequence(sequence);
        bft_base_info.set_replica_id(replica_id);
        return bft_base_info;
    }

    pub fn new_pre_prepare(state: &BftState, value: &[u8], sequence: u64) -> BftSign {
        let mut pre_prepare = BftPrePrepare::new();
        pre_prepare.set_base(Self::new_base_info(
            state.view_number,
            sequence,
            state.replica_id,
        ));
        pre_prepare.set_value(Vec::from(value));
        pre_prepare.set_value_digest(hash_crypto_byte(value));

        let mut bft = Bft::new();
        bft.set_pre_prepare(pre_prepare);
        bft.set_round_number(1);
        bft.set_msg_type(BftMessageType::PRE_PREPARE);

        return Self::sign_bft_message(state, bft);
    }

    pub fn new_prepare(
        state: &BftState,
        pre_prepare: &BftPrePrepare,
        round_number: u64,
    ) -> BftSign {
        let mut prepare = BftPrepare::new();

        prepare.set_base(Self::new_base_info(
            pre_prepare.get_base().get_view_number(),
            pre_prepare.get_base().get_sequence(),
            state.replica_id,
        ));
        prepare.set_value_digest(Vec::from(pre_prepare.get_value_digest()));

        let mut bft = Bft::new();
        bft.set_prepare(prepare);
        bft.set_round_number(round_number);
        bft.set_msg_type(BftMessageType::PREPARE);

        return Self::sign_bft_message(state, bft);
    }

    pub fn new_commit(state: &BftState, prepare: &BftPrepare, round_number: u64) -> BftSign {
        let mut commit = BftCommit::new();
        commit.set_base(Self::new_base_info(
            prepare.get_base().get_view_number(),
            prepare.get_base().get_sequence(),
            state.replica_id,
        ));

        commit.set_value_digest(Vec::from(prepare.get_value_digest()));

        let mut bft = Bft::new();
        bft.set_commit(commit);
        bft.set_round_number(round_number);
        bft.set_msg_type(BftMessageType::COMMIT);

        return Self::sign_bft_message(state, bft);
    }

    pub fn new_view_change_raw_value(
        state: &BftState,
        view_number: i64,
        prepared_set: &BftPreparedSet,
        instances: &BftInstanceMap,
    ) -> BftSign {
        let mut p_view_change = BftViewChange::new();
        p_view_change.set_base(Self::new_base_info(
            view_number,
            state.last_exe_sequence,
            state.replica_id,
        ));

        let mut vc_raw = BftViewChangeValue::new();
        if prepared_set.has_pre_prepare() {
            vc_raw.set_prepared_set(prepared_set.clone());
            info!(parent:state.span(),
                "Got prepared value again, desc({})",
                Self::bft_desc(prepared_set.get_pre_prepare().get_bft())
            );
        } else {
            let mut prepared_set_ = BftPreparedSet::new();

            for index in instances
                .keys()
                .sorted_by(|&a, &b| BftInstanceIndex::cmp(a, b))
                .rev()
            {
                if let Some(instance) = instances.get(index) {
                    info!(parent:state.span(),
                        "get_prepared_set last_exe_seq:{} sequence:{} view_number:{} phase:{:?}",
                        p_view_change.get_base().get_sequence(),
                        index.sequence,
                        index.view_number,
                        instance.phase
                    );
                    if (index.sequence > p_view_change.get_base().get_sequence())
                        && ((instance.phase as i64) == (BftInstancePhase::PREPARED as i64))
                    {
                        //Add prepared message and pre-prepared message
                        if let Some(bft_sign) = instance.get_bft_sign(&BftInstancePhase::NONE, 0) {
                            prepared_set_.set_pre_prepare(bft_sign);
                        } else {
                            error!(parent:state.span(),"Can not find pre-prepared from msg buf");
                            break;
                        }

                        //Add prepared message
                        let vec = instance.get_bft_sign_vec(&BftInstancePhase::PRE_PREPARED);
                        if vec.len() > 0 {
                            prepared_set_.set_prepare(RepeatedField::from(vec));
                        }

                        vc_raw.set_prepared_set(prepared_set_);
                        info!(parent:state.span(),
                            "Got prepared value, desc({})",
                            Self::bft_desc(
                                vc_raw.get_prepared_set().get_pre_prepare().get_bft(),
                            ),
                        );
                        break;
                    }
                }
            }
        }

        //Add 'view change' value digest
        if vc_raw.has_prepared_set() {
            let pp_bft_env = vc_raw.get_prepared_set().get_pre_prepare();
            let pp_bft = pp_bft_env.get_bft().get_pre_prepare();
            p_view_change.set_prepared_value_digest(pp_bft.get_value_digest().to_vec());
        }

        //Add view change
        let mut bft_inner = Bft::new();
        bft_inner.set_view_change(p_view_change.clone());
        bft_inner.set_round_number(0);
        bft_inner.set_msg_type(BftMessageType::VIEW_CHANGE);

        //Add 'view change' signature
        let bft_env_inner = Self::sign_bft_message(state, bft_inner.clone());
        vc_raw.set_view_change_env(bft_env_inner.clone());

        let mut bft = Bft::new();
        bft.set_view_change_value(vc_raw.clone());
        bft.set_round_number(0);
        bft.set_msg_type(BftMessageType::VIEW_CHANGE_VALUE);

        //Add 'view change' raw value signature
        return Self::sign_bft_message(state, bft);
    }

    pub fn new_new_view(state: &BftState, vc_instance: &BftVcInstance) -> BftSign {
        let mut new_view = BftNewView::new();
        new_view.set_base(Self::new_base_info(
            vc_instance.view_number,
            vc_instance.sequence,
            state.replica_id,
        ));
        for it in vc_instance.msg_buf.iter() {
            new_view.mut_view_changes().push(
                it.get_bft()
                    .get_view_change_value()
                    .get_view_change_env()
                    .clone(),
            );
        }

        if vc_instance.pre_prepared_env_set.has_pre_prepare() {
            new_view.set_pre_prepare(vc_instance.pre_prepared_env_set.get_pre_prepare().clone());
        }

        let mut bft = Bft::new();
        bft.set_new_view(new_view);
        bft.set_round_number(0);
        bft.set_msg_type(BftMessageType::NEW_VIEW);

        return Self::sign_bft_message(state, bft);
    }

    pub fn inc_message_round(state: &BftState, bft_sign: &BftSign, round_number: u64) -> BftSign {
        let mut bft = Bft::new();
        bft.clone_from(bft_sign.get_bft());
        bft.set_round_number(round_number);
        return Self::sign_bft_message(state, bft);
    }

    pub(crate) fn get_commited_proof(bft_sign_vec: Vec<BftSign>) -> BftProof {
        let mut proof = BftProof::new();
        let mut commit_node = FxHashSet::default();
        for bft_sign in bft_sign_vec.iter() {
            let sign = bft_sign.get_signature();
            if !commit_node.contains(sign.get_public_key()) {
                proof.commits.push(bft_sign.clone());
                commit_node.insert(sign.get_public_key().to_vec());
            }
        }
        proof
    }

    // ========================for display information==========================================
    pub fn base_info_desc(base_info: &BftBaseInfo) -> String {
        format!(
            "view_number:({}) sequence:({}) replica:({})",
            base_info.get_view_number(),
            base_info.get_sequence(),
            base_info.get_replica_id()
        )
    }

    pub fn bft_desc(bft: &Bft) -> String {
        match bft.get_msg_type() {
            BftMessageType::PRE_PREPARE => Self::pre_prepare_desc(bft.get_pre_prepare()),
            BftMessageType::PREPARE => Self::prepare_desc(bft.get_prepare()),
            BftMessageType::COMMIT => Self::commit_desc(bft.get_commit()),
            BftMessageType::VIEW_CHANGE => Self::view_change_desc(bft.get_view_change()),
            BftMessageType::VIEW_CHANGE_VALUE => {
                Self::view_change_value_desc(bft.get_view_change_value())
            }
            BftMessageType::NEW_VIEW => Self::new_view_desc(bft.get_new_view()),
        }
    }

    pub fn pre_prepare_desc(pre_prepare: &BftPrePrepare) -> String {
        format!(
            "type:Pre-Prepare | {} | digest:({})",
            Self::base_info_desc(pre_prepare.get_base()),
            Self::consensus_value_desc(pre_prepare.get_value())
        )
    }

    pub fn prepare_desc(prepare: &BftPrepare) -> String {
        format!(
            "type:Prepare | {} | digest:({})",
            Self::base_info_desc(prepare.get_base()),
            msp::bytes_to_hex_str(prepare.get_value_digest())
        )
    }

    pub fn commit_desc(commit: &BftCommit) -> String {
        format!(
            "type:Commit | {} | value:({:?})",
            Self::base_info_desc(commit.get_base()),
            msp::bytes_to_hex_str(commit.get_value_digest())
        )
    }

    pub fn view_change_desc(view_change: &BftViewChange) -> String {
        format!(
            "type:ViewChange | {} | value:({:?})",
            Self::base_info_desc(view_change.get_base()),
            msp::bytes_to_hex_str(view_change.get_prepared_value_digest())
        )
    }

    pub fn view_change_value_desc(view_change_value: &BftViewChangeValue) -> String {
        let prepared_set = if view_change_value.has_prepared_set() {
            let mut prepares: Vec<String> = Vec::new();
            for i in view_change_value.get_prepared_set().get_prepare() {
                let s = Self::bft_desc(i.get_bft());
                prepares.push(s);
            }
            let pp = Self::bft_desc(
                view_change_value
                    .get_prepared_set()
                    .get_pre_prepare()
                    .get_bft(),
            );
            format!("pre_prepare({}) | prepares({:?})", pp, prepares)
        } else {
            String::from("")
        };

        let view_change = view_change_value
            .get_view_change_env()
            .get_bft()
            .get_view_change();

        return format!(
            "type:ViewChangeRawValue | {} | prepared_set:({})",
            Self::base_info_desc(view_change.get_base()),
            prepared_set
        );
    }

    pub fn new_view_desc(new_view: &BftNewView) -> String {
        let mut vc: Vec<String> = Vec::new();
        for i in new_view.get_view_changes().iter() {
            let s = Self::bft_desc(i.get_bft());
            vc.push(s);
        }

        let pre_prepares = new_view.get_pre_prepare();

        return format!(
            "type:NewView | {} | view change:({:?}) pre_prepare({})",
            Self::base_info_desc(new_view.get_base()),
            vc,
            Self::bft_desc(pre_prepares.get_bft())
        );
    }

    pub fn consensus_value_desc(data: &[u8]) -> String {
        let block = match ProtocolParser::deserialize::<Ledger>(data) {
            Ok(block) => block,
            Err(e) => {
                error!("Ledger deserialize error:{}", e);
                return format!("error consensus value");
            }
        };

        let consensus_value_hash = match block
            .get_header()
            .get_extended_data()
            .get_extra_data()
            .get(utils::general::BFT_CONSENSUS_VALUE_HASH)
        {
            Some(data) => data.clone(),
            None => {
                error!("Ledger no consensus value hash.");
                return format!("Ledger no consensus value hash");
            }
        };

        format!(
            "value hash({}) | close time({}) | previous hash({}) | ledger sequence({})",
            msp::bytes_to_hex_str(&consensus_value_hash),
            block.get_header().get_timestamp(),
            msp::bytes_to_hex_str(block.get_header().get_previous_hash()),
            block.get_header().get_height()
        )
    }
}
