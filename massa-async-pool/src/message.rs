//! Copyright (c) 2022 MASSA LABS <info@massa.net>

//! This file defines the structure representing an asynchronous message

use massa_models::address::AddressDeserializer;
use massa_models::amount::{AmountDeserializer, AmountSerializer};
use massa_models::slot::{SlotDeserializer, SlotSerializer};
use massa_models::{
    address::Address,
    amount::Amount,
    serialization::{VecU8Deserializer, VecU8Serializer},
    slot::Slot,
};
use massa_serialization::{
    Deserializer, SerializeError, Serializer, U64VarIntDeserializer, U64VarIntSerializer,
};
use nom::error::{context, ContextError, ParseError};
use nom::multi::length_data;
use nom::sequence::tuple;
use nom::{IResult, Parser};
use serde::{Deserialize, Serialize};
use std::ops::Bound::{Excluded, Included};

/// Unique identifier of a message.
/// Also has the property of ordering by priority (highest first) following the triplet:
/// `(rev(max_gas*gas_price), emission_slot, emission_index)`
pub type AsyncMessageId = (std::cmp::Reverse<Amount>, Slot, u64);

#[derive(Clone)]
pub struct AsyncMessageIdSerializer {
    amount_serializer: AmountSerializer,
    slot_serializer: SlotSerializer,
    u64_serializer: U64VarIntSerializer,
}

impl AsyncMessageIdSerializer {
    pub fn new() -> Self {
        Self {
            amount_serializer: AmountSerializer::new(),
            slot_serializer: SlotSerializer::new(),
            u64_serializer: U64VarIntSerializer::new(),
        }
    }
}

