<script lang="ts">
	// Function parameters
	function processArray([first, second]) {
		return first + second;
	}

	function processObject({a, b}) {
		return a + b;
	}

	// Arrow functions
	const fn1 = ([a, b]) => a + b;
	const fn2 = ({x, y}) => x + y;

	// With defaults
	function withDefaults({a = 1, b = 2} = {}) {
		return a + b;
	}

	// Rest in params
	function withRest([a, ...rest]) {
		return rest;
	}
	const arrowRest = ({a, ...rest}) => rest;

	// Nested params
	function nestedArray([[a, b]]) {
		return a + b;
	}
	function nestedObject([{x, y}]) {
		return x + y;
	}

	// Multiple pattern params
	function multiArray([a], [b]) {
		return a + b;
	}
	function mixedParams([a], {b}) {
		return a + b;
	}

	// Pattern defaults
	function arrayDefault([a, b] = [1, 2]) {
		return a + b;
	}
	function objectDefault({x, y} = {x: 1, y: 2}) {
		return x + y;
	}

	// Computed in params
	function computed({[key]: value}) {
		return value;
	}
</script>
