//! One set of tools per agent (ADR-0203, amendment of 2026-10-19).
//!
//! An agent can reach Ghostlight through two server entries at once, such as Claude Code with both
//! the plugin and the installer's entry. Each entry starts its own edge, and the agent would offer
//! the model every tool twice. A client honors an added tool on its next turn but never removes
//! one, so a connection keeps its tools back only by answering its first catalog with none.
//!
//! [`Toolsets`] holds every live MCP connection. A connection is one agent's when its observed
//! source is the app of the agent its client claims to be ([`AgentApp`]): its running process,
//! by id and start time, and the catalog agent. When a connection of that agent already offers the
//! tools, a new one stands by: its instructions say Ghostlight is already connected, and its
//! catalog is empty. When the connection that offers them ends, the first that stands by is
//! promoted and tells its client that its tool list changed.
//!
//! A connection a resume token continues keeps the role its client saw, whatever order the edges
//! reconnect in after a restart: one that stood by stands by again, and one that offered the tools
//! offers them. Its client never drops a tool it was given, and lists the tools again on every
//! catalog change, so only the one it already gave them may offer them.
//!
//! The process comes from the operating system, never from the client. The claim only keeps
//! connections apart: a source that hosts other agents, such as an editor whose terminal runs an
//! agent, is not the claimed agent's own app, so those connections never stand by. Two separate
//! processes, a source that could not be read, and the command line never stand by either.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::intake::mcp::Seating;
use crate::intake::session::IntakeChannel;
use crate::language::provenance::claimed_agent;
use crate::provenance::{Observation, SourceProcess};

use super::lifetime::Drain;
use super::{lock, McpWire};

/// One agent's running app: the process Ghostlight saw start a connection, when that process is
/// the installed app of the agent the connection's client claims to be.
#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentApp {
    process: SourceProcess,
    agent: String,
}

impl AgentApp {
    /// The agent app of a connection on `channel` whose source is `observation` and whose client
    /// claims `claim`; none when the source was not read or is not the claimed agent's own app.
    fn of(
        channel: IntakeChannel,
        observation: Option<&Observation>,
        claim: Option<&str>,
    ) -> Option<Self> {
        let observation = observation?;
        let agent = claimed_agent(claim, channel)?;
        (observation.source.agent.as_deref() == Some(agent.as_str())).then_some(Self {
            process: observation.process,
            agent,
        })
    }
}

/// One live MCP connection.
struct Member {
    id: u64,
    /// The agent app it belongs to, once it is seated; none for a connection that is never one
    /// agent's.
    agent: Option<AgentApp>,
    /// Whether its client is offered the tools.
    offers: bool,
    wire: Arc<McpWire>,
}

/// Every live MCP connection, and which of them offers each agent the tools.
pub(super) struct Toolsets {
    members: Mutex<Vec<Member>>,
    next: AtomicU64,
    /// The service's quit: a connection that ends while the service ends promotes nobody.
    drain: Arc<Drain>,
}

/// One connection's place among the [`Toolsets`]. Dropping it leaves, and promotes the connection
/// that stands by for the same agent when this one offered the tools.
pub(super) struct Seat {
    toolsets: Arc<Toolsets>,
    id: u64,
}

impl Toolsets {
    /// No connection yet, for a service whose quit is `drain`.
    pub(super) fn new(drain: Arc<Drain>) -> Arc<Self> {
        Arc::new(Self {
            members: Mutex::default(),
            next: AtomicU64::new(0),
            drain,
        })
    }

    /// Enter the connection whose stream is `wire`. Until it is seated it offers the tools and
    /// belongs to no agent.
    pub(super) fn enter(self: &Arc<Self>, wire: Arc<McpWire>) -> Seat {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.members).push(Member {
            id,
            agent: None,
            offers: true,
            wire,
        });
        Seat {
            toolsets: Arc::clone(self),
            id,
        }
    }

    /// Seat connection `id` as `agent`'s: it stands by, and true is returned, when `stood_by`
    /// says a resumed connection stood by, or, for a fresh one, when another connection of that
    /// agent already offers the tools. A connection that is no agent's never stands by.
    fn seat(&self, id: u64, agent: Option<AgentApp>, stood_by: Option<bool>) -> bool {
        let mut members = lock(&self.members);
        let stands_by = agent.is_some()
            && stood_by.unwrap_or_else(|| {
                members
                    .iter()
                    .any(|member| member.id != id && member.offers && member.agent == agent)
            });
        if let Some(member) = members.iter_mut().find(|member| member.id == id) {
            member.agent = agent;
            member.offers = !stands_by;
        }
        stands_by
    }

    /// Remove connection `id`. When it offered its agent the tools, no other connection of that
    /// agent does and the service is not ending, the first that stands by now offers them, and its
    /// stream is returned.
    fn leave(&self, id: u64) -> Option<Arc<McpWire>> {
        let mut members = lock(&self.members);
        let index = members.iter().position(|member| member.id == id)?;
        let left = members.remove(index);
        let agent = left.agent.filter(|_| left.offers)?;
        if self.drain.is_quitting()
            || members
                .iter()
                .any(|member| member.offers && member.agent.as_ref() == Some(&agent))
        {
            return None;
        }
        let promoted = members
            .iter_mut()
            .find(|member| member.agent.as_ref() == Some(&agent))?;
        promoted.offers = true;
        Some(Arc::clone(&promoted.wire))
    }
}

impl Seat {
    /// The decision that seats this connection, on `channel` and whose source is `observation`,
    /// once its client's claim is known.
    pub(super) fn seating(
        &self,
        channel: IntakeChannel,
        observation: Option<&Observation>,
    ) -> Seating {
        let toolsets = Arc::clone(&self.toolsets);
        let id = self.id;
        let observation = observation.cloned();
        Seating::new(move |claim, stood_by| {
            let agent = AgentApp::of(channel, observation.as_ref(), claim);
            toolsets.seat(id, agent, stood_by)
        })
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        if let Some(promoted) = self.toolsets.leave(self.id) {
            promoted.promote();
        }
    }
}
