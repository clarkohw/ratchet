// Copyright 2015-2021 SWIM.AI inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::codec::Codec;
use crate::errors::Error;
use crate::extensions::NegotiatedExtension;
use crate::handshake::{exec_client_handshake, HandshakeResult, ProtocolRegistry};
use crate::protocol::frame::Message;
use crate::{
    Deflate, Extension, ExtensionHandshake, Request, Role, WebSocketConfig, WebSocketStream,
};
use futures::io::{ReadHalf, WriteHalf};
use futures::AsyncReadExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::codec::{Decoder, Encoder};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

// todo replace once futures::BiLock is stabilised
//  https://github.com/rust-lang/futures-rs/issues/2289
type Writer<S> = Arc<Mutex<WriteHalf<Compat<S>>>>;

type SplitSocket<S, C, E> = (WebSocketTx<S, C, E>, WebSocketRx<S, C, E>);

pub struct WebSocketRx<S, C = Codec, E = Deflate> {
    writer: Writer<S>,
    codec: C,
    reader: ReadHalf<Compat<S>>,
    role: Role,
    extension: NegotiatedExtension<E>,
    config: WebSocketConfig,
}

pub struct WebSocketTx<S, C = Codec, E = Deflate> {
    writer: Writer<S>,
    codec: C,
    role: Role,
    extension: NegotiatedExtension<E>,
    config: WebSocketConfig,
}

pub async fn client<S, C, E>(
    config: WebSocketConfig,
    mut stream: S,
    request: Request,
    codec: C,
    extension: E,
    subprotocols: ProtocolRegistry,
) -> Result<(SplitSocket<S, C, E::Extension>, Option<String>), Error>
where
    S: WebSocketStream,
    C: Encoder<Message, Error = Error> + Decoder + Clone,
    E: ExtensionHandshake,
{
    let HandshakeResult {
        protocol,
        extension,
    } = exec_client_handshake(&mut stream, request, extension, subprotocols).await?;

    let (read_half, write_half) = stream.compat().split();
    let writer = Arc::new(Mutex::new(write_half));
    let tx = WebSocketTx {
        writer: writer.clone(),
        codec: codec.clone(),
        role: Role::Client,
        extension: extension.clone(),
        config: config.clone(),
    };

    let rx = WebSocketRx {
        writer,
        codec,
        reader: read_half,
        role: Role::Client,
        extension: Default::default(),
        config,
    };

    Ok(((tx, rx), protocol))
}

impl<S, C, E> WebSocketTx<S, C, E>
where
    S: WebSocketStream,
    C: Encoder<Message, Error = Error> + Decoder + Clone,
    E: Extension,
{
    pub async fn read_frame_contents(&mut self, _bytes: &mut [u8]) -> Result<usize, Error> {
        let mut guard = self.writer.lock().await;
        let _contents = &mut (*guard);
        unimplemented!()
    }
}
