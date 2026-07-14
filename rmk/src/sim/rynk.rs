use std::marker::PhantomData;
use std::vec::Vec;

use rmk_types::protocol::rynk::endpoint::Endpoint;
use rmk_types::protocol::rynk::{Cmd, RynkError, RynkMessage, command};

use super::{SimHost, SimKeyboard};
use crate::types::action::{EncoderAction, KeyAction};

impl SimHost {
    pub fn rynk<'k, 'a>(&self, keyboard: &'k mut SimKeyboard<'a>) -> SimRynk<'k, 'a> {
        keyboard.enable_host();
        SimRynk { keyboard }
    }
}

pub struct SimRynk<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
}

impl<'k, 'a> SimRynk<'k, 'a> {
    pub fn request<E: Endpoint>(self, payload: E::Request) -> SimRynkReply<'k, 'a, E> {
        SimRynkReply {
            keyboard: self.keyboard,
            request: rynk_request_frame(E::CMD, 0, &payload),
            endpoint: PhantomData,
        }
    }

    pub fn get_version(self) -> SimRynkReply<'k, 'a, command::GetVersion> {
        self.request::<command::GetVersion>(())
    }

    pub fn get_key(self, layer: u8, row: u8, col: u8) -> SimRynkReply<'k, 'a, command::GetKeyAction> {
        self.request::<command::GetKeyAction>(rmk_types::protocol::rynk::KeyPosition { layer, row, col })
    }

    pub fn set_key(
        self,
        layer: u8,
        row: u8,
        col: u8,
        action: KeyAction,
    ) -> SimRynkReply<'k, 'a, command::SetKeyAction> {
        self.request::<command::SetKeyAction>(rmk_types::protocol::rynk::SetKeyRequest {
            position: rmk_types::protocol::rynk::KeyPosition { layer, row, col },
            action,
        })
    }

    pub fn get_encoder(self, layer: u8, encoder_id: u8) -> SimRynkReply<'k, 'a, command::GetEncoderAction> {
        self.request::<command::GetEncoderAction>(rmk_types::protocol::rynk::GetEncoderRequest { encoder_id, layer })
    }

    pub fn set_encoder(
        self,
        layer: u8,
        encoder_id: u8,
        action: EncoderAction,
    ) -> SimRynkReply<'k, 'a, command::SetEncoderAction> {
        self.request::<command::SetEncoderAction>(rmk_types::protocol::rynk::SetEncoderRequest {
            encoder_id,
            layer,
            action,
        })
    }
}

#[must_use = "Rynk requests must end with an expectation"]
pub struct SimRynkReply<'k, 'a, E: Endpoint> {
    keyboard: &'k mut SimKeyboard<'a>,
    request: Vec<u8>,
    endpoint: PhantomData<E>,
}

impl<'k, 'a, E: Endpoint> SimRynkReply<'k, 'a, E> {
    pub fn expect(self, response: E::Response) -> &'k mut SimKeyboard<'a> {
        let expected = rynk_response_frame(E::CMD, 0, &response);
        self.keyboard.rynk_packet(self.request, expected);
        self.keyboard
    }

    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a>
    where
        E: Endpoint<Response = ()>,
    {
        self.expect(())
    }

    pub fn expect_error(self, error: RynkError) -> &'k mut SimKeyboard<'a> {
        let expected = rynk_error_response_frame(E::CMD, 0, error);
        self.keyboard.rynk_packet(self.request, expected);
        self.keyboard
    }
}

fn rynk_request_frame<T: serde::Serialize>(cmd: Cmd, seq: u8, payload: &T) -> Vec<u8> {
    let mut buf = std::vec![0u8; rmk_types::constants::RYNK_BUFFER_SIZE];
    RynkMessage::build(&mut buf, cmd, seq, payload).expect("simulator Rynk request should encode");
    buf
}

fn rynk_response_frame<T: serde::Serialize>(cmd: Cmd, seq: u8, payload: &T) -> Vec<u8> {
    let mut buf = std::vec![0u8; rmk_types::constants::RYNK_BUFFER_SIZE];
    let msg = RynkMessage::build(&mut buf, cmd, seq, &Ok::<&T, RynkError>(payload))
        .expect("simulator Rynk response should encode");
    let frame_len = msg.frame_len();
    buf.truncate(frame_len);
    buf
}

fn rynk_error_response_frame(cmd: Cmd, seq: u8, error: RynkError) -> Vec<u8> {
    let mut buf = std::vec![0u8; rmk_types::constants::RYNK_BUFFER_SIZE];
    let msg = RynkMessage::build(&mut buf, cmd, seq, &Err::<(), RynkError>(error))
        .expect("simulator Rynk error response should encode");
    let frame_len = msg.frame_len();
    buf.truncate(frame_len);
    buf
}
