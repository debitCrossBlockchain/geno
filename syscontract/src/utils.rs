macro_rules! register_contract_method {
    ($registry:expr, $name: expr,$method:expr) => {
        $registry.insert(
            $name.to_string(),
            Box::new(move |contact, json_param| $method(contact, json_param)),
        );
    };
}
