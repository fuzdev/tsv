<script lang="ts">
	// A non-null assertion on a parenthesized optional-chain base, used as a `new`
	// callee, keeps the parens. `new (a?.b)!()` constructs on the asserted result of
	// `a?.b`; stripping the parens to `new a?.b!()` is a syntax error (an optional
	// chain can't be a `new` callee), so the parens are required.
	new (a?.b)!();
	new (a?.())!();
</script>
