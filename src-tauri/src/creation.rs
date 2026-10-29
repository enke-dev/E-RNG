//! Turning a draft into an invoice, and handing its originals out. Creating is one step: the rules
//! are checked, the PDF and XML are made and validated, and only if all of it holds the invoice gets
//! its number, freezes its settings and layout, locks, and its files are kept (`originals`). When
//! anything fails nothing is stored: no number, no lock, no files. An offer is made the same way
//! but is only a PDF: no XML, no rule check, no validator.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;

use crate::draft::{build_invoice, totals};
use crate::model::{CustomLayout, InvoiceFiles, Kind, Settings, Status, StoredInvoice};
use crate::originals;
use crate::package::{Issue, Level};
use crate::pipeline::{self, ProduceError, Produced};
use crate::render::Branding;
use crate::sidecar::Sidecars;
use crate::store::{Sequence, Store};
use crate::utils::issue_day;

/// The contexts that are in the middle of creating an invoice: two creations in one context would
/// race for the same number, so the second one waits for the user to try again.
#[derive(Default)]
pub struct Creating(Mutex<HashSet<i64>>);

pub struct CreatingGuard<'a> {
    creating: &'a Creating,
    context: i64,
}

impl Creating {
    // a set of numbers stays valid when a holder panicked, so a poisoned lock is no reason to fail
    fn waiting(&self) -> std::sync::MutexGuard<'_, HashSet<i64>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether an invoice is being created somewhere
    pub fn any(&self) -> bool {
        !self.waiting().is_empty()
    }

    pub fn start(&self, context: i64) -> Option<CreatingGuard<'_>> {
        // the lock is let go before a guard exists: dropping a guard takes it again
        let inserted = self.waiting().insert(context);
        inserted.then(|| CreatingGuard {
            creating: self,
            context,
        })
    }
}

impl Drop for CreatingGuard<'_> {
    fn drop(&mut self) {
        self.creating.waiting().remove(&self.context);
    }
}

/// A work directory that is gone when it goes out of scope
struct Work(PathBuf);

