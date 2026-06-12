<script lang="ts">
	// new with class expression - needs parens
	const a = new (class {})();
	const b = new (class Foo {})();
	const c = new (class extends Base {})();

	// contrast: regular callee - no parens needed
	const d = class {}();
</script>
