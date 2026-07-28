<script lang="ts">
	// an own-line directive in the `export`→declaration gap freezes the declaration
	export
		// prettier-ignore
		const  aaa  =  1;

	// every declaration form is the same head
	export
		// prettier-ignore
		function  fn  ()  {}

	export
		// prettier-ignore
		type  A  =  {  a:  1  };

	export
		// prettier-ignore
		interface  B  {
			b:  2
		}

	export
		// prettier-ignore
		enum  C  {  c  }

	// decorators written after `export` belong to the declaration and ride inside its slice
	export
		// prettier-ignore
		@dec
		class  D  {}

	// an own-line block comment behaves identically — placement keys the freeze
	export
		/* prettier-ignore */
		const  eee  =  2;
</script>
