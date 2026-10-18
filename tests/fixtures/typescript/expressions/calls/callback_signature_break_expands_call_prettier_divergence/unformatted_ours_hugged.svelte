<script lang="ts">
	// an authored-expanded default object breaks every argument out
	fn((a = {
		b: 1
	}) => g(a));

	// new
	new Xy((a = {
		b: 1
	}) => g(a));

	// member chain
	obj.a.fn((a = {
		b: 1
	}) => g(a));

	// a pattern with a default
	fn(({ a } = {
		b: 1
	}) => g(a));

	// a preceding argument
	fn(lead, (a = {
		b: 1
	}) => g(a));

	// a preceding argument, member chain
	obj.a.fn(lead, (a = {
		b: 1
	}) => g(a));

	// a sole parameter's authored-expanded object type stays expanded; every argument breaks out
	fn((a: {
		b: C;
	}) => g(a));

	// a preceding argument
	fn(lead, (a: {
		b: C;
	}) => g(a));

	// control: a block body keeps the hug
	fn((a: {
		b: C;
	}) => {
		g(a);
	});
</script>
