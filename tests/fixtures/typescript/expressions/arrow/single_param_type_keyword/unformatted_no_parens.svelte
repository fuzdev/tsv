<script lang="ts">
	// A bare single arrow parameter may be a contextual keyword; both formatters
	// normalize it to the parenthesized form (arrowParens: always)
	const fn1 = any => any;
	const fn2 = string => string;
	const fn3 = async => async;
	const fn4 = from => from;
</script>