impl Drop for Work {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn failed(error: impl std::fmt::Display) -> ProduceError {
    ProduceError::Failed {
        message: crate::utils::message(error),
    }
}

/// What the validator said at one level, as the user reads it: in German where known, and each
/// thing once (the validator repeats a rule for every element it finds it in).
fn for_the_user(issues: Vec<Issue>, level: Level) -> Vec<Issue> {
    let mut seen = HashSet::new();
    issues
        .into_iter()
        .filter(|i| i.level == level)
        .map(crate::invoice::localize_issue)
        .filter(|i| seen.insert((i.code.clone(), i.message.clone())))
        .collect()
}

/// What came out of creating an invoice
#[derive(Debug, Serialize)]
pub struct Created {
    pub invoice: StoredInvoice,
    /// What the validator had to say without refusing the invoice
    pub warnings: Vec<Issue>,
}

/// The produced files and what freezes into the invoice with them
struct Made {
    _work: Work,
    produced: Produced,
    total: String,
    layout: Option<CustomLayout>,
}

/// Builds and validates the invoice, it only comes back if the validator accepted it
fn make(
    sidecars: &Sidecars,
    store: &Store,
    stored: &StoredInvoice,
    settings: &Settings,
    number: &str,
) -> Result<Made, ProduceError> {
    let invoice = build_invoice(&stored.draft, settings, number)
        .map_err(|problems| ProduceError::Input { problems })?;
    // German law, not a rule of the profile: the validator would let it pass
    if let Some(finding) = crate::invoice::recipient_address(&invoice)
        .filter(|f| f.severity == en16931::Severity::Fatal)
    {
        return Err(ProduceError::Invalid {
            findings: vec![finding],
        });
    }
    let total = totals(&invoice).gross;
    let customs = store.all_custom_layouts().map_err(failed)?;
    let branding = Branding::for_document(Kind::Invoice, &stored.draft, settings, &customs);
    let layout = branding.custom.clone();
    let work = Work(
        tempfile::Builder::new()
            .prefix("erng-")
            .tempdir()
            .map_err(failed)?
            .keep(),
    );
    let produced = pipeline::produce(sidecars, &work.0, &invoice, stored.draft.profile, &branding)?;
    if !produced.report.valid {
        return Err(ProduceError::NotValid {
            issues: for_the_user(produced.report.issues, Level::Error),
        });
    }
    Ok(Made {
        _work: work,
        produced,
        total,
        layout,
    })
}

/// The PDF of an offer and what freezes into it with it
struct MadeOffer {
    _work: Work,
    pdf: PathBuf,
    total: String,
    layout: Option<CustomLayout>,
}

/// Builds and renders an offer: the input is checked and the totals are made like an invoice's, the
/// PDF is the only thing that comes out.
fn make_offer(
    sidecars: &Sidecars,
    store: &Store,
    stored: &StoredInvoice,
    settings: &Settings,
    number: &str,
) -> Result<MadeOffer, ProduceError> {
    // an offer has no date of service, the one the invoice model needs is the day it is dated
    let mut draft = stored.draft.clone();
    draft.delivery_date.clone_from(&draft.issue_date);
    draft.delivery_end = None;
    let invoice = build_invoice(&draft, settings, number)
        .map_err(|problems| ProduceError::Input { problems })?;
    let total = totals(&invoice).gross;
    let customs = store.all_custom_layouts().map_err(failed)?;
    let branding = Branding::for_document(Kind::Offer, &draft, settings, &customs);
    let layout = branding.custom.clone();
    let work = Work(
        tempfile::Builder::new()
            .prefix("erng-")
            .tempdir()
            .map_err(failed)?
            .keep(),
    );
    let pdf = crate::render::render_pdf(sidecars, &work.0, &invoice, &branding).map_err(failed)?;
    Ok(MadeOffer {
        _work: work,
        pdf,
        total,
        layout,
    })
}

/// Where the numbers of the draft's context and kind stand on the draft's day: the number it would
/// get and the counter in it
fn next_sequence(store: &Store, id: i64) -> Result<Sequence, ProduceError> {
    let stored = store.invoice(id).map_err(failed)?;
    let settings = store.settings(stored.context_id).map_err(failed)?;
    store
        .sequence(
            stored.context_id,
            stored.kind,
            &settings,
            issue_day(&stored.draft.issue_date),
        )
        .map_err(failed)
}

/// The number the next document of the draft's context and kind would get, for asking before
/// creating
pub fn next_number(store: &Store, id: i64) -> Result<String, ProduceError> {
    let stored = store.invoice(id).map_err(failed)?;
    let settings = store.settings(stored.context_id).map_err(failed)?;
    store
        .next_number(
            stored.context_id,
            stored.kind,
            &settings,
            issue_day(&stored.draft.issue_date),
        )
        .map_err(failed)
}

/// Makes the invoice or offer of a draft: validated, numbered, frozen, locked, its originals kept.
pub fn create(
    sidecars: &Sidecars,
    store: &Store,
    creating: &Creating,
    data: &Path,
    id: i64,
) -> Result<Created, ProduceError> {
    let mut stored = store.invoice(id).map_err(failed)?;
    if stored.status != Status::Draft {
        return Err(failed("the invoice is created already"));
    }
    let Some(_guard) = creating.start(stored.context_id) else {
        return Err(failed(
            "Es wird gerade eine andere Rechnung erstellt, bitte einen Moment warten.",
        ));
    };
    let settings = store.settings(stored.context_id).map_err(failed)?;
    // the offers it is linked to now are the ones it names, `finalize` freezes the same
    if stored.kind == Kind::Invoice && stored.draft.offer_reference {
        stored.draft.frozen_offers = store.offer_numbers(id).map_err(failed)?;
    }
    let sequence = next_sequence(store, id)?;
    let number = sequence.next_number.clone();
    let (files, total, layout, warnings) = match stored.kind {
        Kind::Offer => {
            let made = make_offer(sidecars, store, &stored, &settings, &number)?;
            let files =
                originals::store_pdf(data, stored.kind, stored.context_id, &number, &made.pdf)
                    .map_err(failed)?;
            (files, made.total, made.layout, Vec::new())
        }
        Kind::Invoice => {
            let made = make(sidecars, store, &stored, &settings, &number)?;
            let files = originals::store(data, stored.context_id, &number, &made.produced, false)
                .map_err(failed)?;
            let warnings = for_the_user(made.produced.report.issues, Level::Warning);
            (files, made.total, made.layout, warnings)
        }
    };
    match store.finalize(id, &sequence, &settings, layout.as_ref(), &total, &files) {
        Ok(invoice) => Ok(Created { invoice, warnings }),
        Err(error) => {
            originals::discard(data, &files);
            Err(failed(error))
        }
    }
}

/// The originals of an invoice that was exported before they were kept: made again from the frozen
/// data under its number, validated like any other, and kept from now on.
pub fn regenerate(
    sidecars: &Sidecars,
    store: &Store,
    data: &Path,
    id: i64,
) -> Result<StoredInvoice, ProduceError> {
    let stored = store.invoice(id).map_err(failed)?;
    if stored.kind != Kind::Invoice {
        return Err(failed("only an invoice can be made again"));
    }
    let (Some(number), Some(frozen)) = (stored.number.clone(), stored.draft.frozen.clone()) else {
        return Err(failed("the invoice has no number and no frozen settings"));
    };
    let made = make(sidecars, store, &stored, &frozen, &number)?;
    let files: InvoiceFiles =
        originals::store(data, stored.context_id, &number, &made.produced, true).map_err(failed)?;
    store.set_files(id, &files).map_err(|error| {
        originals::discard(data, &files);
        failed(error)
    })
}

/// Hands the originals out, regenerating them first if the invoice has none
pub fn output(
    sidecars: &Sidecars,
    store: &Store,
    data: &Path,
    id: i64,
    pdf_target: Option<&Path>,
    xml_target: Option<&Path>,
) -> Result<StoredInvoice, ProduceError> {
    let mut stored = store.invoice(id).map_err(failed)?;
    if stored.status != Status::Exported {
        return Err(failed("only an invoice that was created can be handed out"));
    }
    if stored.kind == Kind::Offer && xml_target.is_some() {
        return Err(failed("Ein Angebot hat keine XML-Datei."));
    }
    if stored.files.is_none() {
        stored = regenerate(sidecars, store, data, id)?;
    }
    let files = stored.files.as_ref().ok_or_else(|| failed("no files"))?;
    originals::output(data, files, pdf_target, xml_target).map_err(failed)?;
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_validator_says_each_thing_once_and_in_german_where_known() {
        let issue = |level, code: &str, message: &str| Issue {
            section: crate::package::Section::Xml,
            level,
            code: code.into(),
            message: message.into(),
        };
        let empty = "[PEPPOL-EN16931-R008]-Document MUST not contain empty elements.";
        let issues = vec![
            issue(Level::Warning, "PEPPOL-EN16931-R008", empty),
            issue(Level::Warning, "PEPPOL-EN16931-R008", empty),
            issue(Level::Warning, "XX-1", "something else"),
            issue(Level::Notice, "BR-DE-21", "only a notice"),
        ];

        let shown = for_the_user(issues, Level::Warning);

        assert_eq!(shown.len(), 2);
        assert!(shown[0]
            .message
            .starts_with("Die Rechnung enthält leere Angaben"));
        assert_eq!(shown[1].message, "something else");
    }

    #[test]
    fn a_context_creates_one_invoice_at_a_time() {
        let creating = Creating::default();

        let first = creating.start(1);
        assert!(first.is_some());
        assert!(creating.start(1).is_none());
        // another context is no obstacle
        assert!(creating.start(2).is_some());

        drop(first);
        assert!(creating.start(1).is_some());
    }

    /// The real tools: `bun run generate:sidecars && cargo test -- --include-ignored`
    mod with_tools {
        use super::*;
        use crate::context::Context;
        use crate::draft::fixtures::{draft, settings};
        use crate::model::InvoiceDraft;

        fn setup() -> (Store, i64, tempfile::TempDir) {
            let store = Store::in_memory().unwrap();
            let context = store
                .save_context(&Context::new(settings(), ""), None, None)
                .unwrap()
                .id
                .unwrap();
            (store, context, tempfile::tempdir().unwrap())
        }

        #[test]
        #[ignore = "needs the sidecars: bun run generate:sidecars"]
        fn creating_keeps_validated_originals_and_locks_the_invoice() {
            let (store, context, data) = setup();
            let id = store
                .save_draft(None, context, Kind::Invoice, &draft(), None)
                .unwrap()
                .id;

            let created = create(
                &Sidecars::in_repo(),
                &store,
                &Creating::default(),
                data.path(),
                id,
            )
            .unwrap();

            let invoice = created.invoice;
            assert_eq!(invoice.status, Status::Exported);
            assert_eq!(invoice.number.as_deref(), Some("RE-2026-001"));
            assert!(invoice.draft.frozen.is_some());
            let files = invoice.files.unwrap();
            assert!(data.path().join(&files.pdf.path).is_file());
            assert!(data
                .path()
                .join(&files.xml.as_ref().unwrap().path)
                .is_file());
            assert!(!files.regenerated);

            // handed out as they were kept, and not created twice
            let out = tempfile::tempdir().unwrap();
            output(
                &Sidecars::in_repo(),
                &store,
                data.path(),
                id,
                Some(&out.path().join("a.pdf")),
                Some(&out.path().join("a.xml")),
            )
            .unwrap();
            assert_eq!(
                std::fs::read(out.path().join("a.pdf")).unwrap(),
                std::fs::read(data.path().join(&files.pdf.path)).unwrap()
            );
            assert!(create(
                &Sidecars::in_repo(),
                &store,
                &Creating::default(),
                data.path(),
                id
            )
            .is_err());
        }

        #[test]
        #[ignore = "needs the sidecars: bun run generate:sidecars"]
        fn a_refused_invoice_stores_nothing_and_keeps_its_number() {
            let (store, context, data) = setup();
            // XRechnung needs a buyer reference
            let mut refused = draft();
            refused.profile = crate::invoice::ProfileKind::XRechnung;
            refused.buyer_reference = None;
            refused.customer.buyer_reference = None;
            let id = store
                .save_draft(None, context, Kind::Invoice, &refused, None)
                .unwrap()
                .id;

            let result = create(
                &Sidecars::in_repo(),
                &store,
                &Creating::default(),
                data.path(),
                id,
            );

            assert!(result.is_err());
            let still = store.invoice(id).unwrap();
            assert_eq!(
                (still.status, still.number, still.files),
                (Status::Draft, None, None)
            );
            assert!(!data.path().join("invoices").exists());
            // the number of the refused one is free again
            assert_eq!(next_number(&store, id).unwrap(), "RE-2026-001");
        }

        #[test]
        #[ignore = "needs the sidecars: bun run generate:sidecars"]
        fn creating_an_offer_keeps_one_pdf_and_no_xml() {
            let (store, context, data) = setup();
            let offer = InvoiceDraft {
                validity_days: Some(30),
                ..draft()
            };
            let id = store
                .save_draft(None, context, Kind::Offer, &offer, None)
                .unwrap()
                .id;

            let created = create(
                &Sidecars::in_repo(),
                &store,
                &Creating::default(),
                data.path(),
                id,
            )
            .unwrap();

            let offer = created.invoice;
            assert_eq!(offer.status, Status::Exported);
            assert_eq!(offer.kind, Kind::Offer);
            assert_eq!(offer.number.as_deref(), Some("AN-2026-001"));
            assert!(offer.draft.frozen.is_some());
            assert!(created.warnings.is_empty());
            let files = offer.files.unwrap();
            assert_eq!(files.xml, None);
            assert!(files.pdf.path.starts_with(&format!("offers/{context}/")));
            let kept: Vec<_> =
                std::fs::read_dir(data.path().join("offers").join(context.to_string()))
                    .unwrap()
                    .collect();
            assert_eq!(kept.len(), 1);
            assert!(std::fs::read(data.path().join(&files.pdf.path))
                .unwrap()
                .starts_with(b"%PDF"));
            assert!(!data.path().join("invoices").exists());
            // the next offer goes on counting, the invoices are not touched
            assert_eq!(
                store
                    .next_number(context, Kind::Offer, &settings(), issue_day("2026-10-17"))
                    .unwrap(),
                "AN-2026-002"
            );

            // handed out as the PDF it was kept as, an XML does not exist
            let out = tempfile::tempdir().unwrap();
            assert!(output(
                &Sidecars::in_repo(),
                &store,
                data.path(),
                id,
                Some(&out.path().join("a.pdf")),
                Some(&out.path().join("a.xml")),
            )
            .is_err());
            assert!(!out.path().join("a.pdf").exists());
            output(
                &Sidecars::in_repo(),
                &store,
                data.path(),
                id,
                Some(&out.path().join("a.pdf")),
                None,
            )
            .unwrap();
            assert!(out.path().join("a.pdf").is_file());
            // an offer is not made again
            assert!(regenerate(&Sidecars::in_repo(), &store, data.path(), id).is_err());
        }

        #[test]
        #[ignore = "needs the sidecars: bun run generate:sidecars"]
        fn an_invoice_that_names_its_offers_freezes_them_and_stays_valid() {
            let (store, context, data) = setup();
            let sidecars = Sidecars::in_repo();
            let offer = store
                .save_draft(None, context, Kind::Offer, &draft(), None)
                .unwrap()
                .id;
            let offer = create(&sidecars, &store, &Creating::default(), data.path(), offer)
                .unwrap()
                .invoice;
            let naming = InvoiceDraft {
                offer_reference: true,
                ..draft()
            };
            let id = store
                .save_draft(None, context, Kind::Invoice, &naming, None)
                .unwrap()
                .id;
            store.link(offer.id, id).unwrap();

            let created = create(&sidecars, &store, &Creating::default(), data.path(), id).unwrap();

            // the XML has no reference to an offer, only the document does
            assert_eq!(created.invoice.draft.frozen_offers, ["AN-2026-001"]);
            assert!(created.invoice.files.unwrap().xml.is_some());
        }

        #[test]
        #[ignore = "needs the sidecars: bun run generate:sidecars"]
        fn an_offer_with_a_missing_price_stores_nothing() {
            let (store, context, data) = setup();
            let mut refused = draft();
            refused.positions[0].price = String::new();
            let id = store
                .save_draft(None, context, Kind::Offer, &refused, None)
                .unwrap()
                .id;

            let result = create(
                &Sidecars::in_repo(),
                &store,
                &Creating::default(),
                data.path(),
                id,
            );

            assert!(matches!(result, Err(ProduceError::Input { .. })));
            let still = store.invoice(id).unwrap();
            assert_eq!(
                (still.status, still.number, still.files),
                (Status::Draft, None, None)
            );
            assert!(!data.path().join("offers").exists());
            assert_eq!(next_number(&store, id).unwrap(), "AN-2026-001");
        }

        #[test]
        #[ignore = "needs the sidecars: bun run generate:sidecars"]
        fn an_invoice_exported_without_files_gets_them_made_again_once() {
            let (store, context, data) = setup();
            let id = store
                .save_draft(None, context, Kind::Invoice, &draft(), None)
                .unwrap()
                .id;
            create(
                &Sidecars::in_repo(),
                &store,
                &Creating::default(),
                data.path(),
                id,
            )
            .unwrap();
            // as if it had been exported by an older version
            store.forget_files(id);
            let out = tempfile::tempdir().unwrap();

            let done = output(
                &Sidecars::in_repo(),
                &store,
                data.path(),
                id,
                Some(&out.path().join("a.pdf")),
                None,
            )
            .unwrap();

            assert!(done.files.as_ref().unwrap().regenerated);
            assert!(out.path().join("a.pdf").is_file());
            assert_eq!(done.number.as_deref(), Some("RE-2026-001"));
        }
    }
}
