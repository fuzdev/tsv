<script>
	// Method chain with array of 3+ objects (2+ props each):
	// the opening bracket stays hugged (`fn(a).b([`)
	fn(a).b([{aaaaaa:'bbbbbbbbb',value:2},{aaaaaa:'bbbbbbbbb',value:4},{aaaaaa:'bbbbbbbbb',value:6}]);

	// Same pattern with expect().toEqual() - common in tests
	expect(results).toEqual([{aaaaaa:'bbbbbbbbb',value:2},{aaaaaa:'bbbbbbbbb',value:4},{aaaaaa:'bbbbbbbbb',value:6}]);

	// 2 objects with 2+ props - same hugged expansion
	fn(a).b([{aaaaaa:'bbbbbbbbb',value:2},{aaaaaa:'bbbbbbbbb',value:4}]);

	// 3+ objects with 1 prop each - single-prop objects stay inline
	fn(a).b([{a:1},{b:2},{c:3}]);
</script>
