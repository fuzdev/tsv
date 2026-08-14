<script lang="ts">
	// A type-parameter modifier binds to its name only on the SAME line (tsc
	// `nextTokenIsOnSameLineAndCanFollowModifier`), so the valid baseline keeps every
	// modifier glued to the parameter it marks.
	class Variance<const in out T> {}
	interface Invariant<in out T> {}
	function withConst<const T>() {}
	type Alias<out T> = () => T;
</script>
