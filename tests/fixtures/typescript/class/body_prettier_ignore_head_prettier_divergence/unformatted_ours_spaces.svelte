<script lang="ts">
	// an own-line directive in the heritage→`{` gap freezes the whole class body
	class   Aaa
	// prettier-ignore
	{
		mmm(  ) {}
	}

	// a heritage clause is parent-owned and normalizes; the body still freezes
	class   Bbb   extends   Aaa
	// prettier-ignore
	{
		nnn   =   1;
	}

	// a class expression shares the head, so it shares the freeze
	const   ccc   =   class
	// prettier-ignore
	{
		ooo   =   1;
	};

	// an own-line block comment behaves identically — placement keys the freeze
	class    Ddd
	/* prettier-ignore */
	{
		ppp(  ) {}
	}
</script>
