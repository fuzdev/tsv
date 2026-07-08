<script lang="ts">
	// interface method
	interface A {
		a1(): number /* comment */;
	}

	// type literal method
	type B = {
		a2(): number /* comment */;
	};

	// property signature
	interface C {
		a3: number /* comment */;
	}

	// call signature
	type D = {
		(): number /* comment */;
	};

	// construct signature
	type E = {
		new (): number /* comment */;
	};
</script>
