<script lang="ts">
	// an own-line directive in the header→`{` gap freezes the whole namespace body
	namespace Aaa
	// prettier-ignore
	{
		const mmm   =   1;
	}

	// `module` shares the head, so it shares the freeze
	module Bbb
	// prettier-ignore
	{
		const nnn   =   2;
	}

	// `declare global` — `global` is both keyword and name, and the gap follows it
	declare global
	// prettier-ignore
	{
		const ooo   =   3;
	}

	// an own-line block comment behaves identically — placement keys the freeze
	namespace Ccc
	/* prettier-ignore */
	{
		const ppp   =   4;
	}
</script>
