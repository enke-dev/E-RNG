use tauri::State;

use crate::billing::OfferBilling;
use crate::draft::{build_invoice, totals};
use crate::model::{Answer, Event, InvoiceDraft, Kind, Linked, ListEntry, StoredInvoice};
use crate::on_account::OnAccountInvoice;
use crate::relations::Relation;
use crate::store::Store;
use crate::utils::{today, CommandError, CommandResult};

#[tauri::command]
pub fn invoices_list(
    store: State<'_, Store>,
    context_id: Option<i64>,
    kind: Option<Kind>,
    with_archived: bool,
) -> CommandResult<Vec<ListEntry>> {
    store
        .invoices(context_id, kind, with_archived)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn invoice_get(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store.invoice(id).map_err(CommandError::from)
}

/// A blank draft of a kind dated today, with the layout, texts and validity the settings of the
/// context give it, not stored until it is saved.
#[tauri::command]
pub fn invoice_blank(
    store: State<'_, Store>,
    context_id: i64,
    kind: Kind,
) -> CommandResult<InvoiceDraft> {
    Ok(store.settings(context_id)?.blank_draft(kind, &today()))
}

/// Marks an exported invoice as paid on a date (ISO), or as open again.
#[tauri::command]
pub fn invoice_set_paid(
    store: State<'_, Store>,
    id: i64,
    paid_at: Option<String>,
) -> CommandResult<StoredInvoice> {
    store
        .set_paid(id, paid_at.as_deref())
        .map_err(CommandError::from)
}

/// Notes a payment (gross amount, ISO date) received on an exported invoice
#[tauri::command]
pub fn invoice_add_payment(
    store: State<'_, Store>,
    id: i64,
    amount: String,
    paid_on: String,
) -> CommandResult<StoredInvoice> {
    store
        .add_payment(id, &amount, &paid_on)
        .map_err(CommandError::from)
}

/// Takes a payment back
#[tauri::command]
pub fn invoice_remove_payment(
    store: State<'_, Store>,
    payment_id: i64,
) -> CommandResult<StoredInvoice> {
    store.remove_payment(payment_id).map_err(CommandError::from)
}

/// Marks a created offer as accepted or declined on a day (ISO), or takes the answer back
#[tauri::command]
pub fn offer_set_answer(
    store: State<'_, Store>,
    id: i64,
    answer: Option<Answer>,
    on: Option<String>,
) -> CommandResult<StoredInvoice> {
    let answer = match (answer, on.as_deref()) {
        (Some(answer), Some(on)) => Some((answer, on)),
        (Some(_), None) => return Err("Das Datum der Antwort fehlt.".into()),
        (None, _) => None,
    };
    store.set_answer(id, answer).map_err(CommandError::from)
}

/// The data a version of a document was exported or baked with, to compare versions
#[tauri::command]
pub fn invoice_snapshot(store: State<'_, Store>, event_id: i64) -> CommandResult<InvoiceDraft> {
    store.snapshot(event_id).map_err(CommandError::from)
}

/// What happened to a document, in order
#[tauri::command]
pub fn invoice_events(store: State<'_, Store>, id: i64) -> CommandResult<Vec<Event>> {
    store.events(id).map_err(CommandError::from)
}

/// Creates (`id` is `None`, under `context_id`, of `kind`) or updates a draft, which stays under the
/// context and kind it was made under. Whatever state the form is in is saved, the total shown in
/// the list is kept only while the draft can be calculated.
#[tauri::command]
pub fn invoice_save(
    store: State<'_, Store>,
    id: Option<i64>,
    context_id: Option<i64>,
    kind: Kind,
    draft: InvoiceDraft,
    case_id: Option<i64>,
) -> CommandResult<StoredInvoice> {
    let context_id = match (id, case_id, context_id) {
        (Some(id), _, _) => store.invoice(id)?.context_id,
        (None, Some(case), _) => store.case_context(case)?,
        (None, None, Some(context_id)) => context_id,
        (None, None, None) => return Err("Für eine neue Rechnung fehlt der Kontext.".into()),
    };
    let settings = store.settings(context_id)?;
    // what the deducted invoices came to comes from them, before the total is worked out
    let mut draft = draft;
    store.fill_deductions(&mut draft)?;
    store.fill_correction(&mut draft);
    let total = build_invoice(&draft, &settings, "ENTWURF")
        .ok()
        .map(|invoice| totals(&invoice).due);
    match (id, case_id) {
        // a new document within a Vorgang, else in one of its own
        (None, Some(case)) => store.save_draft_in_case(case, kind, &draft, total.as_deref()),
        _ => store.save_draft(id, context_id, kind, &draft, total.as_deref()),
    }
    .map_err(CommandError::from)
}

/// Takes an exported invoice out of the list, it stays as it is.
#[tauri::command]
pub fn invoice_archive(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.archive_invoice(id).map_err(CommandError::from)
}

/// Puts an archived invoice back into the list.
#[tauri::command]
pub fn invoice_unarchive(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store.unarchive_invoice(id).map_err(CommandError::from)
}

#[tauri::command]
pub fn invoice_duplicate(
    store: State<'_, Store>,
    id: i64,
    same_case: bool,
) -> CommandResult<StoredInvoice> {
    store
        .duplicate_to(id, &today(), same_case)
        .map_err(CommandError::from)
}

/// A new offer draft that refers to the created offer `id` as a Nachtrag, an alternative or a
/// replacement, in its Vorgang
#[tauri::command]
pub fn offer_related(
    store: State<'_, Store>,
    id: i64,
    relation: Relation,
) -> CommandResult<StoredInvoice> {
    store
        .related_offer(id, relation, &today())
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn invoice_delete(store: State<'_, Store>, id: i64) -> CommandResult<()> {
    store.delete_draft(id).map_err(CommandError::from)
}

/// A new invoice draft from a created offer, linked to it: customer and positions are copied, layout
/// and texts are what the invoices of its context have.
#[tauri::command]
pub fn offer_to_invoice(store: State<'_, Store>, offer_id: i64) -> CommandResult<StoredInvoice> {
    let settings = store.settings(store.invoice(offer_id)?.context_id)?;
    store
        .invoice_from_offer(offer_id, &today(), &settings)
        .map_err(CommandError::from)
}

/// The documents an offer or an invoice is linked to
/// What is billed of a created offer, position by position; `except` leaves an invoice draft out
#[tauri::command]
pub fn offer_billing(
    store: State<'_, Store>,
    id: i64,
    except: Option<i64>,
) -> CommandResult<OfferBilling> {
    store.billing(id, except).map_err(CommandError::from)
}

/// What is billed of the offers of a Vorgang that can still be billed
#[tauri::command]
pub fn case_billing(
    store: State<'_, Store>,
    case_id: i64,
    except: Option<i64>,
) -> CommandResult<Vec<OfferBilling>> {
    store
        .case_billing(case_id, except)
        .map_err(CommandError::from)
}

/// A Stornorechnung for the created invoice `id`: a draft that mirrors it
#[tauri::command]
pub fn invoice_cancel(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store
        .cancel_invoice(id, &today())
        .map_err(CommandError::from)
}

/// An Ersatzrechnung for the cancelled invoice `id`: a draft of what it said
#[tauri::command]
pub fn invoice_replace(store: State<'_, Store>, id: i64) -> CommandResult<StoredInvoice> {
    store
        .replace_invoice(id, &today())
        .map_err(CommandError::from)
}

/// The created invoices on account of a Vorgang, with what deducts each
#[tauri::command]
pub fn case_on_account(
    store: State<'_, Store>,
    case_id: i64,
    except: Option<i64>,
) -> CommandResult<Vec<OnAccountInvoice>> {
    store
        .case_on_account(case_id, except)
        .map_err(CommandError::from)
}

/// The documents of the other kind a document is billed with
#[tauri::command]
pub fn document_links(store: State<'_, Store>, id: i64) -> CommandResult<Vec<Linked>> {
    store.links(id).map_err(CommandError::from)
}
