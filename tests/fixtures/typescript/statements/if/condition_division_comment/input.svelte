<script lang="ts">
	// A condition ending in a division, then a comment: the comment stays inside the
	// parens, where the author wrote it.

	// After a type-argument list's `>`
	if (f<T> / 2 /* c */) {
	}
	if (f<T> / 2 && /[)]/.test(b) /* c */) {
	}

	// After a postfix non-null `!`
	if (a! / 2 /* c */) {
	}
	if (a! / 2 && /[)]/.test(b) /* c */) {
	}
</script>
