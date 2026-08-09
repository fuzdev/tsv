<script lang="ts">
	// an own-line directive in the header→`{` gap freezes the whole enum body
	enum Aaa // prettier-ignore
	{
		mmm   =   1,   nnn
	}

	// a `const enum` shares the head, so it shares the freeze
	const enum Bbb // prettier-ignore
	{
		ooo   =   2
	}

	// an own-line block comment behaves identically — placement keys the freeze
	enum Ccc /* prettier-ignore */
	{
		ppp   =   3
	}
</script>
