use std::{collections::HashMap, fmt::Display};

use protos::common::{Validator, ValidatorSet};

pub struct Validators(HashMap<String, i64>);

impl Default for Validators {
    fn default() -> Self {
        Self {
            0: HashMap::default(),
        }
    }
}

impl Clone for Validators {
    fn clone(&self) -> Self {
        let mut map: HashMap<String, i64> = HashMap::default();
        for iter in self.0.iter() {
            map.insert(iter.0.clone(), iter.1.clone());
        }
        Self { 0: map }
    }
}

impl Display for Validators {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let validators = self.iter();
        let mut str = String::from("[");
        for iter in validators.iter() {
            str.push_str(format!("({}:{})", iter.0.clone(), iter.1.clone()).as_str());
        }
        str.push_str("]");
        write!(f, "{}", str)
    }
}

impl Validators {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn remove(&mut self, address: &str) {
        self.0.remove_entry(address);
    }

    pub fn get(&self, address: &str) -> Option<i64> {
        self.0.get(address).cloned()
    }

    pub fn replica_id(&self, address: &str) -> Option<i64> {
        if let Some(index) = self.0.get(address) {
            return Some(*index);
        }
        None
    }

    pub fn contains(&self, address: &str) -> bool {
        self.0.contains_key(address)
    }

    pub fn update_validators(&mut self, validators: &ValidatorSet) {
        self.clear();

        for (index, validator) in validators.get_validators().iter().enumerate() {
            self.0
                .entry(validator.address.clone())
                .or_insert(index as i64);
        }
    }

    pub fn changed(&self, validators: &ValidatorSet) -> bool {
        if validators.get_validators().len() != self.0.keys().len() {
            return true;
        }
        for (index, validator) in validators.get_validators().iter().enumerate() {
            if let Some(replica_id) = self.0.get(validator.get_address()) {
                if (*replica_id) != (index as i64) {
                    return true;
                }
            } else {
                return true;
            }
        }
        false
    }

    pub fn iter(&self) -> Vec<(String, i64)> {
        let mut vec_validators = Vec::default();
        for (addr, index) in self.0.iter() {
            vec_validators.push((addr.to_string(), *index));
        }

        vec_validators.sort_by(|a, b| a.1.cmp(&b.1));
        vec_validators
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn validators_set(&self) -> ValidatorSet {
        let mut validators_set = ValidatorSet::default();
        let validators_vec = self.iter();
        for item in validators_vec.iter() {
            let mut validator: Validator = Validator::new();
            validator.set_address(item.0.clone());
            validator.set_pledge_amount(0);
            validators_set.validators.push(validator);
        }
        validators_set
    }
}
