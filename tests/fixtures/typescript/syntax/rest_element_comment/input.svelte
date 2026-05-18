<script lang="ts">
	const [a, .../* c */ b] = x;
	const {c: d, .../* c */ e} = y;
	function f(.../* c */ args: string[]) {}
	type F = (.../* c */ args: string[]) => void;
</script>
