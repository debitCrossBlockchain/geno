
use protos::ledger::LedgerHeader;

pub trait BlockEnv{
    fn height(&self)-> u64;
    fn coinbase(&self)-> &str;
    fn timestamp(&self)-> i64;
    fn hash(&self)-> &[u8];
}

impl BlockEnv for LedgerHeader{
    fn height(&self)-> u64{
        self.get_height()
    }
    fn coinbase(&self)-> &str{
        self.get_proposer()
    }
    fn timestamp(&self)-> i64{
        self.get_timestamp()
    }
    fn hash(&self)-> &[u8]{
        self.get_hash()
    }
}

pub trait VMState{
    fn height(&self)-> u64;
    fn coinbase(&self)-> &str;
    fn timestamp(&self)-> i64;
    fn hash(&self)-> &[u8];
}

