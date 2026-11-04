//! One module per screen. Each screen has `poll` (called every frame, applies finished
//! background calls) and `ui` (called while visible).

pub mod audit;
pub mod dashboard;
pub mod login;
pub mod operations;
pub mod paging;
pub mod plans;
pub mod searches;
pub mod usage;
pub mod users;

use crate::state::perm;

/// Left navigation entries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Dashboard,
    Users,
    Plans,
    Usage,
    Searches,
    Audit,
    Operations,
}

impl Screen {
    pub const ALL: [Screen; 7] =
        [Screen::Dashboard, Screen::Users, Screen::Plans, Screen::Usage, Screen::Searches, Screen::Audit, Screen::Operations];

    pub fn label(self) -> &'static str {
        match self {
            Screen::Dashboard => "Dashboard",
            Screen::Users => "Users",
            Screen::Plans => "Plans",
            Screen::Usage => "Usage",
            Screen::Searches => "Searches",
            Screen::Audit => "Audit",
            Screen::Operations => "Operations",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Screen::Dashboard => "📊",
            Screen::Users => "👥",
            Screen::Plans => "📋",
            Screen::Usage => "📈",
            Screen::Searches => "🔍",
            Screen::Audit => "📜",
            Screen::Operations => "⚙",
        }
    }

    /// Permission needed to open the screen.
    pub fn permission(self) -> &'static str {
        match self {
            Screen::Dashboard | Screen::Operations => perm::SYSTEM_READ,
            Screen::Users | Screen::Plans => perm::USERS_READ,
            Screen::Usage | Screen::Searches => perm::USAGE_READ,
            Screen::Audit => perm::AUDIT_READ,
        }
    }
}

#[derive(Default)]
pub struct Screens {
    pub dashboard: dashboard::Dashboard,
    pub users: users::Users,
    pub plans: plans::Plans,
    pub usage: usage::Usage,
    pub searches: searches::Searches,
    pub audit: audit::Audit,
    pub operations: operations::Operations,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_needs_a_documented_permission() {
        for s in Screen::ALL {
            assert!(s.permission().contains('.'), "{s:?}");
            assert!(!s.label().is_empty());
        }
    }
}
