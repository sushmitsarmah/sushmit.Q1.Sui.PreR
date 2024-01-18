// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, fmt::Debug, sync::Arc, thread};

use mysten_metrics::monitored_scope;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

use crate::{
    block::{Block, BlockRef, Round},
    context::Context,
    core::Core,
    core_thread::CoreError::Shutdown,
};

#[allow(unused)]
pub(crate) struct CoreThreadDispatcherHandle {
    sender: mpsc::Sender<CoreThreadCommand>,
    join_handle: thread::JoinHandle<()>,
}

impl CoreThreadDispatcherHandle {
    #[allow(unused)]
    pub fn stop(self) {
        // drop the sender, that will force all the other weak senders to not able to upgrade.
        drop(self.sender);
        self.join_handle.join().ok();
    }
}

#[allow(unused)]
struct CoreThread {
    core: Core,
    receiver: mpsc::Receiver<CoreThreadCommand>,
    context: Arc<Context>,
}

impl CoreThread {
    pub fn run(mut self) {
        tracing::debug!("Started core thread");

        while let Some(command) = self.receiver.blocking_recv() {
            let _scope = monitored_scope("CoreThread::loop");
            self.context.metrics.node_metrics.core_lock_dequeued.inc();
            match command {
                CoreThreadCommand::AddBlocks(blocks, sender) => {
                    let missing_blocks = self.core.add_blocks(blocks);
                    sender.send(missing_blocks).ok();
                }
                CoreThreadCommand::ForceNewBlock(round, sender) => {
                    self.core.force_new_block(round);
                    sender.send(()).ok();
                }
                CoreThreadCommand::GetMissing(sender) => {
                    // TODO: implement the logic to fetch the missing blocks.
                    sender.send(vec![]).ok();
                }
            }
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct CoreThreadDispatcher {
    sender: mpsc::WeakSender<CoreThreadCommand>,
    context: Arc<Context>,
}

enum CoreThreadCommand {
    /// Add blocks to be processed and accepted
    AddBlocks(Vec<Block>, oneshot::Sender<Vec<BlockRef>>),
    /// Called when a leader timeout occurs and a block should be produced
    ForceNewBlock(Round, oneshot::Sender<()>),
    /// Request missing blocks that need to be synced.
    GetMissing(oneshot::Sender<Vec<HashSet<BlockRef>>>),
}

#[derive(Error, Debug)]
pub(crate) enum CoreError {
    #[error("Core thread shutdown: {0}")]
    Shutdown(RecvError),
}

#[allow(unused)]
impl CoreThreadDispatcher {
    pub fn start(core: Core, context: Arc<Context>) -> (Self, CoreThreadDispatcherHandle) {
        let (sender, receiver) = mpsc::channel(32);
        let core_thread = CoreThread {
            core,
            receiver,
            context: context.clone(),
        };
        let join_handle = thread::Builder::new()
            .name("consensus-core".to_string())
            .spawn(move || core_thread.run())
            .unwrap();
        // Explicitly using downgraded sender in order to allow sharing the CoreThreadDispatcher but
        // able to shutdown the CoreThread by dropping the original sender.
        let dispatcher = CoreThreadDispatcher {
            sender: sender.downgrade(),
            context,
        };
        let handler = CoreThreadDispatcherHandle {
            join_handle,
            sender,
        };
        (dispatcher, handler)
    }

    pub async fn add_blocks(&self, blocks: Vec<Block>) -> Result<Vec<BlockRef>, CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::AddBlocks(blocks, sender))
            .await;
        receiver.await.map_err(Shutdown)
    }

    pub async fn force_new_block(&self, round: Round) -> Result<(), CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::ForceNewBlock(round, sender))
            .await;
        receiver.await.map_err(Shutdown)
    }

    pub async fn get_missing_blocks(&self) -> Result<Vec<HashSet<BlockRef>>, CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::GetMissing(sender)).await;
        receiver.await.map_err(Shutdown)
    }

    async fn send(&self, command: CoreThreadCommand) {
        self.context.metrics.node_metrics.core_lock_enqueued.inc();
        if let Some(sender) = self.sender.upgrade() {
            if let Err(err) = sender.send(command).await {
                warn!(
                    "Couldn't send command to core thread, probably is shutting down: {}",
                    err
                );
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::context::Context;
    use crate::metrics::test_metrics;
    use consensus_config::{AuthorityIndex, Committee, Parameters};
    use sui_protocol_config::ProtocolConfig;

    #[tokio::test]
    async fn test_core_thread() {
        let (committee, _) = Committee::new_for_test(0, vec![1, 1, 1, 1]);
        let metrics = test_metrics();
        let context = Arc::new(Context::new(
            AuthorityIndex::new_for_test(0),
            committee,
            Parameters::default(),
            ProtocolConfig::get_for_min_version(),
            metrics,
        ));

        let core = Core::new(context.clone());
        let (core_dispatcher, handle) = CoreThreadDispatcher::start(core, context);

        // Now create some clones of the dispatcher
        let dispatcher_1 = core_dispatcher.clone();
        let dispatcher_2 = core_dispatcher.clone();

        // Try to send some commands
        assert!(dispatcher_1.add_blocks(vec![]).await.is_ok());
        assert!(dispatcher_2.add_blocks(vec![]).await.is_ok());

        // Now shutdown the dispatcher
        handle.stop();

        // Try to send some commands
        assert!(dispatcher_1.add_blocks(vec![]).await.is_err());
        assert!(dispatcher_2.add_blocks(vec![]).await.is_err());
    }
}
