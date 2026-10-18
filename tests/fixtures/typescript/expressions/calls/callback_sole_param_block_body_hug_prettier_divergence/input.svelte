<script lang="ts">
	// a sole parameter's authored-expanded object type stays expanded, and a block body keeps
	// the hug
	fn((a: {
		b: C;
	}) => {
		g(a);
	});

	// new
	new Xy((a: {
		b: C;
	}) => {
		g(a);
	});

	// member chain
	obj.a.fn((a: {
		b: C;
	}) => {
		g(a);
	});

	// a preceding argument
	fn(lead, (a: {
		b: C;
	}) => {
		g(a);
	});

	// a return type
	fn((a: {
		b: C;
	}): void => {
		g(a);
	});

	// the first argument
	reduce((a: {
		b: C;
	}) => {
		return 1;
	}, '');

	// the first argument, new
	new Reduce((a: {
		b: C;
	}) => {
		return 1;
	}, '');

	// the first argument, member chain
	obj.reduce((a: {
		b: C;
	}) => {
		return 1;
	}, '');

	// the object type annotating a sole destructured parameter
	fn(lead, ({
		typography
	}: {
		typography: T;
	}) => {
		g(typography);
	});

	// a function callback's hugged sole parameter
	fn(lead, function (a: {
		b: C;
	}) {
		g(a);
	});
</script>
