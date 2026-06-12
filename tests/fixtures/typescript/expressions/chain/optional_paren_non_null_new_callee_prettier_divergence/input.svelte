<script lang="ts">
	// A non-null assertion on a parenthesized optional-chain base, used as a `new`
	// callee, keeps the parens. `new (a?.b)!()` constructs on the asserted result of
	// `a?.b`. Prettier strips the parens to `new a?.b!()` — but that is a syntax
	// error (an optional chain can't be a `new` callee), so prettier's own output
	// fails to re-parse. tsv keeps the parens (correct, idempotent).
	new (a?.b)!();
	new (a?.())!();
</script>
