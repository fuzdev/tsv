<script lang="ts">
	// an own-line directive in the `export default`→value gap freezes the value
	export default // prettier-ignore
	fn  (  aaa  );

	// a declaration value is the same head
	export default // prettier-ignore
	function  fn  ()  {}

	export default // prettier-ignore
	interface  B  {
			b:  2
		}

	// decorators written after `default` belong to the class and ride inside its slice
	export default // prettier-ignore
	@dec
		class  D  {}

	// an own-line block comment behaves identically — placement keys the freeze
	export default /* prettier-ignore */
	{  aaa:  1  };
</script>