impl Default for AsyncMessageIdSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl Serializer<AsyncMessageId> for AsyncMessageIdSerializer {
    /// ## Example
    /// ```
    /// use std::ops::Bound::Included;
    /// use massa_serialization::Serializer;
    /// use massa_models::{address::Address, amount::Amount, slot::Slot};
    /// use std::str::FromStr;
    /// use massa_async_pool::{AsyncMessage, AsyncMessageId, AsyncMessageIdSerializer};
    ///
    /// let message = AsyncMessage {
    ///     emission_slot: Slot::new(1, 0),
    ///     emission_index: 0,
    ///     sender:  Address::from_str("A12dG5xP1RDEB5ocdHkymNVvvSJmUL9BgHwCksDowqmGWxfpm93x").unwrap(),
    ///     destination: Address::from_str("A12htxRWiEm8jDJpJptr6cwEhWNcCSFWstN1MLSa96DDkVM9Y42G").unwrap(),
    ///     handler: String::from("test"),
    ///     max_gas: 10000000,
    ///     gas_price: Amount::from_str("1").unwrap(),
    ///     coins: Amount::from_str("1").unwrap(),
    ///     validity_start: Slot::new(2, 0),
    ///     validity_end: Slot::new(3, 0),
    ///     data: vec![1, 2, 3, 4]
    /// };
    /// let id: AsyncMessageId = message.compute_id();
    /// let mut serialized = Vec::new();
    /// let serializer = AsyncMessageIdSerializer::new();
    /// serializer.serialize(&id, &mut serialized).unwrap();
    /// ```
    fn serialize(
        &self,
        value: &AsyncMessageId,
        buffer: &mut Vec<u8>,
    ) -> Result<(), massa_serialization::SerializeError> {
        self.amount_serializer.serialize(&value.0 .0, buffer)?;
        self.slot_serializer.serialize(&value.1, buffer)?;
        self.u64_serializer.serialize(&value.2, buffer)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct AsyncMessageIdDeserializer {
    amount_deserializer: AmountDeserializer,
    slot_deserializer: SlotDeserializer,
    emission_index_deserializer: U64VarIntDeserializer,
}

impl AsyncMessageIdDeserializer {
    pub fn new(thread_count: u8) -> Self {
        Self {
            amount_deserializer: AmountDeserializer::new(
                Included(Amount::MIN),
                Included(Amount::MAX),
            ),
            slot_deserializer: SlotDeserializer::new(
                (Included(u64::MIN), Included(u64::MAX)),
                (Included(0), Excluded(thread_count)),
            ),
            emission_index_deserializer: U64VarIntDeserializer::new(
                Included(u64::MIN),
                Included(u64::MAX),
            ),
        }
    }
}

impl Deserializer<AsyncMessageId> for AsyncMessageIdDeserializer {
    /// ## Example
    /// ```
    /// use std::ops::Bound::Included;
    /// use massa_serialization::{Serializer, Deserializer, DeserializeError};
    /// use massa_models::{address::Address, amount::Amount, slot::Slot};
    /// use std::str::FromStr;
    /// use massa_async_pool::{AsyncMessage, AsyncMessageId, AsyncMessageIdSerializer, AsyncMessageIdDeserializer};
    ///
    /// let message = AsyncMessage {
    ///     emission_slot: Slot::new(1, 0),
    ///     emission_index: 0,
    ///     sender:  Address::from_str("A12dG5xP1RDEB5ocdHkymNVvvSJmUL9BgHwCksDowqmGWxfpm93x").unwrap(),
    ///     destination: Address::from_str("A12htxRWiEm8jDJpJptr6cwEhWNcCSFWstN1MLSa96DDkVM9Y42G").unwrap(),
    ///     handler: String::from("test"),
    ///     max_gas: 10000000,
    ///     gas_price: Amount::from_str("1").unwrap(),
    ///     coins: Amount::from_str("1").unwrap(),
    ///     validity_start: Slot::new(2, 0),
    ///     validity_end: Slot::new(3, 0),
    ///     data: vec![1, 2, 3, 4]
    /// };
    /// let id: AsyncMessageId = message.compute_id();
    /// let mut serialized = Vec::new();
    /// let serializer = AsyncMessageIdSerializer::new();
    /// let deserializer = AsyncMessageIdDeserializer::new(10);
    /// serializer.serialize(&id, &mut serialized).unwrap();
    /// let (rest, id_deser) = deserializer.deserialize::<DeserializeError>(&serialized).unwrap();
    /// assert!(rest.is_empty());
    /// assert_eq!(id, id_deser);
    /// ```
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], AsyncMessageId, E> {
        context(
            "Failed AsyncMessageId deserialization",
            tuple((
                context("Failed gas_price deserialization", |input| {
                    self.amount_deserializer.deserialize(input)
                }),
                context("Failed emission_slot deserialization", |input| {
                    self.slot_deserializer.deserialize(input)
                }),
                context("Failed emission_index deserialization", |input| {
                    self.emission_index_deserializer.deserialize(input)
                }),
            )),
        )
        .map(|(amount, slot, index)| (std::cmp::Reverse(amount), slot, index))
        .parse(buffer)
    }
}

/// Structure defining an asynchronous smart contract message
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct AsyncMessage {
    /// Slot at which the message was emitted
    pub emission_slot: Slot,

    /// Index of the emitted message within the `emission_slot`.
    /// This is used for disambiguate the emission of multiple messages at the same slot.
    pub emission_index: u64,

    /// The address that sent the message
    pub sender: Address,

    /// The address towards which the message is being sent
    pub destination: Address,

    /// the handler function name within the destination address' bytecode
    pub handler: String,

    /// Maximum gas to use when processing the message
    pub max_gas: u64,

    /// Gas price to take into account when executing the message.
    /// `max_gas * gas_price` are burned by the sender when the message is sent.
    pub gas_price: Amount,

    /// Coins sent from the sender to the target address of the message.
    /// Those coins are spent by the sender address when the message is sent,
    /// and credited to the destination address when receiving the message.
    /// In case of failure or discard, those coins are reimbursed to the sender.
    pub coins: Amount,

    /// Slot at which the message starts being valid (bound included in the validity range)
    pub validity_start: Slot,

    /// Slot at which the message stops being valid (bound not included in the validity range)
    pub validity_end: Slot,

    /// Raw payload data of the message
    pub data: Vec<u8>,
}

impl AsyncMessage {
    /// Compute the ID of the message for use when choosing which operations to keep in priority (highest score) on pool overflow.
    /// For now, the formula is simply `score = (gas_price * max_gas, rev(emission_slot), rev(emission_index))`
    pub fn compute_id(&self) -> AsyncMessageId {
        (
            std::cmp::Reverse(self.gas_price.saturating_mul_u64(self.max_gas)),
            self.emission_slot,
            self.emission_index,
        )
    }
}

pub struct AsyncMessageSerializer {
    slot_serializer: SlotSerializer,
    amount_serializer: AmountSerializer,
    u64_serializer: U64VarIntSerializer,
    vec_u8_serializer: VecU8Serializer,
}

impl AsyncMessageSerializer {
    pub fn new() -> Self {
        Self {
            slot_serializer: SlotSerializer::new(),
            amount_serializer: AmountSerializer::new(),
            u64_serializer: U64VarIntSerializer::new(),
            vec_u8_serializer: VecU8Serializer::new(),
        }
    }
}

impl Default for AsyncMessageSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl Serializer<AsyncMessage> for AsyncMessageSerializer {
    /// ## Example
    /// ```
    /// use massa_async_pool::{AsyncMessage, AsyncMessageSerializer};
    /// use massa_models::{address::Address, amount::Amount, slot::Slot};
    /// use massa_serialization::Serializer;
    /// use std::str::FromStr;
    /// let message = AsyncMessage {
    ///     emission_slot: Slot::new(1, 0),
    ///     emission_index: 0,
    ///     sender:  Address::from_str("A12dG5xP1RDEB5ocdHkymNVvvSJmUL9BgHwCksDowqmGWxfpm93x").unwrap(),
    ///     destination: Address::from_str("A12htxRWiEm8jDJpJptr6cwEhWNcCSFWstN1MLSa96DDkVM9Y42G").unwrap(),
    ///     handler: String::from("test"),
    ///     max_gas: 10000000,
    ///     gas_price: Amount::from_str("1").unwrap(),
    ///     coins: Amount::from_str("1").unwrap(),
    ///     validity_start: Slot::new(2, 0),
    ///     validity_end: Slot::new(3, 0),
    ///     data: vec![1, 2, 3, 4]
    /// };
    /// let mut buffer = Vec::new();
    /// let message_serializer = AsyncMessageSerializer::new();
    /// message_serializer.serialize(&message, &mut buffer).unwrap();
    /// ```
    fn serialize(
        &self,
        value: &AsyncMessage,
        buffer: &mut Vec<u8>,
    ) -> Result<(), massa_serialization::SerializeError> {
        self.slot_serializer
            .serialize(&value.emission_slot, buffer)?;
        self.u64_serializer
            .serialize(&value.emission_index, buffer)?;
        buffer.extend(value.sender.to_bytes());
        buffer.extend(value.destination.to_bytes());

        let handler_bytes = value.handler.as_bytes();
        let handler_name_len: u8 = handler_bytes.len().try_into().map_err(|_| {
            SerializeError::GeneralError("could not convert handler name length to u8".into())
        })?;
        buffer.extend([handler_name_len]);
        buffer.extend(handler_bytes);

        self.u64_serializer.serialize(&value.max_gas, buffer)?;
        self.amount_serializer.serialize(&value.gas_price, buffer)?;
        self.amount_serializer.serialize(&value.coins, buffer)?;
        self.slot_serializer
            .serialize(&value.validity_start, buffer)?;
        self.slot_serializer
            .serialize(&value.validity_end, buffer)?;
        self.vec_u8_serializer.serialize(&value.data, buffer)?;
        Ok(())
    }
}

pub struct AsyncMessageDeserializer {
    slot_deserializer: SlotDeserializer,
    amount_deserializer: AmountDeserializer,
    emission_index_deserializer: U64VarIntDeserializer,
    max_gas_deserializer: U64VarIntDeserializer,
    data_deserializer: VecU8Deserializer,
    address_deserializer: AddressDeserializer,
}

impl AsyncMessageDeserializer {
    pub fn new(thread_count: u8, max_async_message_data: u64) -> Self {
        Self {
            slot_deserializer: SlotDeserializer::new(
                (Included(0), Included(u64::MAX)),
                (Included(0), Excluded(thread_count)),
            ),
            amount_deserializer: AmountDeserializer::new(
                Included(Amount::MIN),
                Included(Amount::MAX),
            ),
            emission_index_deserializer: U64VarIntDeserializer::new(
                Included(0),
                Included(u64::MAX),
            ),
            max_gas_deserializer: U64VarIntDeserializer::new(Included(0), Included(u64::MAX)),
            data_deserializer: VecU8Deserializer::new(
                Included(0),
                Included(max_async_message_data),
            ),
            address_deserializer: AddressDeserializer::new(),
        }
    }
}

impl Deserializer<AsyncMessage> for AsyncMessageDeserializer {
    /// ## Example
    /// ```
    /// use massa_async_pool::{AsyncMessage, AsyncMessageSerializer, AsyncMessageDeserializer};
    /// use massa_models::{address::Address, amount::Amount, slot::Slot};
    /// use massa_serialization::{Serializer, Deserializer, DeserializeError};
    /// use std::str::FromStr;
    /// let message = AsyncMessage {
    ///     emission_slot: Slot::new(1, 0),
    ///     emission_index: 0,
    ///     sender:  Address::from_str("A12dG5xP1RDEB5ocdHkymNVvvSJmUL9BgHwCksDowqmGWxfpm93x").unwrap(),
    ///     destination: Address::from_str("A12htxRWiEm8jDJpJptr6cwEhWNcCSFWstN1MLSa96DDkVM9Y42G").unwrap(),
    ///     handler: String::from("test"),
    ///     max_gas: 10000000,
    ///     gas_price: Amount::from_str("1").unwrap(),
    ///     coins: Amount::from_str("1").unwrap(),
    ///     validity_start: Slot::new(2, 0),
    ///     validity_end: Slot::new(3, 0),
    ///     data: vec![1, 2, 3, 4]
    /// };
    /// let message_serializer = AsyncMessageSerializer::new();
    /// let mut serialized = Vec::new();
    /// message_serializer.serialize(&message, &mut serialized).unwrap();
    /// let message_deserializer = AsyncMessageDeserializer::new(32, 100000);
    /// let (rest, message_deserialized) = message_deserializer.deserialize::<DeserializeError>(&serialized).unwrap();
    /// assert!(rest.is_empty());
    /// assert_eq!(message, message_deserialized);
    /// ```
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], AsyncMessage, E> {
        context(
            "Failed AsyncMessage deserialization",
            tuple((
                context("Failed emission_slot deserialization", |input| {
                    self.slot_deserializer.deserialize(input)
                }),
                context("Failed emission_index deserialization", |input| {
                    self.emission_index_deserializer.deserialize(input)
                }),
                context("Failed sender deserialization", |input| {
                    self.address_deserializer.deserialize(input)
                }),
                context("Failed destination deserialization", |input| {
                    self.address_deserializer.deserialize(input)
                }),
                context("Failed handler deserialization", |input| {
                    let (rest, array) = length_data(|input: &'a [u8]| match input.first() {
                        Some(len) => Ok((&input[1..], *len)),
                        None => Err(nom::Err::Error(ParseError::from_error_kind(
                            input,
                            nom::error::ErrorKind::LengthValue,
                        ))),
                    })(input)?;
                    Ok((
                        rest,
                        String::from_utf8(array.to_vec()).map_err(|_| {
                            nom::Err::Error(ParseError::from_error_kind(
                                input,
                                nom::error::ErrorKind::Fail,
                            ))
                        })?,
                    ))
                }),
                context("Failed max_gas deserialization", |input| {
                    self.max_gas_deserializer.deserialize(input)
                }),
                context("Failed gas_price deserialization", |input| {
                    self.amount_deserializer.deserialize(input)
                }),
                context("Failed coins deserialization", |input| {
                    self.amount_deserializer.deserialize(input)
                }),
                context("Failed validity_start deserialization", |input| {
                    self.slot_deserializer.deserialize(input)
                }),
                context("Failed validity_end deserialization", |input| {
                    self.slot_deserializer.deserialize(input)
                }),
                context("Failed data deserialization", |input| {
                    self.data_deserializer.deserialize(input)
                }),
            )),
        )
        .map(
            |(
                emission_slot,
                emission_index,
                sender,
                destination,
                handler,
                max_gas,
                gas_price,
                coins,
                validity_start,
                validity_end,
                data,
            )| AsyncMessage {
                emission_slot,
                emission_index,
                sender,
                destination,
                handler,
                max_gas,
                gas_price,
                coins,
                validity_start,
                validity_end,
                data,
            },
        )
        .parse(buffer)
    }
}

