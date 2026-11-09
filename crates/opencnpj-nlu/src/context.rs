//! Explicit conversation state. New self-contained requests never inherit filters.

use opencnpj_query::{export::Export, ir::*};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    pub last: Option<Request>,
    pub focus: Vec<Member>,
    pub pending: Vec<Request>,
    #[serde(default)]
    pub result_members: Vec<Member>,
    /// File delivery of the current thread, refined by follow-ups ("agora em CSV").
    #[serde(default)]
    pub export: Option<Export>,
}

impl Conversation {
    /// Called only by the host after executing the interpreter's own ranking.
    pub fn remember_result(&mut self, members: Vec<Member>) {
        self.result_members = members;
    }

    pub(crate) fn remember(&mut self, request: &Request) {
        let members: Vec<Member> = match &request.op {
            Op::Compare { members, .. } => members.iter().copied().collect(),
            _ => match &request.scope.place {
                PlaceFilter::Municipalities(ids) => ids.iter().map(|id| Member::Municipality(*id)).collect(),
                PlaceFilter::Ufs(ids) => ids.iter().map(|id| Member::Uf(*id)).collect(),
                _ => Vec::new(),
            },
        };
        for member in members.into_iter().rev() {
            self.focus.retain(|m| *m != member);
            self.focus.insert(0, member);
        }
        self.focus.truncate(4);
        self.last = Some(request.clone());
        self.pending.clear();
        self.result_members.clear();
    }
}
