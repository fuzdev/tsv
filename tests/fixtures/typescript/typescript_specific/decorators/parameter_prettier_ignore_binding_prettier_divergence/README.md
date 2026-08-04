# parameter_prettier_ignore_binding_prettier_divergence

An ignore directive alone on its line between a parameter's **decorators** and its
**binding** freezes the binding — the node the directive precedes. The decorators
print normally and stay outside the frozen slice; the directive keeps the line the
author gave it. (A directive before the *first* decorator is the parameter-list
gap instead, and freezes the whole parameter, decorators included —
[params member](../../../declarations/function/params_prettier_ignore_member/).)

Prettier honors neither position here:

- at a **parameter property** (`@dec⏎// prettier-ignore⏎private a: T`) it freezes
  nothing at all and reformats the binding;
- at a plain **binding** (`@dec1⏎@dec2⏎// prettier-ignore⏎c: T`) it re-binds the
  directive *past the name* — trailing `c`, freezing only the `: T` annotation —
  and that form is not self-stable: its own second pass floats the directive up to
  trail `@dec2` and **loses the freeze** (pinned by `audit_signature.txt`).

## Reason

A directive freezes the construct it precedes, and a placement tsv honors must be
one it can reproduce: a name-trailing directive is inert under the placement
classification, so prettier's relocated form would lose the freeze on tsv's second
pass.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
