<script lang="ts">
	// String-literal enum member names: each member `id` is a `Literal`, not an
	// `Identifier`. Identifier-valid keys (`a`) are unquoted by prettier, so these
	// use quote-requiring names to stay string literals.
	enum A {'needs quotes'=1,'has-hyphen'=2,'0leading digit'=3}

	// String-literal member name without an initializer (auto-numbered from 0)
	enum B {'no init'}
</script>
