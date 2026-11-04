//! Session state shared by all screens: the authenticated client, the staff user and their
//! permissions, cached roles and plans, notifications, and cross-screen signals.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::api::{ApiError, ApiResult, Client};
use crate::models::{Plan, RoleInfo, User, Whoami};

/// Permission names from docs/ACCOUNTS.md.
pub mod perm {
    pub const USERS_READ: &str = "users.read";
    pub const USERS_MODERATE: &str = "users.moderate";
    pub const USERS_MANAGE: &str = "users.manage";
    pub const PLANS_MANAGE: &str = "plans.manage";
    pub const USAGE_READ: &str = "usage.read";
    pub const AUDIT_READ: &str = "audit.read";
    pub const SYSTEM_READ: &str = "system.read";
    pub const SYSTEM_OPERATE: &str = "system.operate";
    pub const SETTINGS_MANAGE: &str = "settings.manage";
}

pub const ROLES: [&str; 4] = ["user", "analyst", "moderator", "admin"];
pub const STATUSES: [&str; 4] = ["pending", "active", "suspended", "banned"];

pub struct Session {
    pub client: Client,
    pub user: User,
    pub permissions: BTreeSet<String>,
    pub roles: Vec<RoleInfo>,
    pub plans: Vec<Plan>,
}

impl Session {
    pub fn new(client: Client, me: Whoami) -> Self {
        Self { client, user: me.user, permissions: me.permissions.into_iter().collect(), roles: Vec::new(), plans: Vec::new() }
    }

    pub fn can(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    /// Role names for pickers: from `/admin/api/roles` when loaded, else the documented set.
    pub fn role_names(&self) -> Vec<String> {
        if self.roles.is_empty() {
            ROLES.iter().map(|r| r.to_string()).collect()
        } else {
            self.roles.iter().map(|r| r.role.clone()).collect()
        }
    }

    pub fn plan_names(&self) -> Vec<String> {
        self.plans.iter().map(|p| p.name.clone()).collect()
    }

    /// Whether the signed-in staff member may moderate `target` (moderators act only on `user`
    /// accounts; nobody moderates themselves).
    pub fn can_moderate(&self, target: &User) -> bool {
        self.can(perm::USERS_MODERATE) && target.id != self.user.id && (self.can(perm::USERS_MANAGE) || target.role == "user")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub created: Instant,
}

impl Toast {
    pub fn ttl(&self) -> Duration {
        match self.kind {
            ToastKind::Error => Duration::from_secs(10),
            _ => Duration::from_secs(5),
        }
    }
}

#[derive(Default)]
pub struct Toasts {
    pub items: Vec<Toast>,
    /// Last error, kept for the status bar until the next success.
    pub last_error: Option<String>,
}

impl Toasts {
    pub fn push(&mut self, kind: ToastKind, text: impl Into<String>) {
        let text = text.into();
        if kind == ToastKind::Error {
            self.last_error = Some(text.clone());
        }
        self.items.push(Toast { kind, text, created: Instant::now() });
        if self.items.len() > 6 {
            self.items.remove(0);
        }
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.last_error = None;
        self.push(ToastKind::Success, text);
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Info, text);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Error, text);
    }

    pub fn expire(&mut self) {
        self.items.retain(|t| t.created.elapsed() < t.ttl());
    }
}

/// Requests a screen can make of the application.
#[derive(Default)]
pub struct Signals {
    /// A call returned 401: drop the session and show the login screen.
    pub unauthorized: bool,
    /// Open the Users screen with this account selected.
    pub open_user: Option<i64>,
    /// Plans changed: refresh the cached plan list.
    pub plans_changed: bool,
}

/// What a screen receives every frame.
pub struct Env<'a> {
    pub ctx: egui::Context,
    pub session: &'a Session,
    pub toasts: &'a mut Toasts,
    pub signals: &'a mut Signals,
}

impl Env<'_> {
    pub fn client(&self) -> &Client {
        &self.session.client
    }

    pub fn can(&self, permission: &str) -> bool {
        self.session.can(permission)
    }

    /// Unwraps a result, reporting errors (a 401 also ends the session).
    pub fn check<T>(&mut self, r: ApiResult<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.report(&e);
                None
            }
        }
    }

    pub fn report(&mut self, e: &ApiError) {
        if e.is_unauthorized() {
            self.signals.unauthorized = true;
        }
        self.toasts.error(e.to_string());
    }
}
