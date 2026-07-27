<script lang="ts">
	// an own-line directive in a default's `=`→value gap freezes the whole value
	function fn(
		aaa =
			// prettier-ignore
			bbb  +  ccc
	) {}

	// a destructuring default is the same head
	const {
		ddd: eee =
			// prettier-ignore
			fff  +  ggg
	} = hhh;

	// an array-pattern default is the same head
	const [
		iii =
			// prettier-ignore
			jjj  ||  kkk
	] = lll;
</script>
