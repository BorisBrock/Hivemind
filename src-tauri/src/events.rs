//! Events pushed to the frontend when the agent manager changes things.

use serde::Serialize;
use tauri::AppHandle;
use tauri_specta::Event;

use crate::agent::manager::Notifier;
use crate::models::{Ticket, TicketEvent, Usage};

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
pub struct TicketChanged(pub Ticket);

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
pub struct TicketEventAdded(pub TicketEvent);

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
pub struct UsageChanged(pub Usage);

pub struct TauriNotifier(pub AppHandle);

impl Notifier for TauriNotifier {
    fn ticket_changed(&self, ticket: &Ticket) {
        let _ = TicketChanged(ticket.clone()).emit(&self.0);
    }

    fn event_added(&self, event: &TicketEvent) {
        let _ = TicketEventAdded(event.clone()).emit(&self.0);
    }

    fn usage_changed(&self, usage: &Usage) {
        let _ = UsageChanged(usage.clone()).emit(&self.0);
    }
}
