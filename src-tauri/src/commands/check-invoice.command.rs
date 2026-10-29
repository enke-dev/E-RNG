use serde::Serialize;
use tauri::State;

use crate::draft::{build_invoice, totals, InputProblem, Totals};
use crate::invoice::{validate, Finding};
use crate::model::{InvoiceDraft, Kind};
use crate::store::Store;
use crate::utils::CommandResult;

#[derive(Debug, Serialize)]
pub struct Check {
    /// Input that is no invoice yet, per field
    pub problems: Vec<InputProblem>,
    /// What the profile's rules have to say about the invoice built from the input
    pub findings: Vec<Finding>,
    /// Present as soon as the input can be calculated
    pub totals: Option<Totals>,
}

/// Live feedback for the form: totals and rule findings without storing anything. An offer has no
/// rules to check, only its input and totals.
#[tauri::command]
pub fn check_invoice(
    store: State<'_, Store>,
    context_id: i64,
    kind: Kind,
    draft: InvoiceDraft,
) -> CommandResult<Check> {
    let settings = store.settings(context_id)?;
    Ok(match build_invoice(&draft, &settings, "ENTWURF") {
        Ok(invoice) => Check {
            problems: Vec::new(),
            findings: match kind {
                Kind::Invoice => validate(&invoice, draft.profile),
                Kind::Offer => Vec::new(),
            },
            totals: Some(totals(&invoice)),
        },
        Err(problems) => Check {
            problems,
            findings: Vec::new(),
            totals: None,
        },
    })
}
