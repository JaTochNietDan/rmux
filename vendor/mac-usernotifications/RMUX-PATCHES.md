# rmux patch provenance

Vendored from crates.io `mac-usernotifications` 0.3.1, licensed MIT OR Apache-2.0
as declared by upstream Cargo.toml. Upstream:
https://github.com/hoodie/mac-usernotifications

The response wait now borrows the oneshot receiver while retaining PendingGuard.
Upstream `into_receiver` used ManuallyDrop, abandoning the request-id String and
disabling deregistration on cancellation, timeout and silent dismissal. Keeping
the guard through the wait releases both on all exits, with no unsafe extraction.
Two regression tests cover a polled pending wait and an unpolled future being
dropped. They do not call Notification Center or request permission.

Remove this patch when an upstream version includes equivalent lifetime cleanup.
