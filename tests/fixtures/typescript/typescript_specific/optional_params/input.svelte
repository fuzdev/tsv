<script lang="ts">
	// bare optional params (no type annotation) across positions
	const fn1 = (a?) => a;
	function fn2(a, b?) {}
	type F = (a?) => void;
	class C {
		constructor(a?, b?) {}
		fn3(a?) {}
	}

	// annotated optional params for contrast
	const fn4 = (a?: T) => a;
	function fn5(a, b?: T) {}
</script>
