<script lang="ts">
	// A line comment in the head→body gap stays where the author wrote it, and the
	// body `{` drops to its own line, flush with the head.
	function a() // c
	{
		fn();
	}

	class A {
		b() // c
		{
			fn();
		}

		get c() // c
		{
			return 1;
		}

		constructor() // c
		{}
	}

	let d = function () // c
	{
		fn();
	};

	let e = {
		f() // c
		{
			fn();
		}
	};

	// The class and interface body braces already answer the gap this way.
	class B // c
	{
		x = 1;
	}

	interface C // c
	{
		x: number;
	}
</script>
