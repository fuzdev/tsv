<script lang="ts">
	// arrow - line comment after last param
	const a1 = (x:{b:number},
	// line comment
	):void => {};

	// arrow - block comment after last param
	const a2 = (x:{b:number},
	/* block comment */
	):void => {};

	// function expression - line comment
	const a3 = function(x:{b:number},
	// line comment
	):void {};

	// function expression - block comment
	const a4 = function(x:{b:number},
	/* block comment */
	):void {};

	// class method - line comment
	class A {a5(x:{b:number},
	// line comment
	) {}}

	// class method - block comment
	class B {a6(x:{b:number},
	/* block comment */
	) {}}

	// function declaration - line comment
	function a7(x:number,
	// line comment
	):number {return x;}

	// function declaration - block comment
	function a8(x:number,
	/* block comment */
	):number {return x;}

	// function type - block comment (line comment already works)
	type F = (x:{b:number},
	/* block comment */
	) => number;

	// object method - line comment
	const c = {a9(x:{b:number},
	// line comment
	) {}};

	// object method - block comment
	const d = {a10(x:{b:number},
	/* block comment */
	) {}};
</script>
