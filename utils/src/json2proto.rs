extern crate protobuf;
extern crate serde_json;

use protobuf::Message;
use protobuf::descriptor::FieldDescriptorProto_Type;
use protobuf::reflect::FieldDescriptor;
use serde_json::Value;
use std::borrow::BorrowMut;


pub fn json_to_proto(root:serde_json::Value,message: &dyn Message) ->bool{

    if let Some(m) = root.as_object() {
        for it in m.iter(){
            let name = it.0;
            let value = it.1;
            let mut find_feild =false;

            for field in message.descriptor().fields() {
                if field.name() == name {
                    find_feild =true;

                    if field.is_repeated() {
                        if !value.is_array() {
                            return false;
                        }

                        let arr = value.as_array().unwrap();
                        for v in arr.iter() {
                            json2field(message,field,value);
                        }
                    }else{

                        json2field(message,field,value);

                    }
                    break;
                }
            }
        }
    }
    true
}

fn json2field(message: &dyn Message, fd: &FieldDescriptor,value:&Value)->bool{

    true
}
