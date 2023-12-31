macro_rules! register_rpc_method {
    ($registry:expr, $name: expr, $method: expr, $required_num_args: expr, $opt_num_args: expr) => {
        $registry.insert(
            $name.to_string(),
            Box::new(move |service, request| {
                Box::pin(async move {
                    if request.params.len() < $required_num_args
                        || request.params.len() > $required_num_args + $opt_num_args
                    {
                        let expected = if $opt_num_args == 0 {
                            format!("{}", $required_num_args)
                        } else {
                            format!(
                                "{}..{}",
                                $required_num_args,
                                $required_num_args + $opt_num_args
                            )
                        };
                        anyhow::bail!(JsonRpcError::invalid_params_size(format!(
                            "wrong number of arguments (given {}, expected {})",
                            request.params.len(),
                            expected,
                        )));
                    }

                    // fail_point!(format!("jsonrpc::method::{}", $name).as_str(), |_| {
                    //     Err(anyhow::format_err!("Injected error for method {} error", $name).into())
                    // });
                    Ok(serde_json::to_value($method(service, request).await?)?)
                })
            }),
        );
    };
}
