# handler_prettier_ignore_head_prettier_divergence

An own-line directive in the `}`→`catch` gap (or `}`→`finally`), where the following
node is the whole clause. tsv freezes that clause over its own node span, so the
`catch` binding rides inside the frozen slice:

```ts
try {
	fn(aaa);
}
// prettier-ignore
catch (  eee  ) {
	fn(  bbb  );
}
```

Prettier **relocates** the directive into the clause's block body and freezes the
first statement there instead — so its `catch` binding normalizes (`catch (eee)`)
while `fn(  bbb  )` freezes. Two different nodes end up frozen from the same
authoring.

The relocation is the same one the plain-comment form already pins in
[catch_between_comment](../catch_between_comment_prettier_divergence/): prettier moves
a comment between try/catch/finally blocks into the subsequent block body, and tsv
keeps it where the author wrote it. Adding the directive only changes *what* the kept
position then freezes. **Both spellings** behave alike — placement keys the freeze,
not the comment's spelling.

## Reason

tsv never relocates a directive: the placement the author wrote is the placement that
decides the freeze, so the frozen node is the one that actually follows the directive;
◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
