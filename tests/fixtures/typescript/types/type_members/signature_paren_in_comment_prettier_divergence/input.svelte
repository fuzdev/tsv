<script lang="ts">
	// type literal call signature
	type A = {
		<T> /* a(b) */(x: T): void;
	};

	// type literal construct signature
	type B = {
		new <T> /* a(b) */(x: T): A;
	};

	// interface call signature
	interface C {
		<T> /* a(b) */(x: T): void;
	}

	// interface construct signature
	interface D {
		new <T> /* a(b) */(x: T): A;
	}
</script>
