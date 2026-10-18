<script lang="ts">
	// a parameter's authored-expanded object type breaks every argument out
	reduce((a, b: {
		c: string;
	}) => {
		return 1;
	}, '');

	// new
	new Reduce((a, b: {
		c: string;
	}) => {
		return 1;
	}, '');

	// member chain
	obj.reduce((a, b: {
		c: string;
	}) => {
		return 1;
	}, '');

	// an authored-expanded default object
	reduce((a, b = {
		c: 1
	}) => {
		return 1;
	}, '');

	// an authored-expanded object return type
	reduce((a, b): {
		c: string;
	} => {
		return 1;
	}, '');

	// a matrix default array breaks every argument out, though no newline spells the break
	reduce((a = [[1, 2], [3, 4]]) => {
		return 1;
	}, '');

	// a default object inside a sole destructured parameter
	reduce(({ a = {
		b: 1
	} }) => {
		return 1;
	}, '');

	// a comment in an empty parameter list
	reduce((
		// c
	) => {
		return 1;
	}, '');

	// control: a function keeps the hug, its parameters broken
	reduce(function (a, b: {
		c: string;
	}) {
		return 1;
	}, '');
</script>
