use protobuf::{Message, ProtobufResult};
pub struct ProtocolParser {}

impl ProtocolParser {
    pub fn deserialize<T: Message>(data: &[u8]) -> anyhow::Result<T> {
        let result: ProtobufResult<T> = Message::parse_from_bytes(data);
        match result {
            Ok(value) => return Ok(value),
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "failed to deserialize the proto message,err {:?}",
                    err
                ))
            }
        }
    }

    pub fn serialize<T: Message>(value: &T) -> Vec<u8> {
        value.write_to_bytes().unwrap()
    }
}

trait ProtoDeserialize<T:Message> {
    type Out;
    fn deserialize(data: &Self::Out) -> anyhow::Result<T>;
    fn serialize(value: &T) -> Self::Out;
}

impl<T:Message> ProtoDeserialize<T> for T
{
    type Out = Vec<u8>;
    fn deserialize(data: &Self::Out) -> anyhow::Result<T> {
        let result: ProtobufResult<T> = Message::parse_from_bytes(data);
        match result {
            Ok(value) => Ok(value),
            Err(err) => {
                anyhow::bail!(
                    "failed to deserialize the proto message, err: {:?}",
                    err
                )
            }
        }
    }

    fn serialize(value: &T) -> Self::Out {
        value.write_to_bytes().unwrap()
    }
}
