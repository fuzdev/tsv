<script lang="ts">
	// a redundant paren around a huggable parameter annotation is dropped and changes no
	// layout: the sole parameter still hugs, so `(a: {` stays on the signature line

	// function declaration
	function f1(a: ({
		[K in T]: U
	})) {}

	// class method
	class C {
		m(a: ({
			[K in T]: U
		})) {}
	}

	// arrow
	const f2 = (a: ({
		[K in T]: U
	})) => {};

	// bodyless overload signature
	function f3(a: ({
		[K in T]: U
	})): boolean;

	// interface method signature
	interface I {
		m(a: ({
			[K in T]: U
		})): void;
	}

	// function type
	type X1 = (a: ({ aaaaaaaaaaaaaaa: A; bbbbbbbbbbbbbbb: B; ccccccccccccccc: C; ddddddddddddddd: D })) => void;

	// constructor type
	type X2 = new (a: ({ aaaaaaaaaaaaaaa: A; bbbbbbbbbbbbbbb: B; ccccccccccccccc: C; ddddddddddddddd: D })) => void;
</script>
