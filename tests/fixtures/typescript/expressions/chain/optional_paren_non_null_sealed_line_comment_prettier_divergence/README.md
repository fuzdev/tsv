# Sealed optional-chain non-null, line comment before the `!`

The required-paren sibling of
[grouped_operand_member_line_comment](../../non_null/grouped_operand_member_line_comment_prettier_divergence/),
at the two positions a sealed optional chain never reaches through the member
chain: a `new` callee and a template tag (the parens there are required — the
bare `new a?.b!()` / `` a?.b!`tpl` `` are syntax errors, see
[optional_paren_non_null_new_callee](../optional_paren_non_null_new_callee/) and
[optional_paren_non_null_tag_boundary](../optional_paren_non_null_tag_boundary/)).
The member-access position reaches the same gap through the chain printer (the
sealed parenthesized base); the assignment hugs the shell (`const m = (`), whose
break is the comment's, not the operator's.

- **tsv**: keeps the comment inside the parens where the author wrote it,
  forcing the multiline paren layout — the same shape the chain path already
  produces for `(x + y // c)!.foo`.
- **prettier**: relocates the comment outside, after `)!`, leaving the call
  arguments (`()`) and the template (`` `tpl` ``) stranded on the next line.

A line comment can't trail inline before `)` (the `//` would swallow it), so
unlike the block-comment case — which stays inline and matches prettier, see
[optional_paren_non_null_sealed_comment](../optional_paren_non_null_sealed_comment/)
— it forces the operand onto its own line.

Reason: comment preservation. See
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation (Non-null grouped operand) and
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy.
