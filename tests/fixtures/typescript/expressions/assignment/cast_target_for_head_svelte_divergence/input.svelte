<script lang="ts">
	// A type assertion around a simple target is a valid no-declaration for-in/of head.
	// The non-null postfix, bare and inside a destructuring pattern.
	for (a! of arr) {
	}
	for (a.b! in obj) {
	}
	for ([a!, { b: c! }] of arr) {
	}
	// The binary-level assertions print without parens in a for head.
	for (a as T of arr) {
	}
	for (a satisfies T in obj) {
	}
	for (<T>a of arr) {
	}
	for ([a as T] of arr) {
	}
</script>
