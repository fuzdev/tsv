<script lang="ts">
	// Call expression callee with comment in removed parens
	const a = /* comment */ fn();

	// Member expression object with comment
	const b = /* comment */ obj.prop;

	// Member call (common with @__PURE__ annotations)
	const c = /* @__PURE__ */ new Date().toISOString();

	// New expression callee with comment
	const d = new /* comment */ Foo();
</script>
