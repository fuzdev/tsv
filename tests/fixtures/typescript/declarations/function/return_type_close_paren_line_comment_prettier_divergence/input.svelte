<script lang="ts">
	// A line comment in the `)`→return-type-`:` gap stays trailing `)`, with `:`
	// forced onto the next line (the `)`→`=>` gap does the same). Prettier instead
	// relocates the comment onto the last parameter and breaks the params.

	// function declaration
	function f(a: string) // c
	: string {
		return a;
	}

	// class method
	class C {
		m(a: string) // c
		: string {
			return a;
		}
	}

	// function expression
	const g = function (a: string) // c
	: string {
		return a;
	};

	// object method
	const o = {
		m(a: string) // c
		: string {
			return '';
		}
	};

	// type-member method signature
	type T = {
		m(a: string) // c
		: string;
	};

	// arrow function — the param breaks (arrows are excluded from the single-param
	// hug the signature builders above apply), but the comment still stays after `)`
	const h = (
		a: string
	) // c
	: string => a;
</script>
