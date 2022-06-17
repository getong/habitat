//! Code for actually executing user commands (e.g., `hab svc load`,
//! `hab svc stop`, etc.)

use crate::{ctl_gateway::server::CtlCommand,
            manager::{action::ActionSender,
                      ManagerState}};
use futures::{future::Future,
              task::{Context,
                     Poll}};
use log::debug;
use std::{pin::Pin,
          sync::Arc};

pub struct CtlHandler {
    /// The command to execute
    cmd:           CtlCommand,
    /// Reference to the Supervisor's main state. This is passed into
    /// commands that need to access, e.g., what services are running,
    /// etc.
    state:         Arc<ManagerState>,
    /// Communication channel back into the main Supervisor loop. This
    /// is passed into any commands as a way to send resulting actions
    /// into the Supervisor.
    action_sender: ActionSender,
}

impl CtlHandler {
    pub fn new(cmd: CtlCommand, state: Arc<ManagerState>, action_sender: ActionSender) -> Self {
        CtlHandler { cmd,
                     state,
                     action_sender }
    }
}

impl Future for CtlHandler {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        // Execute the given command.
        //
        // TODO (CM): survey the existing commands for things that may
        // take a long time to run. Anything done in the body of
        // `poll` should execute pretty quickly to avoid monopolizing
        // the reactor (long-running tasks should spawn their own
        // threads to do the main work).
        let inner = self.get_mut();
        if let Err(err) = inner.cmd.run(&inner.state, inner.action_sender.clone()) {
            debug!("CtlHandler failed, {:?}", err);
            if inner.cmd.req.transactional() {
                inner.cmd.req.reply_complete(err);
            }
        }

        // Regardless of whether the command was successful or not,
        // the future is now done.
        Poll::Ready(())
    }
}