#[cfg(test)]
mod tests {
    use massa_serialization::{DeserializeError, Deserializer, Serializer};

    use crate::{AsyncMessage, AsyncMessageDeserializer, AsyncMessageSerializer};
    use massa_models::{
        address::Address,
        amount::Amount,
        config::{MAX_ASYNC_MESSAGE_DATA, THREAD_COUNT},
        slot::Slot,
    };
    use std::str::FromStr;

    #[test]
    fn bad_serialization_version() {
        let message = AsyncMessage {
            emission_slot: Slot::new(1, 2),
            emission_index: 0,
            sender: Address::from_str("A12dG5xP1RDEB5ocdHkymNVvvSJmUL9BgHwCksDowqmGWxfpm93x")
                .unwrap(),
            destination: Address::from_str("A12htxRWiEm8jDJpJptr6cwEhWNcCSFWstN1MLSa96DDkVM9Y42G")
                .unwrap(),
            handler: String::from("test"),
            max_gas: 10000000,
            gas_price: Amount::from_str("1").unwrap(),
            coins: Amount::from_str("1").unwrap(),
            validity_start: Slot::new(2, 0),
            validity_end: Slot::new(3, 0),
            data: vec![1, 2, 3, 4],
        };
        let message_serializer = AsyncMessageSerializer::new();
        let mut serialized = Vec::new();
        message_serializer
            .serialize(&message, &mut serialized)
            .unwrap();
        let message_deserializer =
            AsyncMessageDeserializer::new(THREAD_COUNT, MAX_ASYNC_MESSAGE_DATA);
        serialized[1] = 50;
        message_deserializer
            .deserialize::<DeserializeError>(&serialized)
            .unwrap_err();
    }
}
