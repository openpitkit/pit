# checks

Deterministic checks for the invariants this project states in prose but no
language linter enforces: domain value types, unagreed fallbacks, reachable
panics, identifier privacy, binding isolation, text hygiene.

`clippy`, `go vet`, `golangci-lint`, `ruff`, `black` and `eslint` are not
duplicated here. Anything they already catch belongs to them.

## How to run

```sh
just check-semgrep
```

The target is a prerequisite of `lint-all`, so `check-debug`, `check-release`
and `check-full` run it too. In CI it is the `Invariant checks` job of the
verify workflow: the per-language jobs run per-language targets, and none of
those reach `lint-all`.

The scanner is declared in `checks/requirements.txt`, next to the rules;
the repository-root `requirements.txt` includes that file, so
`just _ensure-python-env` still provisions everything in one step.

No native Windows build of the scanner is published, so the dependency is
marked off Windows and the target fails there instead of reporting a pass it
never ran. Run the gate from macOS, Linux, or WSL.

## Scope

The rules carry no `paths.include` unless the invariant itself is scoped to one
area, so a rule sees every text file in the repository: sources of every
language, tests, Markdown, YAML, JSON, the justfiles, CI definitions.

What stays out of scope is named once, in `.semgrepignore` at the repository
root - generated output, caches, third-party trees, machine-written manifests.
That file lives at the root because the scanner reads it only from the working
directory. Its presence replaces the scanner's bundled default ignore list,
which excludes test files; this project does not, because a message asserted in
a test is the message a user sees.

## Rules

- `semgrep/text-style.yml` - ASCII punctuation, no invisible characters, no
  emoji, no narrating comments.
- `semgrep/isolation.yml` - no reference to unpublished design material, no
  tracker identifiers, no binding documenting itself by pointing at a sibling.
- `semgrep/privacy.yml` - account and account-group identifiers never rendered
  into a message.
- `semgrep/financial-types-rust.yml` - domain value types instead of `f64`, no
  raw `Decimal` in a public signature, no numeric casts in policy calculations.
- `semgrep/financial-types-go.yml` - domain value types instead of `float64` in
  the Go binding packages.
- `semgrep/fallbacks-rust.yml` - no unagreed fallback, no catch-all arm over a
  domain enum, no panic on a reachable path.
- `semgrep/errors-go.yml` - no discarded error, no error checked and then
  dropped.
- `semgrep/tests/` - regression fixtures for `financial-types-rust.yml` and
  `fallbacks-rust.yml`; `errors-go.yml` has no fixture.
  `just check-semgrep` runs `semgrep --test` over them after the repository
  scan; they hold deliberate violations.

## The agreement markers

Two markers, each named after what it approves, both read from the line
directly above the construct.

`identifier-in-rendered-message` matches the identifier type names as well as
the variable spellings, so a type that renders itself - a `Display`, a
`__repr__` - lands in the gate and is marked in place:

```rust
// identifier(approved): the repr of the identifier type is the identifier
format!("AccountId(value={:?})", self.value())
```

Defining those away in the rule instead would take every literal mention of an
identifier type out of the gate along with them.

`unapproved-fallback` and `unapproved-catch-all` do not forbid the construct.
They forbid an *unagreed* one. Agreement is recorded on the line directly
above, in the comment syntax of the language:

```rust
// fallback(approved): unbounded by design, the caller rejected a stale book
let age = quote.age().unwrap_or(Duration::ZERO);
```

Directly above is literal: both rules read the raw text and look at exactly one
preceding line, so a marker wrapped over two lines leaves the rule firing. Keep
it on one line, or put the explanation in the lines above the marker.

`panic-on-reachable-path` takes no marker. It needs the syntax tree to keep
inline `mod tests` blocks out of scope, and the scanner drops comments before
the tree is built, so a marker there would be invisible and silently
ineffective. An approved exception is recorded instead by naming the file in
that rule's `paths.exclude`, with the reason - a decision that shows up in the
diff.

The marker records a decision that was taken. It never silences a false
positive: a rule that fires on correct code is narrowed instead, in the rule
file, with the reason in a comment.

When the scanner misreports one specific construct, that narrowing can be a
signature-level `pattern-not` inside the rule. It records the exception beside
the rule itself, where the diff shows it, without widening the exemption.

## The baseline

Bringing the whole repository to zero is not the goal, and never was. Existing
code is worked out of the gate as development reaches it. The instrument is
file-granular: a baseline entry suspends its rule for the whole listed file,
code added to it later included, until the entry is cleared. That is the price
of this mechanism, and the reason the list has to stay short.

A file that carries pre-existing findings is listed in that rule's
`paths.exclude` under a `# Baseline:` comment that names the pending work in
words and, in one line, what is actually wrong. The entry is a debt, not a
verdict: it says the code was never reviewed against the rule, not that the
construct was approved.

Two rules follow from that.

- **A file leaves the list when the work reaches it**, and only then. The
  finding is resolved the way the rule prescribes - an explicit refusal, or a
  marker recording the decision - and the line is deleted in the same change.
- **The list never grows for new code.** A finding in a file that is not on the
  list is a defect in the change that produced it. Adding a line to make it go
  away is the one move this whole mechanism exists to prevent.

The exclusions live in the rules rather than in the repository-root
`.semgrepignore` on purpose: that file removes a path from every rule at once,
and a file with one unagreed fallback still has to answer to the other rules.
