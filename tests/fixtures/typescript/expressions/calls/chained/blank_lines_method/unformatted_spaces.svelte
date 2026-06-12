<script lang="ts">
	// Blank line before chain - chain must break (preserved)
	const  a0000000000000000000000000  =  obj000000000000000000000000000000


		.fn000000000( arg )
		.chain00( arg );

	// Blank line mid-chain - chain must break (preserved)
	const  b0000000000000000000000000  =  obj000000000000000000000000000000
		.fn000000000( arg )


		.chain00( arg );

	// Multiple blank lines in chain (all preserved)
	const  c  =  obj


		.method1( arg )


		.method2( arg )


		.method3( arg );

	// Blank line before chain - chain fits on one line (removed)
	const  d  =  obj . method1( arg ) . method2( arg );

	// Blank line before semicolon (removed)
	const  e  =  obj . method1( arg ) . method2( arg );

	// Blank line with comment mid-chain (preserved)
	const  f  =  obj
		// Comment mid-chain
		.method1( arg )


		.method2( arg );

	// No blank lines - compact chain that fits (stays compact)
	const  g  =  obj . method1( arg ) . method2( arg ) . method3( arg );

	// No blank lines - long chain that breaks
	const  h0000000000000000000000000  =  obj000000000000000000000000000000
		.fn000000000( arg )
		.chain00( arg );
</script>
