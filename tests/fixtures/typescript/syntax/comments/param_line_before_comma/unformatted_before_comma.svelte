<script lang="ts">
	// Line comment trailing a parameter
	function fn(
		a: A // c
		, b: B
	) {}

	// Arrow parameters
	const fn2 = (
		a: A // c
		, b: B
	) => {};

	// Class method
	class C {
		m(
			a: A // c
			, b: B
		) {}
	}
</script>
