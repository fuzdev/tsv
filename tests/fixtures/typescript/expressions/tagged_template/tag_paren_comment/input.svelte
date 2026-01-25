<script lang="ts">
	// Tagged template with comment in removed parens
	const a = /* comment */ tag`template`;

	// Call expression as tag with comment
	const b = /* comment */ fn()`template`;

	// Tagged template in chain
	const c = /* comment */ tag`template`.length;

	// Nested call in tag
	const d = /* @__PURE__ */ getTag()`template`;
</script>
