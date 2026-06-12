<script lang="ts">
	// interface method - line comment
	interface A {
		a1(x:number,
		// line comment
		):void;
	}

	// interface method - block comment
	interface B {
		a2(x:number,
		/* block comment */
		):void;
	}

	// call signature - line comment
	type C = {
		(x:number,
		// line comment
		):void;
	};

	// call signature - block comment
	type D = {
		(x:number,
		/* block comment */
		):void;
	};

	// construct signature - line comment
	type E = {
		new(x:number,
		// line comment
		):A;
	};

	// construct signature - block comment
	type F = {
		new(x:number,
		/* block comment */
		):A;
	};
</script>
