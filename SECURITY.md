# Security Policy

## Supported versions

calcli is pre-1.0, so only the latest released version on
[crates.io](https://crates.io/crates/calcli) receives security fixes. Please
upgrade before reporting an issue.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue. Use
GitHub's private vulnerability reporting on the
[repository's Security tab](https://github.com/cgroening/rs-calcli/security/advisories/new)
("Report a vulnerability"). Include the affected version, a description of the
problem and, where possible, steps to reproduce it.

You can expect an acknowledgement within a few days. Once a fix is ready it is
released and the advisory is published.

## Scope

calcli is an offline terminal calculator. It reads its own configuration and
session files and does not open network connections. The most relevant surface
is therefore the parsing of user input and of the on-disk `config.toml` and
`state.toml`; reports about crashes or unexpected behaviour there are welcome.
