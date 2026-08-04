# body_prettier_ignore_empty_prettier_divergence

An own-line directive in a class head→`{` gap whose body is **empty**. tsv freezes the body
like any other (`{}` is the whole slice) and the input is its fixed point.

Prettier **throws** on it — `Comment "prettier-ignore" was not printed. Please report this
error!` — its own internal assertion that every comment reached the output. Its class printer
takes the empty-body path, which emits no member for the relocated directive to lead, so the
comment is dropped and the assertion fires. There is no prettier output to compare against,
hence `prettier_rejects.txt` rather than `output_prettier.svelte`.

The non-empty body is the ordinary case, cataloged at
[body_prettier_ignore_head](../body_prettier_ignore_head_prettier_divergence/) — prettier
relocates the directive into the body there and freezes the first member.

## Reason

◆prettier_bug — prettier crashes on valid input that tsv formats stably. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(cataloged in [conformance_prettier_ts.md §Prettier rejects valid input](../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input), where prettier's own assertion firing is the divergence)
and §"Prettier rejects valid input".
