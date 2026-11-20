# Local compatibility patch

Source: crates.io `xattr` 0.2.3, used transitively by GPUI 0.2.2.
Upstream: https://github.com/Stebalien/xattr
Original MIT and Apache licenses are included unchanged.

The only source change maps the crate's internal `ENOATTR` constant to
`libc::ENODATA` on Linux/Android. Current libc no longer supplies the old alias
on those targets. macOS and BSD continue using their native `ENOATTR`.
Remove this patch when the GPUI dependency chain no longer requires xattr 0.2.3.
