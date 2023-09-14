use protos::consensus::{BftMessageType, BftSign};
use std::cmp::Ordering;

#[derive(Debug, Copy, Clone, Hash)]
pub struct BftInstanceIndex {
    pub view_number: i64,
    pub sequence: u64,
}

impl PartialEq for BftInstanceIndex {
    fn eq(&self, other: &Self) -> bool {
        self.view_number == other.view_number && self.sequence == other.sequence
    }
}

impl Eq for BftInstanceIndex {}

impl BftInstanceIndex {
    pub fn new(view_number: i64, sequence: u64) -> Self {
        Self {
            view_number,
            sequence,
        }
    }

    pub fn less_than(&self, index: &BftInstanceIndex) -> bool {
        if self.view_number < index.view_number {
            return true;
        } else if (self.view_number == index.view_number) && (self.sequence < index.sequence) {
            return true;
        }
        return false;
    }

    pub fn index(bft_sign: &BftSign) -> BftInstanceIndex {
        let bft = bft_sign.get_bft();
        match bft.get_msg_type() {
            BftMessageType::PRE_PREPARE => BftInstanceIndex {
                view_number: bft.get_pre_prepare().get_base().get_view_number(),
                sequence: bft.get_pre_prepare().get_base().get_sequence(),
            },
            BftMessageType::PREPARE => BftInstanceIndex {
                view_number: bft.get_prepare().get_base().get_view_number(),
                sequence: bft.get_prepare().get_base().get_sequence(),
            },
            BftMessageType::COMMIT => BftInstanceIndex {
                view_number: bft.get_commit().get_base().get_view_number(),
                sequence: bft.get_commit().get_base().get_sequence(),
            },
            _ => {
                return BftInstanceIndex {
                    view_number: 0,
                    sequence: 0,
                };
            }
        }
    }

    pub fn cmp(a: &BftInstanceIndex, b: &BftInstanceIndex) -> Ordering {
        if a.view_number < b.view_number {
            return Ordering::Less;
        } else if a.view_number < b.view_number {
            return Ordering::Greater;
        } else {
            if a.sequence < b.sequence {
                return Ordering::Less;
            } else if a.sequence < b.sequence {
                return Ordering::Greater;
            } else {
                return Ordering::Equal;
            }
        }
    }
}
