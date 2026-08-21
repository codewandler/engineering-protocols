//! The billing system, realized, in a browser.
//!
//! The generated bridge under `generated/web/billing/` knows the model and the wire format and
//! nothing about behaviour: with nothing installed it runs the generated stubs, and every command
//! comes back with the typed refusal naming the obligation the plan owes. This crate is the other
//! half — it links [`billing_realization`]'s honest implementation of all eight obligations and
//! hands the assembled system over.
//!
//! # One export, and it is optional by design
//!
//! [`ess_realize`] is the hook the emitted page calls *if the module has it*. That is what lets one
//! page drive two modules: the generated bridge alone, which shows the plan by refusing, and this
//! one, which shows the system by running. Neither was written against the other.
//!
//! # The linker still does not choose
//!
//! [`billing_realization::linker::honest`] offers exactly one implementation per obligation and
//! resolves them — zero is an unsatisfied obligation, two is an ambiguity naming both (gap
//! register D-2). This crate adds no policy on top of it; it takes the assembled system and
//! installs it.

#![deny(missing_docs)]

// No `#![forbid(unsafe_code)]`: a WebAssembly export is a `#[no_mangle]` item, and rustc's
// `unsafe_code` lint flags one. There is no `unsafe` block, no `unsafe fn` and no raw-pointer
// dereference below — the same weakening the bridge's own `TARGET.md` states.

/// Installs the honest realization of every billing obligation, and answers nothing.
///
/// The emitted page calls this once, before its first request. Calling it again replaces the
/// installed system with a fresh one — an empty invoice store, an empty log — which is the only
/// reset a page has.
#[no_mangle]
pub extern "C" fn ess_realize() {
    // `Assembled` also hands back the provider control and the invoice store; both are `Rc`
    // handles the realizations already hold clones of, so dropping them here takes nothing away.
    // The conformance runner needs them to inject a provider refusal; a person clicking a page
    // does not, and a browser realization that could force an outcome would be a system whose
    // transport can lie.
    let assembled = billing_realization::linker::honest();
    billing_web::install(Box::new(assembled.system));
}
