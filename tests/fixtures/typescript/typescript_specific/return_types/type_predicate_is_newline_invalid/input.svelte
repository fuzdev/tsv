<script lang="ts">
	// `parameterName [no LineTerminator here] is Type`: a line terminator before
	// `is` makes it not a type predicate, so `is` is a stray token (acorn-typescript
	// `hasPrecedingLineBreak`). The valid baseline keeps `is` on the parameter line.
	function f(x: unknown): x is string {
		return typeof x === 'string';
	}
</script>
