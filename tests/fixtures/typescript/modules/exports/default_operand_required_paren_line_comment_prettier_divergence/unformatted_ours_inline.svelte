<script lang="ts">
	// a class or function value takes a pair at this position anyway: the authored pair is
	// that pair, printed once, with the comment inside it
	export default (class E {} // c1
	); // c2
	export default (function () {} // c3
	); // c4
	export default (function () {}.m() // c5
	);
</script>
